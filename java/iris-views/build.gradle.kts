import java.io.ByteArrayOutputStream
import javax.inject.Inject

plugins { id("iris.kotlin-conventions") }

// The views that `iris gen --lang java` and `--lang kotlin` print for every format, for the specs
// in
// specs/ and for a sweep of declaration widths, compiled with the build's flags, Error Prone,
// NullAway, detekt and the style checks; and the classes that iris-processor generates for the same
// specs. Each package that receives generated code declares classes named after the platform types
// the code uses, which it must not resolve to. The tests read every case through both and compare
// what each handle gives with untyped access.
dependencies {
    implementation(project(":iris-kotlin"))
    compileOnly(project(":iris-processor"))
    annotationProcessor(project(":iris-processor"))
}

val repository: File = rootDir.parentFile
val generated = layout.buildDirectory.dir("generated/views")

// The CLI that prints the views, built with the Rust workspace's lock file.
val buildCli =
    tasks.register<Exec>("buildCli") {
        description = "Builds the iris CLI that prints the views."
        workingDir = repository.resolve("rust")
        commandLine(
            "cargo",
            "build",
            "--quiet",
            "--locked",
            "-p",
            "gecode-iris-cli",
            "--bin",
            "iris",
        )
    }

val cargoTarget =
    providers
        .environmentVariable("CARGO_TARGET_DIR")
        .map(::File)
        .orElse(provider { repository.resolve("rust/target") })
val executable = if (System.getProperty("os.name").startsWith("Windows")) "iris.exe" else "iris"

// Names of every length around the widths where a declaration changes layout, for kinds of handle
// from the narrowest to the widest: a single integer, a float at depth 3, an optional string, an
// optional number deeper than the deepest view type, and an entry without one shape.
val longestName = 130

val writeWidths =
    tasks.register("writeWidths") {
        description = "Writes the spec and input of the declaration width sweep."
        val directory = layout.buildDirectory.dir("specs")
        outputs.dir(directory)
        doLast {
            val spec = StringBuilder("c: int\n")
            for (length in 1..longestName) {
                val name = "n".repeat(length)
                spec.append("${name}_i: int\n")
                spec.append("${name}_f[1][1][1]: float\n")
                spec.append("if c {\n  ${name}_s: word\n}\n")
                spec.append("if c {\n  ${name}_d[1][1][1][1][1][1][1][1]: num\n}\n")
                spec.append("${name}_x: int\nlet ${name}_x = \"s\"\n")
            }
            val output = directory.get().asFile
            output.mkdirs()
            output.resolve("widths.iris").writeText(spec.toString())
            output
                .resolve("widths.input")
                .writeText("1 " + "1 1.5 w 1.5 1 ".repeat(longestName) + "\n")
        }
    }

/** Prints the Java and Kotlin view of each spec into a package, and lists the views in order. */
abstract class GenerateViews @Inject constructor(private val exec: ExecOperations) : DefaultTask() {
    @get:InputFile @get:PathSensitive(PathSensitivity.NONE) abstract val cli: RegularFileProperty

    @get:InputFiles
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val specs: ConfigurableFileCollection

    @get:OutputDirectory abstract val output: DirectoryProperty

    @TaskAction
    fun generate() {
        val root = output.get().asFile
        root.deleteRecursively()
        val manifest = StringBuilder()
        for (spec in specs.files.sortedBy { it.path }) {
            val java = print(spec, "java")
            val name = Regex("public final class (\\w+) \\{").find(java)!!.groupValues[1]
            write(
                root.resolve("java/dev/gecode/iris/views/java/$name.java"),
                java,
                "package dev.gecode.iris.views.java;",
            )
            write(
                root.resolve("kotlin/dev/gecode/iris/views/kotlin/$name.kt"),
                print(spec, "kotlin"),
                "package dev.gecode.iris.views.kotlin",
            )
            manifest.append(name).append('\t').append(spec.path).append('\n')
        }
        root
            .resolve("resources/views.tsv")
            .apply { parentFile.mkdirs() }
            .writeText(manifest.toString())
    }

    private fun print(spec: File, language: String): String {
        val text = ByteArrayOutputStream()
        exec.exec {
            commandLine(cli.get().asFile.path, "gen", "--lang", language, spec.path)
            standardOutput = text
        }
        return text.toString(Charsets.UTF_8)
    }

    // The package goes after the generated header, the view's first line, as a user would add it.
    private fun write(file: File, view: String, packageLine: String) {
        val header = view.substringBefore('\n')
        file.parentFile.mkdirs()
        file.writeText(header + "\n" + packageLine + "\n" + view.substringAfter('\n'))
    }
}

val generateViews =
    tasks.register<GenerateViews>("generateViews") {
        description = "Prints the Java and Kotlin views of every format and test spec."
        dependsOn(buildCli)
        cli = cargoTarget.map { RegularFile { it.resolve("debug/$executable") } }
        specs.from(fileTree(repository.resolve("formats")) { include("*/*.iris") })
        specs.from(fileTree("specs") { include("*.iris") })
        specs.from(writeWidths.map { fileTree(it.outputs.files.singleFile) { include("*.iris") } })
        output = generated
    }

sourceSets {
    main {
        java.srcDir(generateViews.map { it.output.dir("java") })
        resources.srcDir(generateViews.map { it.output.dir("resources") })
    }
}

kotlin { sourceSets.main { kotlin.srcDir(generateViews.map { it.output.dir("kotlin") }) } }

detekt { config.from(file("detekt.yml")) }

// The annotated types of src/main/java name their reader specs relative to the repository, and the
// processor reads them at compile time.
tasks.compileJava {
    options.compilerArgs.add("-Airis.root=${repository.path}")
    // The processor leaves @IrisSpec unclaimed, so javac would warn that no processor claimed it.
    options.compilerArgs.add("-Xlint:-processing")
    inputs.files(generateViews.map { it.specs }).withPathSensitivity(PathSensitivity.RELATIVE)
}

tasks.test {
    systemProperty("iris.repository", repository.path)
    inputs.dir(repository.resolve("formats")).withPathSensitivity(PathSensitivity.RELATIVE)
    inputs.dir("specs").withPathSensitivity(PathSensitivity.RELATIVE)
}
