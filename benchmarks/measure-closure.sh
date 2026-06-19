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
# Usage: measure-closure.sh <result-or-store-path> [label]
set -euo pipefail

if [ $# -lt 1 ]; then
  echo "usage: $0 <result-or-store-path> [label]" >&2
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

# Total closure size in bytes (-S = add closure size; -r = recurse the closure).
# Sum the per-path sizes so the number is the whole transitive closure, not just
# the top path.
total_bytes=$(nix path-info -rS "$target" | awk '{sum += $NF} END {print sum}')
path_count=$(nix path-info -r "$target" | wc -l | tr -d ' ')
human=$(numfmt --to=iec --suffix=B "$total_bytes" 2>/dev/null || echo "${total_bytes}B")

printf '%-16s closure=%s paths=%s (%s)\n' "$label" "$human" "$path_count" "$target"
