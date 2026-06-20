#!/usr/bin/env bash
# measure-closure.sh — report the Nix closure size and store-path count of a
# build result. Use it to make the `consolidateMavenRepo` tradeoff data-driven:
# the symlink default produces a many-small-paths closure (good dedup +
# incremental transfer on warm builders), while consolidateMavenRepo=true
# produces a few-large-paths closure (one NAR, good for cold ephemeral runners).
# See docs/ci-cache-strategy.md for the full discussion.
#
# Closure size ~= the bytes a binary cache must store/transfer for this output;
# path count ~= the number of per-object cache GETs a cold runner pays. Restore
# time on a real cache backend depends on both — measure it there too (the doc
# shows `nix copy` timing against a throwaway store).
#
# Read-only: this script never builds, deletes, or garbage-collects anything. It
# only inspects the path you pass. Build the inputs yourself first, e.g.:
#
#   # symlink (default):
#   nix build .#<app> -o result-symlink
#   ./benchmarks/measure-closure.sh result-symlink symlink
#
#   # consolidated (set consolidateMavenRepo = true on the build, then):
#   nix build .#<app-consolidated> -o result-consolidated
#   ./benchmarks/measure-closure.sh result-consolidated consolidated
#
# Pass --compression to also measure the NAR's xz-vs-zstd compressed size and,
# more importantly, decompression wall-clock — the lever that decides cold-restore
# time when a single big NAR (consolidateMavenRepo=true) dominates. A binary cache
# stores each path as one compressed NAR; on a cold ephemeral runner that NAR is
# downloaded and decompressed on the critical path, single-threaded per path. xz
# wins on size, zstd wins (often several-fold) on decompression speed, so for the
# consolidated Maven repo zstd usually wins end-to-end CI even though the NAR is a
# little larger. This makes that tradeoff measurable instead of asserted.
#
# Usage: measure-closure.sh [--compression] <result-or-store-path> [label]
set -euo pipefail

do_compression=0
args=()
for a in "$@"; do
  case $a in
    --compression) do_compression=1 ;;
    *) args+=("$a") ;;
  esac
done
set -- "${args[@]}"

if [ $# -lt 1 ]; then
  echo "usage: $0 [--compression] <result-or-store-path> [label]" >&2
  exit 2
fi

target=$1
label=${2:-$(basename "$target")}

if ! command -v nix >/dev/null 2>&1; then
  echo "error: 'nix' not found on PATH" >&2
  exit 1
fi
if [ ! -e "$target" ]; then
  echo "error: '$target' does not exist (build it first)" >&2
  exit 1
fi

# Resolve to the real store path: `nix path-info` reads a bare name like
# `result` as a flake installable (`flake:result`), not a path, so follow the
# symlink to /nix/store/... first.
store_path=$(readlink -f "$target")

# Total closure size in bytes (-S = add closure size; -r = recurse the closure).
# Sum the per-path sizes so the number is the whole transitive closure, not just
# the top path.
total_bytes=$(nix path-info -rS "$store_path" | awk '{sum += $NF} END {print sum}')
path_count=$(nix path-info -r "$store_path" | wc -l | tr -d ' ')
human=$(numfmt --to=iec --suffix=B "$total_bytes" 2>/dev/null || echo "${total_bytes}B")

printf '%-16s closure=%s paths=%s (%s)\n' "$label" "$human" "$path_count" "$target"

if [ "$do_compression" = 1 ]; then
  # Dump just this path's NAR (the bytes a cache stores for one store path) and
  # time compression + the cold-restore-critical decompression for xz and zstd.
  # Levels match what Nix's codecs actually emit by default — xz -6 and zstd -3
  # (what `compression=zstd` on a cache produces) — so the sizes are the ones a
  # real cache would store. Threading also mirrors Nix's push: xz single-threaded
  # (Nix does not pass -T0 to liblzma), zstd -T0 (matching parallel-compression=
  # true). The decompress column is what matters for restore time.
  for tool in xz zstd; do
    command -v "$tool" >/dev/null 2>&1 || {
      echo "  (skip compression: '$tool' not on PATH)" >&2
      do_compression=0
    }
  done
fi

if [ "$do_compression" = 1 ]; then
  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' EXIT
  nix store dump-path "$store_path" > "$tmp/nar"
  nar_bytes=$(stat -c %s "$tmp/nar" 2>/dev/null || wc -c < "$tmp/nar")
  printf '  nar(uncompressed)=%s\n' "$(numfmt --to=iec --suffix=B "$nar_bytes" 2>/dev/null || echo "${nar_bytes}B")"
  # secs <start_ns> — elapsed seconds since a captured `date +%s.%N`, via awk so we
  # need no bc/coreutils float support.
  secs() { awk -v s="$1" -v e="$(date +%s.%N)" 'BEGIN{printf "%.1f", e-s}'; }
  # spec = tool:clevel:decompress-cmd:compress-threads-flag (empty = single-thread)
  for spec in "xz:-6:xz -d:" "zstd:-3:zstd -d:-T0"; do
    IFS=: read -r tool clevel decmd cthreads <<<"$spec"
    # shellcheck disable=SC2086  # $cthreads is intentionally a bare flag or empty
    t=$(date +%s.%N); "$tool" "$clevel" $cthreads -c "$tmp/nar" > "$tmp/nar.$tool" 2>/dev/null; ct=$(secs "$t")
    csize=$(stat -c %s "$tmp/nar.$tool" 2>/dev/null || wc -c < "$tmp/nar.$tool")
    t=$(date +%s.%N); $decmd -c "$tmp/nar.$tool" > /dev/null 2>&1; dt=$(secs "$t")
    ratio=$(awk -v n="$nar_bytes" -v c="$csize" 'BEGIN{printf "%.2f", (c>0)? n/c : 0}')
    printf '  %-5s comp=%s (%ss, %sx)  decomp=%ss\n' \
      "$tool" "$(numfmt --to=iec --suffix=B "$csize" 2>/dev/null || echo "${csize}B")" "$ct" "$ratio" "$dt"
  done
fi
