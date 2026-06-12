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
  camera_avfoundation = mkPod {
    name = "camera_avfoundation";
    url = "https://github.com/flutter/plugins/releases/download/0.9.15/camera_avfoundation.zip";
    sha256 = "8888888888888888888888888888888888888888888888888888888888888888";
  };
  connectivity_plus = mkPod {
    name = "connectivity_plus";
    url = "https://github.com/flutter/plugins/releases/download/1.2.0/connectivity_plus.zip";
    sha256 = "aaaa0000aaaa0000aaaa0000aaaa0000aaaa0000aaaa0000aaaa0000aaaa0000";
  };
  device_info_plus = mkPod {
    name = "device_info_plus";
    url = "https://github.com/flutter/plugins/releases/download/9.1.0/device_info_plus.zip";
    sha256 = "cccc2222cccc2222cccc2222cccc2222cccc2222cccc2222cccc2222cccc2222";
  };
  file_picker = mkPod {
    name = "file_picker";
    url = "https://github.com/flutter/plugins/releases/download/6.1.0/file_picker.zip";
    sha256 = "1111777711117777111177771111777711117777111177771111777711117777";
  };
  firebase_auth = mkPod {
    name = "firebase_auth";
    url = "https://github.com/firebase/firebase-ios-sdk/releases/download/10.0.0/firebase_auth.zip";
    sha256 = "1111111111111111111111111111111111111111111111111111111111111111";
  };
  firebase_core = mkPod {
    name = "firebase_core";
    url = "https://github.com/firebase/firebase-ios-sdk/releases/download/10.0.0/firebase_core.zip";
    sha256 = "cafebabe1234567890abcdefdeadbeefcafebabe1234567890abcdef12345678";
  };
  firebase_firestore = mkPod {
    name = "firebase_firestore";
    url = "https://github.com/firebase/firebase-ios-sdk/releases/download/4.0.0/firebase_firestore.zip";
    sha256 = "2222222222222222222222222222222222222222222222222222222222222222";
  };
  firebase_storage = mkPod {
    name = "firebase_storage";
    url = "https://github.com/firebase/firebase-ios-sdk/releases/download/11.0.0/firebase_storage.zip";
    sha256 = "3333333333333333333333333333333333333333333333333333333333333333";
  };
  google_sign_in_ios = mkPod {
    name = "google_sign_in_ios";
    url = "https://github.com/flutter/plugins/releases/download/6.0.0/google_sign_in_ios.zip";
    sha256 = "4444444444444444444444444444444444444444444444444444444444444444";
  };
  image_picker_ios = mkPod {
    name = "image_picker_ios";
    url = "https://github.com/flutter/plugins/releases/download/0.8.9/image_picker_ios.zip";
    sha256 = "7777777777777777777777777777777777777777777777777777777777777777";
  };
  in_app_purchase_storekit = mkPod {
    name = "in_app_purchase_storekit";
    url = "https://github.com/flutter/plugins/releases/download/0.3.6/in_app_purchase_storekit.zip";
    sha256 = "2222888822228888222288882222888822228888222288882222888822228888";
  };
  local_auth_darwin = mkPod {
    name = "local_auth_darwin";
    url = "https://github.com/flutter/plugins/releases/download/2.2.0/local_auth_darwin.zip";
    sha256 = "ffff5555ffff5555ffff5555ffff5555ffff5555ffff5555ffff5555ffff5555";
  };
  package_info_plus = mkPod {
    name = "package_info_plus";
    url = "https://github.com/flutter/plugins/releases/download/7.0.0/package_info_plus.zip";
    sha256 = "dddd3333dddd3333dddd3333dddd3333dddd3333dddd3333dddd3333dddd3333";
  };
  path_provider_foundation = mkPod {
    name = "path_provider_foundation";
    url = "https://github.com/flutter/plugins/releases/download/2.3.0/path_provider_foundation.zip";
    sha256 = "5555555555555555555555555555555555555555555555555555555555555555";
  };
  permission_handler_apple = mkPod {
    name = "permission_handler_apple";
    url = "https://github.com/flutter/plugins/releases/download/9.2.0/permission_handler_apple.zip";
    sha256 = "eeee4444eeee4444eeee4444eeee4444eeee4444eeee4444eeee4444eeee4444";
  };
  shared_preferences_foundation = mkPod {
    name = "shared_preferences_foundation";
    url = "https://github.com/flutter/plugins/releases/download/2.3.0/shared_preferences_foundation.zip";
    sha256 = "6666666666666666666666666666666666666666666666666666666666666666";
  };
  sqflite_darwin = mkPod {
    name = "sqflite_darwin";
    url = "https://github.com/flutter/plugins/releases/download/2.3.0/sqflite_darwin.zip";
    sha256 = "0000666600006666000066660000666600006666000066660000666600006666";
  };
  url_launcher_ios = mkPod {
    name = "url_launcher_ios";
    url = "https://github.com/flutter/plugins/releases/download/6.2.0/url_launcher_ios.zip";
    sha256 = "bbbb1111bbbb1111bbbb1111bbbb1111bbbb1111bbbb1111bbbb1111bbbb1111";
  };
  video_player_avfoundation = mkPod {
    name = "video_player_avfoundation";
    url = "https://github.com/flutter/plugins/releases/download/2.3.0/video_player_avfoundation.zip";
    sha256 = "9999999999999999999999999999999999999999999999999999999999999999";
  };
}
