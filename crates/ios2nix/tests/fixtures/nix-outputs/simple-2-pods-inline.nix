{ lib, fetchurl }:
{
  Flutter = fetchurl {
    url = "https://storage.googleapis.com/flutter_infra_release/releases/stable/ios/Flutter-1.0.0.zip";
    sha256 = "deadbeefcafebabe1234567890abcdef1234567890abcdef1234567890abcdef";
  };
  firebase_core = fetchurl {
    url = "https://github.com/firebase/firebase-ios-sdk/releases/download/10.0.0/firebase_core.zip";
    sha256 = "cafebabe1234567890abcdefdeadbeefcafebabe1234567890abcdef12345678";
  };
}
