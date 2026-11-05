import org.jetbrains.kotlin.gradle.dsl.ExplicitApiMode

plugins { id("iris.kotlin-conventions") }

dependencies {
    implementation(project(":iris"))
    implementation(project(":iris-kotlin"))
}

// The examples are programs: a visibility modifier on each main would only add noise on the page.
kotlin { explicitApi = ExplicitApiMode.Disabled }

val repository = rootDir.parentFile

tasks.test {
    val examples = sourceSets.main.get().runtimeClasspath
    jvmArgumentProviders.add(
        CommandLineArgumentProvider { listOf("-Diris.examplesClasspath=${examples.asPath}") }
    )
    systemProperty("iris.repository", repository.path)
    systemProperty("iris.examples", layout.projectDirectory.dir("src/main").asFile.path)
    // The .out recordings sit beside the sources, and the examples read the format library.
    inputs.dir("src/main").withPathSensitivity(PathSensitivity.RELATIVE)
    inputs.dir(repository.resolve("formats")).withPathSensitivity(PathSensitivity.RELATIVE)
    // The test checks that the guide shows exactly these examples.
    inputs
        .file(repository.resolve("site/src/content/docs/using/java.mdx"))
        .withPathSensitivity(PathSensitivity.RELATIVE)
}
