#!/usr/bin/env bash
# ci-restore-sim.sh — reproduce a cold-CI binary-cache *restore* locally, in a
# sandbox, and measure how compression + Nix substitution tuning change it.
#
# Why this exists: on a fresh ephemeral runner the dominant cost of a hermetic
# Flutter build is not the compile — it is Nix realising the dependency closure
# from the remote cache (measured on one consumer: ~11 min of a ~14.5 min step;
# the compile was ~43 s). That is hard to see in CI logs and impossible to
# A/B-test there. This harness recreates it on one machine with no cloud and no
# root:
#
#   - a *simulated remote cache*: the closure is pushed to a `file://` binary
#     cache (per compression), then served over HTTP by a throttling static
#     server (below) — S3/HTTP GET semantics, but bandwidth- and latency-limited;
#   - *cold restore*: each cell substitutes into a fresh throwaway store, so every
#     path is a miss and must come over the throttled link;
#   - *CPU limit*: the restore is pinned to N cores (taskset) to mirror an
#     N-vcpu runner (xz decompression is single-threaded, so cores matter);
#   - a *matrix* of {xz,zstd} × {default,tuned} Nix settings, timed.
#
# Calibration to a real consumer: effective restore throughput ≈ closure_bytes /
# restore_seconds. ~5.6 GB in ~11 min ≈ ~8.5 MB/s. So --rate 8 reproduces that
# per-byte timing; a smaller test closure finishes faster but the *ratios*
# (xz/default vs zstd/tuned) and the per-GB extrapolation hold. Use a real
# closure for a faithful absolute number:
#
#   nix build .#<app>.offlineDeps -o result-deps     # or any heavy installable
#   ./benchmarks/ci-restore-sim.sh --store-path result-deps --rate 8 --cpus 8
#
# Read-only w.r.t. /nix/store and ~/.gradle etc.: it only writes under a temp dir
# (removed on exit unless --keep) and into throwaway stores it creates there.
#
# Usage:
#   ci-restore-sim.sh --store-path <result-or-store-path> [opts]
# Options:
#   --rate <MB/s>     aggregate link-level bandwidth cap  (default 8)
#   --latency <ms>    added per-object (GET) latency      (default 30)
#   --cpus <N>        cores to pin the restore to         (default 8)
#   --cells "<list>"  space-separated comp:mode cells     (default all four)
#                     comp ∈ {xz,zstd}; mode ∈ {default,tuned}
#   --keep            keep the temp work dir for inspection
set -euo pipefail

store_arg=""
rate=8
latency=30
cpus=8
cells="xz:default xz:tuned zstd:default zstd:tuned"
keep=0
while [ $# -gt 0 ]; do
  case $1 in
    --store-path) store_arg=$2; shift 2 ;;
    --rate) rate=$2; shift 2 ;;
    --latency) latency=$2; shift 2 ;;
    --cpus) cpus=$2; shift 2 ;;
    --cells) cells=$2; shift 2 ;;
    --keep) keep=1; shift ;;
    -h|--help) sed -n '2,/^set -euo/p' "$0" | sed 's/^# \{0,1\}//; s/^#//'; exit 0 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

[ -n "$store_arg" ] || { echo "error: --store-path is required (build a heavy installable first, e.g. nix build .#<app>.offlineDeps -o result-deps)" >&2; exit 2; }
for c in nix python3; do command -v "$c" >/dev/null || { echo "error: '$c' not on PATH" >&2; exit 1; }; done
[ -e "$store_arg" ] || { echo "error: '$store_arg' does not exist" >&2; exit 1; }
store_path=$(readlink -f "$store_arg")

# CPU pinning is best-effort: taskset if present, else run unpinned with a note.
taskset_pin=()
if command -v taskset >/dev/null && [ "$cpus" -gt 0 ]; then
  taskset_pin=(taskset -c "0-$((cpus - 1))")
else
  echo "note: taskset unavailable or --cpus 0 — running unpinned (CPU not limited)" >&2
fi

work=$(mktemp -d)
# Nix store paths are read-only (0444/0555), so a plain `rm -rf` on a chroot store
# fails — make it writable first.
rmrf() { chmod -R u+w "$1" 2>/dev/null || true; rm -rf "$1"; }
cleanup() { kill "${server_pid:-}" 2>/dev/null || true; if [ "$keep" = 1 ]; then echo "kept work dir: $work"; else rmrf "$work"; fi; }
trap cleanup EXIT

