plugins {
    id("com.android.library")
}

android {
    namespace = "dev.flutter2nix.test"
    compileSdk = 34
    defaultConfig {
        minSdk = 21
    }
}

dependencies {
    implementation("androidx.core:core-ktx:1.12.0")
    implementation("com.google.guava:guava:32.1.3-android")
}
