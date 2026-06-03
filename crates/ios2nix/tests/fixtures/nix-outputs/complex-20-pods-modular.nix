{ lib, fetchurl }:
let
  mkPod = { name, url, sha256 }: fetchurl { inherit url sha256; };
in
{
  Flutter = mkPod {
    name = "Flutter";
    url = "https://storage.googleapis.com/flutter_infra_release/releases/stable/ios/Flutter-1.0.0.zip";
    sha256 = "deadbeefcafebabe1234567890abcdef1234567890abcdef1234567890abcdef";
  };
  firebase_core = mkPod {
    name = "firebase_core";
    url = "https://github.com/firebase/firebase-ios-sdk/releases/download/10.0.0/firebase_core.zip";
    sha256 = "cafebabe1234567890abcdefdeadbeefcafebabe1234567890abcdef12345678";
  };
  firebase_auth = mkPod {
    name = "firebase_auth";
    url = "https://github.com/firebase/firebase-ios-sdk/releases/download/10.0.0/firebase_auth.zip";
    sha256 = "1111111111111111111111111111111111111111111111111111111111111111";
  };
  # ... 17 more pods (stub — full list in complex-20-pods.lock)
}
