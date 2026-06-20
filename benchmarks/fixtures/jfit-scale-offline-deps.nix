# jfit-scale-offline-deps.nix — a synthetic offline-deps closure sized like a real
# Flutter app's, for benchmarking cold-CI restore with benchmarks/ci-restore-sim.sh.
#
# It is one store path (≈ consolidateMavenRepo=true shape) holding ~numArtifacts
# "JAR" files summing to ~totalMiB. Each file is DETERMINISTIC, INCOMPRESSIBLE
# bytes (an AES-CTR keystream over /dev/zero, seeded per file) — real Maven JARs
# and Dart pub tarballs are already deflate/gzip-compressed, so a binary cache's
# xz/zstd shrinks them almost not at all. Modelling that is the whole point: it is
# why jfit's ~5.6 GB closure transfers ~5.6 GB over the wire and why, for this
# shape, restore time is bandwidth-bound (and codec choice barely moves it). For a
# softer, more-compressible mix, drop `incompressibleFrac`.
#
# Build (uses the system <nixpkgs>; pass your own pkgs to pin):
#   nix build --impure -f benchmarks/fixtures/jfit-scale-offline-deps.nix -o result-deps
#   # smaller, faster fixture for a quick check:
#   nix build --impure --expr 'import ./benchmarks/fixtures/jfit-scale-offline-deps.nix { totalMiB = 1024; }' -o result-deps-small
#
# Then: ./benchmarks/ci-restore-sim.sh --store-path result-deps --rate 8 --cpus 8
{
  pkgs ? import <nixpkgs> { },
  totalMiB ? 5120, # ≈ 5 GiB, a real app's offline-deps closure
  numArtifacts ? 2900, # ≈ jfit's Maven path count
  incompressibleFrac ? 100, # 0..100; 100 = JAR-like (no headroom for xz/zstd)
}:
let
  bytesPer = totalMiB * 1024 * 1024 / numArtifacts;
  randPer = bytesPer * incompressibleFrac / 100;
  zeroPer = bytesPer - randPer; # compressible remainder (xz/zstd collapse it)
in
pkgs.runCommand "jfit-scale-offline-deps-${toString totalMiB}MiB"
  {
    nativeBuildInputs = [
      pkgs.openssl
      pkgs.coreutils
    ];
  }
  ''
    for i in $(seq 1 ${toString numArtifacts}); do
      d="$out/repo/g$((i % 64))/a$i/1.0"
      mkdir -p "$d"
      f="$d/a$i-1.0.jar"
      # CTR mode is a stream cipher: output length == input length, no padding and
      # no SIGPIPE (we feed exactly randPer zero-bytes). A distinct *key* per file
      # gives each artifact an INDEPENDENT keystream. Varying only the IV would not:
      # AES-CTR treats the IV as the initial counter block, so file i+1 (IV=i+1)
      # would be file i's keystream (IV=i) shifted by a single block — the files
      # overlap almost entirely and xz/zstd collapse that cross-file redundancy
      # (measured 4x+ on the concatenation), defeating the "incompressible" premise
      # the whole benchmark rests on. A per-file key removes the overlap, so the
      # concatenated NAR stays incompressible (~1.00x) while every file is still
      # fully deterministic across rebuilds.
      key=$(printf '%064x' "$i")
      head -c ${toString randPer} /dev/zero \
        | openssl enc -aes-256-ctr -nosalt \
            -K "$key" -iv 00000000000000000000000000000000 2>/dev/null > "$f"
      ${pkgs.lib.optionalString (zeroPer > 0) ''head -c ${toString zeroPer} /dev/zero >> "$f"''}
    done
  ''
