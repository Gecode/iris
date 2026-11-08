import org.jetbrains.kotlin.gradle.dsl.ExplicitApiMode

plugins { id("iris.kotlin-conventions") }

dependencies {
    implementation(project(":iris"))
    implementation(project(":iris-kotlin"))
    // Jobshop.java names its reader spec with @IrisSpec, and the processor writes its handles.
    compileOnly(project(":iris-processor"))
    annotationProcessor(project(":iris-processor"))
    // The format builders answer the conformance runner through the driver.
    testImplementation(project(":iris-driver"))
}

// The examples are programs: a visibility modifier on each main would only add noise on the page.
kotlin { explicitApi = ExplicitApiMode.Disabled }

val repository = rootDir.parentFile

// The examples run from the repository root, so @IrisSpec names its file relative to it.
tasks.compileJava {
    options.compilerArgs.add("-Airis.root=${repository.path}")
    // The processor leaves @IrisSpec unclaimed, so javac would warn that no processor claimed it.
    options.compilerArgs.add("-Xlint:-processing")
    inputs
        .file(repository.resolve("formats/jobshop/jobshop.iris"))
        .withPathSensitivity(PathSensitivity.RELATIVE)
}

tasks.test {
    val examples = sourceSets.main.get().runtimeClasspath
    jvmArgumentProviders.add(
        CommandLineArgumentProvider { listOf("-Diris.examplesClasspath=${examples.asPath}") }
    )
    // FormatBuildersTest starts FormatBuilderDriver, a test class, in a JVM of its own.
    val tests = sourceSets.test.get().runtimeClasspath
    jvmArgumentProviders.add(
        CommandLineArgumentProvider { listOf("-Diris.testClasspath=${tests.asPath}") }
    )
    systemProperty("iris.repository", repository.path)
    systemProperty("iris.examples", layout.projectDirectory.dir("src/main").asFile.path)
    // The .out recordings sit beside the sources, and the examples read the format library.
    inputs.dir("src/main").withPathSensitivity(PathSensitivity.RELATIVE)
    inputs.dir(repository.resolve("formats")).withPathSensitivity(PathSensitivity.RELATIVE)
    // FormatBuildersTest runs the conformance runner from the Rust workspace.
    inputs
        .dir(repository.resolve("rust/iris-conformance"))
        .withPathSensitivity(PathSensitivity.RELATIVE)
    // The test checks that the guide shows exactly these examples.
    inputs
        .file(repository.resolve("site/src/content/docs/using/java.mdx"))
        .withPathSensitivity(PathSensitivity.RELATIVE)
}
