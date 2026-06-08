plugins {
    id("com.android.application") version "7.4.0" apply false
}

repositories {
    google()
    mavenCentral()
}

dependencies {
    testImplementation("junit:junit:4.13.2")
    implementation("com.google.guava:guava:31.1-jre")
}
