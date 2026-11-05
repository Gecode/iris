plugins {
  `java-library`
  id("iris.java-conventions")
}

dependencies { api(libs.jspecify) }

val repository = rootDir.parentFile
val driver = project(":iris-driver").layout.buildDirectory.file("install/iris-driver/bin/iris-driver")

tasks.test {
  dependsOn(":iris-driver:installDist")
  systemProperty("iris.driver", driver.get().asFile.path)
  systemProperty("iris.repository", repository.path)
  providers.gradleProperty("irisFloatValues").orNull?.let { systemProperty("iris.floatValues", it) }
  providers.gradleProperty("irisCli").orNull?.let { systemProperty("iris.cli", it) }
  for (path in listOf("conformance/cases/0.1", "formats", "rust/iris-conformance")) {
    inputs.dir(repository.resolve(path)).withPathSensitivity(PathSensitivity.RELATIVE)
  }
}
