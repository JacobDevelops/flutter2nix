# Prebuilt binary release metadata consumed by flake.nix's *-bin packages.
# The download URL is derived, not stored:
#   https://github.com/JacobDevelops/flutter2nix/releases/download/v${version}/flutter2nix-v${version}-${system}.tar.gz
# Hashes are SRI (sha256). Refresh them after each release with scripts/update-prebuilt.sh;
# the AAA... placeholders are lib.fakeHash-style stand-ins that eval but never fetch.
{
  version = "0.2.0";
  artifacts = {
    x86_64-linux = { hash = "sha256-ytiLADTvZzhpagvYq1aRA3+gcEv+Y3DkPCcWsI/RvmA="; };
    aarch64-linux = { hash = "sha256-/WaQ+rZ0SKXfMm15b/rVm/C+eMI635CaRBXGK2EswMM="; };
    x86_64-darwin = { hash = "sha256-rNezRJO6ZSR5peer/8TCV79E2ru9+ZCj5q6E3RklkIo="; };
    aarch64-darwin = { hash = "sha256-dMTWk7DYy6x9dA47nnYBeJ617liKmKpEOFbkzGBPqus="; };
  };
}
