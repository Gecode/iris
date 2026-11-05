import dev.detekt.gradle.extensions.FailOnSeverity
import org.jetbrains.kotlin.gradle.dsl.JvmTarget
import org.jetbrains.kotlin.gradle.dsl.jvm.JvmTargetValidationMode
import org.jetbrains.kotlin.gradle.tasks.KotlinJvmCompile

plugins {
  id("iris.java-conventions")
  id("org.jetbrains.kotlin.jvm")
  id("dev.detekt")
}

val libs = versionCatalogs.named("libs")

kotlin {
  explicitApi()
  compilerOptions {
    jvmTarget = JvmTarget.JVM_26
    allWarningsAsErrors = true
    freeCompilerArgs.add("-Xjspecify-annotations=strict")
  }
}

// Kotlin has no JVM 27 target, so its classes are one version below what javac writes.
tasks.withType<KotlinJvmCompile>().configureEach {
  jvmTargetValidationMode = JvmTargetValidationMode.IGNORE
}

detekt {
  buildUponDefaultConfig = true
  config.setFrom(rootDir.resolve("detekt.yml"))
  failOnSeverity = FailOnSeverity.Info
}

tasks.check { dependsOn("detektMain", "detektTest") }

spotless {
  kotlin { ktfmt(libs.findVersion("ktfmt").get().requiredVersion).kotlinlangStyle() }
  kotlinGradle { ktfmt(libs.findVersion("ktfmt").get().requiredVersion).kotlinlangStyle() }
}
