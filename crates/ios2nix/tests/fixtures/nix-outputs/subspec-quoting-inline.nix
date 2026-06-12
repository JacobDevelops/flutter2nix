{ lib, fetchurl }:
{
  "Firebase/CoreOnly" = fetchurl {
    url = "https://github.com/firebase/firebase-ios-sdk/releases/download/10.0.0/Firebase-CoreOnly.zip";
    sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
  };
  "GTMSessionFetcher/Core" = fetchurl {
    url = "https://github.com/google/gtm-session-fetcher/releases/download/3.1.0/GTMSessionFetcher-Core.zip";
    sha256 = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
  };
  PINCache = fetchurl {
    url = "https://github.com/pinterest/PINCache/releases/download/3.0.3/PINCache.zip";
    sha256 = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
  };
  "nanopb-2.30908.0" = fetchurl {
    url = "https://github.com/nanopb/nanopb/releases/download/2.30908.0/nanopb.zip";
    sha256 = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
  };
}
