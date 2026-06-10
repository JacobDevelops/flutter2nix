pluginManagement {
    repositories {
        google()
        mavenCentral()
    }
}

plugins {
    id("com.android.application") version "8.6.0" apply false
    id("org.jetbrains.kotlin.android") version "2.1.0" apply false
}

include(":app")
