import org.gradle.tooling.GradleConnector
import org.gradle.tooling.ProjectConnection
import org.gradle.tooling.model.idea.IdeaProject
import org.gradle.tooling.model.idea.IdeaSingleEntryLibraryDependency
import org.gradle.tooling.model.build.BuildEnvironment
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import java.io.ByteArrayOutputStream
import java.io.File
import java.nio.file.Files

@Serializable
data class TapiArtifact(
    val group: String,
    val artifact: String,
    val version: String,
    val classifier: String?,
    val extension: String,
    val scope: String,
)

private val lenientJson = Json { ignoreUnknownKeys = true }

// The Gradle init script lives in src/main/resources so it gets Groovy syntax
// highlighting and is maintainable as a standalone file. It is bundled into the
// fat JAR by the jar task (sourceSets.main.output includes processed resources).
private val INIT_SCRIPT: String =
    object {}.javaClass.getResource("/flutter2nix-init.gradle")!!.readText()

private val shimStart = System.currentTimeMillis()

/// Stage timing on stderr: attributes shim wall-clock to distro install /
/// configuration / dependency resolution without needing --info noise.
private fun mark(label: String) {
    System.err.println("TAPI-TIMING: $label +${System.currentTimeMillis() - shimStart}ms")
}

fun main(args: Array<String>) {
    val projectDir = File(if (args.isNotEmpty()) args[0] else ".")

    if (!projectDir.exists()) {
        System.err.println("Error: project directory does not exist: ${projectDir.absolutePath}")
        System.exit(1)
    }

    // --init-script (passed via TAPI withArguments) only applies to the top-level build.
    // Plugin builds loaded via pluginManagement { includeBuild } use a separate Gradle
    // instance and never see --init-script. Files in $GRADLE_USER_HOME/init.d/ ARE applied
    // to all builds in the composite including plugin builds.
    // We create a temp home that symlinks the real caches/wrapper (so existing artifacts and
    // distributions are reused) and writes our init script into its init.d/.
    val realGradleHome = File(
        System.getenv("GRADLE_USER_HOME")
            ?: (System.getProperty("user.home") + "/.gradle")
    )
    val tempGradleHome = Files.createTempDirectory("flutter2nix-gradle-home-").toFile()
    val kgpPersistentDir = Files.createTempDirectory("flutter2nix-kgp-").toFile()

    var exitCode = 0
    try {
        // Share the Gradle distribution and Maven artifact cache with the real home.
        // The dirs are created if missing: on a cold home, everything Gradle downloads
        // flows through the symlinks into the real home and survives this run —
        // otherwise a cold start downloads ~1GB into the temp home and deletes it,
        // and the next run pays the entire download again.
        for (sharedDir in listOf("caches", "wrapper")) {
            val target = File(realGradleHome, sharedDir)
            Files.createDirectories(target.toPath())
            Files.createSymbolicLink(
                File(tempGradleHome, sharedDir).toPath(),
                target.toPath()
            )
        }
        val initDir = File(tempGradleHome, "init.d")
        initDir.mkdirs()
        File(initDir, "flutter2nix.gradle").writeText(INIT_SCRIPT)

        // Write kotlin.project.persistent.dir to gradle.properties so that all builds
        // in the composite (including flutter_tools) read the writable temp path via
        // providers.gradleProperty() even when the init script's beforeSettings hook
        // fires after GradleProperties has been snapshotted.
        File(tempGradleHome, "gradle.properties").writeText(
            "kotlin.project.persistent.dir=${kgpPersistentDir.absolutePath}\n" +
            // Run Kotlin compiler in-process (no separate daemon) to avoid "daemon has
            // terminated unexpectedly" failures in Nix sandbox and dev-shell environments
            // where the Kotlin compile daemon exits immediately after saying "ready".
            // KGP accepts only lowercase hyphenated values: daemon | in-process | out-of-process.
            "kotlin.compiler.execution.strategy=in-process\n"
        )

        val connection = GradleConnector.newConnector()
            .forProjectDirectory(projectDir)
            .useGradleUserHomeDir(tempGradleHome)
            .connect()

        connection.use { conn ->
            mark("connected")
            val buildEnv = conn.getModel(BuildEnvironment::class.java)
            val gradleVersion = buildEnv.gradle.gradleVersion
            mark("distro ready + daemon up (BuildEnvironment)")

            val initArtifacts = tryInitScript(conn, kgpPersistentDir)
            mark("init-script build finished")

            if (initArtifacts != null && initArtifacts.isNotEmpty()) {
                outputSentinels(gradleVersion, initArtifacts)
                return
            }

            if (initArtifacts != null) {
                System.err.println("Init script produced zero artifacts; attempting IdeaProject fallback")
            }

            val ideaArtifacts = tryIdeaProject(conn, kgpPersistentDir)

            if (ideaArtifacts.isEmpty()) {
                System.err.println("Init-script: 0; IdeaProject: 0. Expected for pure-Java. Check Gradle setup if expecting Android dependencies.")
            }

            outputSentinels(gradleVersion, ideaArtifacts)
        }
    } catch (e: Exception) {
        System.err.println("TapiShim error: ${e.message}")
        e.printStackTrace(System.err)
        // exitProcess would skip the finally block; record the failure and exit after cleanup.
        exitCode = 1
    } finally {
        // Delete tempGradleHome without following symlinks: it contains a `caches` symlink
        // pointing to the real ~/.gradle/caches, and File.deleteRecursively() follows symlinks,
        // which would wipe the user's Gradle module cache. Per-entry runCatching: one
        // undeletable file (e.g. still held by a daemon) must not abort the rest.
        runCatching {
            Files.walk(tempGradleHome.toPath())
                .sorted(Comparator.reverseOrder())
                .forEach { runCatching { Files.delete(it) } }
        }
        kgpPersistentDir.deleteRecursively()
    }
    if (exitCode != 0) {
        kotlin.system.exitProcess(exitCode)
    }
}

