plugins {
    id("com.android.application")
    id("kotlin-android")
}

android {
    namespace = "dev.flutter2nix.minimal"
    compileSdk = 34

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }

    kotlinOptions {
        jvmTarget = "1.8"
    }

    defaultConfig {
        applicationId = "dev.flutter2nix.minimal"
        minSdk = 21
        targetSdk = 34
        versionCode = 1
        versionName = "1.0"
    }

    buildTypes {
        release {
            signingConfig = signingConfigs.getByName("debug")
        }
    }

    lint {
        // lintVitalRelease resolves com.android.tools.lint:lint-gradle at task
        // execution time via a detached configuration — unavailable offline and
        // out of scope for the offline-build E2E.
        checkReleaseBuilds = false
    }
}
