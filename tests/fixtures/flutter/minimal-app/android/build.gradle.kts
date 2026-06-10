allprojects {
    repositories {
        google()
        mavenCentral()
    }
}

// Match the Flutter template: redirect Gradle build output to <project>/build
// (one level above android/). flutter_tools looks for the .aab/.apk there.
val newBuildDir: Directory = rootProject.layout.buildDirectory.dir("../../build").get()
rootProject.layout.buildDirectory.value(newBuildDir)

subprojects {
    val newSubprojectBuildDir: Directory = newBuildDir.dir(project.name)
    project.layout.buildDirectory.value(newSubprojectBuildDir)
}
subprojects {
    project.evaluationDependsOn(":app")
}

tasks.register<Delete>("clean") {
    delete(rootProject.layout.buildDirectory)
}