# Throttling static HTTP server fronting a file:// binary cache dir. An aggregate
# (link-level) bandwidth cap + per-GET latency, shared across all connections.
# Threaded, so many small-object GETs (symlink mode) get parallelism up to the link
# while a single big NAR (consolidated mode) saturates the same shared cap on one
# connection — faithfully reproducing why http-connections
# helps the SDK/pub tier but not the one consolidated Maven NAR.
cat > "$work/throttle_server.py" <<'PY'
import http.server, socketserver, sys, time, threading
ROOT, PORT, RATE, LAT = sys.argv[1], int(sys.argv[2]), float(sys.argv[3]) * 1024 * 1024, float(sys.argv[4]) / 1000.0
# Shared token bucket: caps AGGREGATE throughput across all connections to RATE,
# modelling a runner's total egress (not a per-stream cap). So more
# http-connections cut serial latency but never exceed the link, and one big NAR
# on a single connection saturates it exactly like many small paths do.
_lock = threading.Lock()
_st = {"allow": float(RATE), "last": time.monotonic()}
def throttle(n):
    if RATE <= 0: return
    with _lock:
        now = time.monotonic()
        _st["allow"] = min(RATE, _st["allow"] + (now - _st["last"]) * RATE)
        _st["last"] = now
        if _st["allow"] >= n:
            _st["allow"] -= n; wait = 0.0
        else:
            wait = (n - _st["allow"]) / RATE; _st["allow"] = 0.0; _st["last"] = now + wait
    if wait > 0: time.sleep(wait)
class H(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *a, **k): super().__init__(*a, directory=ROOT, **k)
    def log_message(self, *a): pass
    def copyfile(self, source, outputfile):
        if LAT > 0: time.sleep(LAT)               # per-object (GET) latency
        while True:
            b = source.read(65536)
            if not b: break
            throttle(len(b))                      # aggregate bandwidth cap
            outputfile.write(b)
class S(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True
S(("127.0.0.1", PORT), H).serve_forever()
PY

nar_human=$(numfmt --to=iec --suffix=B "$(nix path-info -rS "$store_path" | awk '{s+=$NF} END{print s}')" 2>/dev/null || echo "?")
echo "store path : $store_path"
echo "closure    : $nar_human   cpus=$cpus  rate=${rate}MB/s  latency=${latency}ms"
echo

# Push the closure into one file:// cache per compression (the "remote cache").
declare -A pushed
for comp in xz zstd; do
  case "$cells" in *"$comp:"*) ;; *) continue ;; esac
  cache="$work/cache-$comp"
  nix copy --to "file://$cache?compression=$comp" "$store_path" >/dev/null 2>&1
  pushed[$comp]=$cache
done

run_cell() {
  local comp=$1 mode=$2 cache=$3
  local hc mj db
  if [ "$mode" = tuned ]; then hc=128; mj=128; db=536870912; else hc=25; mj=16; db=67108864; fi
  # Fresh throwaway store: every path is a cold miss, pulled over the throttled link.
  local dest="$work/store-$comp-$mode"
  local port=$(( (RANDOM % 20000) + 20000 ))
  python3 "$work/throttle_server.py" "$cache" "$port" "$rate" "$latency" &
  server_pid=$!
  # Wait for the server to accept connections.
  for _ in $(seq 1 50); do (exec 3<>/dev/tcp/127.0.0.1/"$port") 2>/dev/null && { exec 3>&- 3<&-; break; }; sleep 0.1; done
  local t0 t1
  t0=$(date +%s.%N)
  "${taskset_pin[@]}" nix copy --no-check-sigs \
    --from "http://127.0.0.1:$port" --to "$dest" "$store_path" \
    --option http-connections "$hc" \
    --option max-substitution-jobs "$mj" \
    --option download-buffer-size "$db" >/dev/null 2>&1
  t1=$(date +%s.%N)
  kill "$server_pid" 2>/dev/null || true; wait "$server_pid" 2>/dev/null || true
  [ "$keep" = 1 ] || rmrf "$dest"   # bound scratch disk to ~1× closure, not 4×
  awk -v c="$comp" -v m="$mode" -v s="$t0" -v e="$t1" \
    'BEGIN{printf "  %-5s %-8s restore=%6.1fs\n", c, m, e-s}'
}

printf '%-5s %-8s %s\n' comp mode result
baseline=""; best=""
for cell in $cells; do
  comp=${cell%%:*}; mode=${cell##*:}
  [ -n "${pushed[$comp]:-}" ] || { echo "  (skip $cell: no $comp cache)"; continue; }
  line=$(run_cell "$comp" "$mode" "${pushed[$comp]}")
  echo "$line"
  secs=$(echo "$line" | grep -oE 'restore= *[0-9.]+' | grep -oE '[0-9.]+')
  [ "$cell" = "xz:default" ] && baseline=$secs
  [ "$cell" = "zstd:tuned" ] && best=$secs
done

if [ -n "$baseline" ] && [ -n "$best" ]; then
  awk -v b="$baseline" -v g="$best" 'BEGIN{ if(g>0) printf "\nspeedup (xz/default → zstd/tuned): %.2fx  (%.1fs → %.1fs)\n", b/g, b, g }'
fi
