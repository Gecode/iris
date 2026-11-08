import org.jetbrains.kotlin.gradle.dsl.ExplicitApiMode

plugins { id("iris.kotlin-conventions") }

dependencies { implementation(project(":iris-kotlin")) }

// The generated programs are what a user writes: a visibility modifier on each function would only
// add noise.
kotlin { explicitApi = ExplicitApiMode.Disabled }

// The builder programs that rust/gecode-iris/tests/builder_round_trip_kotlin.rs generates, copied
// into the build directory, since Spotless checks only files inside the project. They join the
// runner in one compilation, with the build's flags and warnings as errors, Spotless checks them
// with the build's ktfmt, and detekt checks them with the build's rules and the exceptions for
// generated code in this module's detekt.yml.
val sources = providers.gradleProperty("irisRoundTripSources")
val programs = layout.buildDirectory.dir("programs")

val copyPrograms =
    tasks.register<Sync>("copyPrograms") {
        from(sources)
        into(programs)
    }

detekt { config.from(file("detekt.yml")) }

if (sources.isPresent) {
    kotlin { sourceSets.main { kotlin.srcDir(copyPrograms) } }
}

tasks.register<JavaExec>("roundTrip") {
    description = "Runs every generated builder program and records the spec it prints."
    dependsOn("spotlessCheck", "detektMain")
    classpath = sourceSets.main.get().runtimeClasspath
    mainClass = "dev.gecode.iris.roundtrip.kotlin.MainKt"
    val out = providers.gradleProperty("irisRoundTripOut")
    doFirst {
        require(sources.isPresent && out.isPresent) {
            "set -PirisRoundTripSources=DIR and -PirisRoundTripOut=DIR"
        }
    }
    argumentProviders.add(
        CommandLineArgumentProvider { listOf(programs.get().asFile.path, out.get()) }
    )
}
