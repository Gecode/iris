plugins {
  `java-library`
  id("iris.java-conventions")
  id("iris.published")
}

dependencies { api(libs.jspecify) }

// Only the exported package has a public API. The source path resolves the rest.
tasks.javadoc {
  exclude("dev/gecode/iris/internal/**")
  (options as StandardJavadocDocletOptions).apply {
    addBooleanOption("Xwerror", true)
    addStringOption("-source-path", sourceSets.main.get().java.srcDirs.single().path)
  }
}

tasks.check { dependsOn(tasks.javadoc) }

// Error Prone and NullAway for the compilation tests of dev.gecode.iris.build, which run javac.
val nullawayCheck: Configuration by configurations.creating

dependencies {
  nullawayCheck(libs.errorprone.core)
  nullawayCheck(libs.nullaway)
}

val repository = rootDir.parentFile
val installedDriver = project(":iris-driver").layout.buildDirectory.dir("install/iris-driver")

tasks.withType<Test>().configureEach {
  dependsOn(":iris-driver:installDist")
  systemProperty("iris.driverHome", installedDriver.get().asFile.path)
  systemProperty("iris.repository", repository.path)
  inputs.files(nullawayCheck).withNormalizer(ClasspathNormalizer::class)
  jvmArgumentProviders.add(
      CommandLineArgumentProvider { listOf("-Diris.errorprone=${nullawayCheck.asPath}") }
  )
  providers.gradleProperty("irisFloatValues").orNull?.let { systemProperty("iris.floatValues", it) }
  providers.gradleProperty("irisCli").orNull?.let { systemProperty("iris.cli", it) }
  inputs.dir(installedDriver).withPathSensitivity(PathSensitivity.RELATIVE)
  val sources = listOf("rust/iris-conformance", "rust/iris-gen", "rust/iris-sema")
  for (path in listOf("conformance/cases/0.1", "formats") + sources) {
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
