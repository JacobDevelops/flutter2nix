plugins {
    kotlin("jvm") version "1.9.24"
    kotlin("plugin.serialization") version "1.9.24"
    application
}

group = "dev.flutter2nix"
version = "0.1.0"

repositories {
    mavenCentral()
    maven {
        url = uri("https://repo.gradle.org/gradle/libs-releases")
    }
}

dependencies {
    implementation("org.gradle:gradle-tooling-api:9.4.1")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.6.3")
    // SLF4J binding required by gradle-tooling-api
    runtimeOnly("org.slf4j:slf4j-simple:2.0.9")
}

application {
    mainClass.set("TapiShimKt")
}

kotlin {
    jvmToolchain(17)
}

// Create fat JAR using built-in Gradle jar task
tasks.jar {
    from(sourceSets.main.get().output)
    from(configurations.runtimeClasspath.get().map { if (it.isDirectory) it else zipTree(it) })
    archiveFileName.set("tapi-shim.jar")
    duplicatesStrategy = DuplicatesStrategy.EXCLUDE
    manifest {
        attributes["Main-Class"] = "TapiShimKt"
    }
}
