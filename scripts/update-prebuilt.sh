#!/usr/bin/env bash
# Refresh nix/prebuilt.nix hashes (and version) from a published release's
# checksums.txt. Usage: scripts/update-prebuilt.sh [vX.Y.Z]
# Default tag is v<version> read from nix/prebuilt.nix.
set -euo pipefail

repo="JacobDevelops/flutter2nix"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
prebuilt="$root/nix/prebuilt.nix"

current_version() {
  sed -nE 's/^[[:space:]]*version[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p' "$prebuilt" | head -n1
}

tag="${1:-v$(current_version)}"
version="${tag#v}"

tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT

url="https://github.com/${repo}/releases/download/${tag}/checksums.txt"
echo "Fetching $url" >&2
curl -fsSL "$url" -o "$tmp"

sed -i -E "s/^([[:space:]]*version[[:space:]]*=[[:space:]]*\")[^\"]*/\1${version}/" "$prebuilt"

for sys in x86_64-linux aarch64-linux x86_64-darwin aarch64-darwin; do
  archive="flutter2nix-${tag}-${sys}.tar.gz"
  hex="$(awk -v f="$archive" '{ n = $2; sub(/^\.\//, "", n) } n == f { print $1 }' "$tmp")"
  if [ -z "$hex" ]; then
    echo "error: no checksum for $archive in checksums.txt" >&2
    exit 1
  fi
  sri="$(nix hash to-sri --type sha256 "$hex")"
  sed -i -E "s|(${sys} = \{ hash = \")[^\"]*|\1${sri}|" "$prebuilt"
done

echo "Updated $prebuilt to $tag" >&2
