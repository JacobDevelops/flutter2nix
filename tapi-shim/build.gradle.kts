plugins {
    kotlin("jvm") version "1.9.24"
    application
}

group = "dev.flutter2nix"
version = "0.1.0"

repositories {
    mavenCentral()
}

dependencies {
    // Phase 1: add org.gradle:gradle-tooling-api dependency here
}

application {
    mainClass.set("TapiShimKt")
}

java {
    sourceCompatibility = JavaVersion.VERSION_11
    targetCompatibility = JavaVersion.VERSION_11
}
