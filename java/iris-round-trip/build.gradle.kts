plugins { id("iris.java-conventions") }

dependencies { implementation(project(":iris")) }

// The builder programs that rust/gecode-iris/tests/builder_round_trip_java.rs generates, copied
// into the build directory, since Spotless checks only files inside the project. They join the
// runner in one compilation, with the build's flags, Error Prone and NullAway, and Spotless checks
// them with the build's google-java-format.
val sources = providers.gradleProperty("irisRoundTripSources")
val programs = layout.buildDirectory.dir("programs")

val copyPrograms =
    tasks.register<Sync>("copyPrograms") {
      from(sources)
      into(programs)
    }

if (sources.isPresent) {
  sourceSets { main { java { srcDir(copyPrograms) } } }
}

tasks.register<JavaExec>("roundTrip") {
  description = "Runs every generated builder program and records the spec it prints."
  dependsOn("spotlessCheck")
  classpath = sourceSets.main.get().runtimeClasspath
  mainClass = "dev.gecode.iris.roundtrip.Main"
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
