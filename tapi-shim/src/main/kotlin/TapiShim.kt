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

private val INIT_SCRIPT = """
gradle.projectsEvaluated {
    def seen = Collections.synchronizedSet(new HashSet())
    def allArtifacts = Collections.synchronizedList(new ArrayList())
    def gradleHomeDir = gradle.gradleUserHomeDir

    def collectTasks = rootProject.allprojects.collect { proj ->
        proj.task('flutter2nixDepsCollect') {
            doLast {
                proj.configurations.names.toList().each { configName ->
                    if (!configName.endsWith('RuntimeClasspath') && !configName.endsWith('CompileClasspath')) {
                        return
                    }
                    try {
                        def config = proj.configurations.getByName(configName)
                        if (config.canBeResolved) {
                            config.resolvedConfiguration.resolvedArtifacts.each { ra ->
                                def mv = ra.moduleVersion.id
                                def declaredExt = ra.extension ?: 'jar'
                                def realExt = declaredExt
                                if (declaredExt == 'jar') {
                                    def artifactDir = new File(gradleHomeDir, 'caches/modules-2/files-2.1/' + mv.group + '/' + mv.name + '/' + mv.version)
                                    if (artifactDir.exists()) {
                                        def aarName = mv.name + '-' + mv.version + '.aar'
                                        def hasAar = artifactDir.listFiles()?.any { hashDir ->
                                            hashDir.isDirectory() && new File(hashDir, aarName).exists()
                                        }
                                        if (hasAar) realExt = 'aar'
                                    }
                                }
                                def key = mv.group + ':' + mv.name + ':' + mv.version + ':' + (ra.classifier ?: '') + ':' + realExt
                                if (seen.add(key)) {
                                    allArtifacts.add([group: mv.group, artifact: mv.name, version: mv.version, classifier: ra.classifier, extension: realExt, scope: configName])
                                }
                            }
                        }
                    } catch (Exception ignored) {}
                }
            }
        }
    }

    rootProject.task('flutter2nixDeps') {
        dependsOn collectTasks
        doLast {
            println 'FLUTTER2NIX_DEPS:' + groovy.json.JsonOutput.toJson(allArtifacts)
        }
    }
}
""".trimIndent()

fun main(args: Array<String>) {
    val projectDir = File(if (args.isNotEmpty()) args[0] else ".")

    if (!projectDir.exists()) {
        System.err.println("Error: project directory does not exist: ${projectDir.absolutePath}")
        System.exit(1)
    }

    try {
        val connection = GradleConnector.newConnector()
            .forProjectDirectory(projectDir)
            .connect()

        connection.use { conn ->
            val buildEnv = conn.getModel(BuildEnvironment::class.java)
            val gradleVersion = buildEnv.gradle.gradleVersion

            val initArtifacts = tryInitScript(conn)

            if (initArtifacts != null && initArtifacts.isNotEmpty()) {
                outputSentinels(gradleVersion, initArtifacts)
                return
            }

            if (initArtifacts != null) {
                System.err.println("Init script produced zero artifacts; attempting IdeaProject fallback")
            }

            val ideaArtifacts = tryIdeaProject(conn)

            if (ideaArtifacts.isEmpty()) {
                System.err.println("Init-script: 0; IdeaProject: 0. Expected for pure-Java. Check Gradle setup if expecting Android dependencies.")
            }

            outputSentinels(gradleVersion, ideaArtifacts)
        }
    } catch (e: Exception) {
        System.err.println("TapiShim error: ${e.message}")
        e.printStackTrace(System.err)
        System.exit(1)
    }
}

private fun tryInitScript(conn: ProjectConnection): List<TapiArtifact>? {
    val tempFile = Files.createTempFile("flutter2nix-init-", ".gradle").toFile()
    return try {
        tempFile.writeText(INIT_SCRIPT)
        val stdout = ByteArrayOutputStream()
        conn.newBuild()
            .forTasks(":flutter2nixDeps")
            .withArguments("--init-script", tempFile.absolutePath, "--quiet", "--no-configuration-cache")
            .setStandardOutput(stdout)
            .setStandardError(System.err)
            .run()
        parseSentinelDeps(stdout.toString(Charsets.UTF_8))
    } catch (e: Exception) {
        System.err.println("Init script approach failed: ${e.message}")
        null
    } finally {
        tempFile.delete()
    }
}

private fun parseSentinelDeps(output: String): List<TapiArtifact> {
    val re = Regex("""^FLUTTER2NIX_DEPS:(.*)$""", RegexOption.MULTILINE)
    val match = re.find(output) ?: return emptyList()
    return try {
        lenientJson.decodeFromString<List<TapiArtifact>>(match.groupValues[1])
    } catch (e: Exception) {
        System.err.println("Failed to parse sentinel JSON: ${e.message}")
        emptyList()
    }
}

private fun tryIdeaProject(conn: ProjectConnection): List<TapiArtifact> {
    val ideaProject = conn.model(IdeaProject::class.java)
        .withArguments("--no-configuration-cache", "--quiet")
        .get()

    val seen = mutableSetOf<String>()
    val artifacts = mutableListOf<TapiArtifact>()

    ideaProject.modules.forEach moduleLoop@{ module ->
        module.dependencies.forEach depLoop@{ dep ->
            if (dep is IdeaSingleEntryLibraryDependency) {
                val mv = dep.gradleModuleVersion ?: return@depLoop
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
