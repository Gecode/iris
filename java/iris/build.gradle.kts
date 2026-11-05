plugins {
  `java-library`
  id("iris.java-conventions")
}

dependencies { api(libs.jspecify) }

val repository = rootDir.parentFile
val installedDriver = project(":iris-driver").layout.buildDirectory.dir("install/iris-driver")

tasks.withType<Test>().configureEach {
  dependsOn(":iris-driver:installDist")
  systemProperty("iris.driver", installedDriver.get().file("bin/iris-driver").asFile.path)
  systemProperty("iris.repository", repository.path)
  providers.gradleProperty("irisFloatValues").orNull?.let { systemProperty("iris.floatValues", it) }
  providers.gradleProperty("irisCli").orNull?.let { systemProperty("iris.cli", it) }
  inputs.dir(installedDriver).withPathSensitivity(PathSensitivity.RELATIVE)
  for (path in listOf("conformance/cases/0.1", "formats", "rust/iris-conformance")) {
    inputs.dir(repository.resolve(path)).withPathSensitivity(PathSensitivity.RELATIVE)
  }
}

val conformanceTests = "dev.gecode.iris.ConformanceTest"

tasks.test { filter { excludeTestsMatching(conformanceTests) } }

val conformance =
    tasks.register<Test>("conformance") {
      description = "Runs every conformance case through the Java driver."
      group = LifecycleBasePlugin.VERIFICATION_GROUP
      testClassesDirs = sourceSets.test.get().output.classesDirs
      classpath = sourceSets.test.get().runtimeClasspath
      filter { includeTestsMatching(conformanceTests) }
    }

tasks.check { dependsOn(conformance) }