private fun tryInitScript(conn: ProjectConnection, kgpPersistentDir: File): List<TapiArtifact>? {
    val projectCacheDir = Files.createTempDirectory("flutter2nix-gradle-cache-").toFile()
    return try {
        val stdout = ByteArrayOutputStream()
        conn.newBuild()
            .forTasks(":flutter2nixDeps")
            .withArguments(
                "--quiet",
                "--no-configuration-cache",
                "--project-cache-dir", projectCacheDir.absolutePath,
                // Belt-and-suspenders: -P propagates to all included builds and sets the
                // property via startParameter before GradleProperties is snapshotted.
                "-Pkotlin.project.persistent.dir=${kgpPersistentDir.absolutePath}",
            )
            .setStandardOutput(stdout)
            .setStandardError(System.err)
            .run()
        parseSentinelDeps(stdout.toString(Charsets.UTF_8))
    } catch (e: Exception) {
        System.err.println("Init script approach failed: ${e.message}")
        null
    } finally {
        projectCacheDir.deleteRecursively()
    }
}

private fun parseSentinelDeps(output: String): List<TapiArtifact> {
    val re = Regex("""^FLUTTER2NIX_DEPS:(.*)$""", RegexOption.MULTILINE)
    val seen = mutableSetOf<String>()
    val merged = mutableListOf<TapiArtifact>()
    for (match in re.findAll(output)) {
        try {
            val artifacts = lenientJson.decodeFromString<List<TapiArtifact>>(match.groupValues[1])
            for (a in artifacts) {
                val key = "${a.group}:${a.artifact}:${a.version}:${a.classifier}:${a.extension}"
                if (seen.add(key)) merged.add(a)
            }
        } catch (e: Exception) {
            System.err.println("Failed to parse sentinel JSON: ${e.message}")
        }
    }
    return merged
}

private fun tryIdeaProject(conn: ProjectConnection, kgpPersistentDir: File): List<TapiArtifact> {
    val ideaProject = conn.model(IdeaProject::class.java)
        .withArguments("--no-configuration-cache", "--quiet",
            "-Pkotlin.project.persistent.dir=${kgpPersistentDir.absolutePath}")
        .get()

    val seen = mutableSetOf<String>()
    val artifacts = mutableListOf<TapiArtifact>()

    ideaProject.modules.forEach moduleLoop@{ module ->
        module.dependencies.forEach depLoop@{ dep ->
            if (dep is IdeaSingleEntryLibraryDependency) {
                val mv = dep.gradleModuleVersion ?: return@depLoop
                if (mv.version.isNullOrEmpty() || mv.version == "unspecified") return@depLoop
                val file = dep.file ?: return@depLoop

                val extension = file.extension.ifEmpty { "jar" }
                val classifier = extractClassifier(file.nameWithoutExtension, mv.name, mv.version)
                val scope = dep.scope?.scope?.lowercase() ?: "compile"

                val key = "${mv.group}:${mv.name}:${mv.version}:${classifier}:${extension}"
                if (seen.add(key)) {
                    artifacts.add(
                        TapiArtifact(
                            group = mv.group,
                            artifact = mv.name,
                            version = mv.version,
                            classifier = classifier,
                            extension = extension,
                            scope = scope,
                        )
                    )
                }
            }
        }
    }

    return artifacts.sortedWith(compareBy({ it.group }, { it.artifact }, { it.version }))
}

private fun outputSentinels(gradleVersion: String, artifacts: List<TapiArtifact>) {
    println("FLUTTER2NIX_VERSION:$gradleVersion")
    println("FLUTTER2NIX_DEPS:${Json.encodeToString(artifacts)}")
}

fun extractClassifier(nameWithoutExt: String, artifactId: String, version: String): String? {
    val prefix = "$artifactId-$version-"
    return if (nameWithoutExt.startsWith(prefix)) {
        nameWithoutExt.removePrefix(prefix).ifEmpty { null }
    } else null
}
