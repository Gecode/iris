import com.diffplug.spotless.LineEnding
import net.ltgt.gradle.errorprone.errorprone
import net.ltgt.gradle.nullaway.nullaway

plugins {
  java
  id("com.diffplug.spotless")
  id("net.ltgt.errorprone")
  id("net.ltgt.nullaway")
}

// The libs accessor does not exist in precompiled script plugins.
val libs = versionCatalogs.named("libs")

fun library(alias: String) = libs.findLibrary(alias).get()

java { toolchain { languageVersion = JavaLanguageVersion.of(27) } }

dependencies {
  errorprone(library("errorprone-core"))
  errorprone(library("nullaway"))
  testImplementation(platform(library("junit-bom")))
  testImplementation(library("junit-jupiter"))
  testRuntimeOnly(library("junit-launcher"))
}

nullaway { onlyNullMarked = true }

tasks.withType<JavaCompile>().configureEach {
  options.release = 27
  options.encoding = "UTF-8"
  options.compilerArgs.addAll(listOf("-Xlint:all", "-Werror"))
  options.errorprone.nullaway {
    error()
    jspecifyMode = true
  }
}

tasks.withType<Test>().configureEach {
  useJUnitPlatform()
  testLogging {
    showStandardStreams = true
    exceptionFormat = org.gradle.api.tasks.testing.logging.TestExceptionFormat.FULL
  }
}

spotless {
  // Git settings decide the default, which would ask for CRLF on Windows.
  lineEndings = LineEnding.UNIX
  java { googleJavaFormat(libs.findVersion("google-java-format").get().requiredVersion) }
}
