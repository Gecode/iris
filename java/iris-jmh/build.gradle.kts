import net.ltgt.gradle.errorprone.errorprone
import net.ltgt.gradle.nullaway.nullaway

plugins { id("iris.java-conventions") }

dependencies {
  implementation(project(":iris"))
  implementation(libs.jmh.core)
  annotationProcessor(libs.jmh.generator)
}

tasks.compileJava {
  // The JMH processor claims only its own annotations, which javac reports as a warning.
  options.compilerArgs.add("-Xlint:-processing")
  // Error Prone checks the benchmarks, but not the classes that JMH generates from them.
  options.errorprone.excludedPaths = ".*/jmh_generated/.*"
  // JMH sets up a benchmark's state in its @Setup method, not in a constructor.
  options.errorprone.nullaway { customInitializerAnnotations.add("org.openjdk.jmh.annotations.Setup") }
}

val formats = rootDir.resolveSibling("formats")

tasks.test {
  systemProperty("iris.formats", formats.path)
  inputs.dir(formats).withPathSensitivity(PathSensitivity.RELATIVE)
}

tasks.register<JavaExec>("jmh") {
  description = "Runs the benchmarks. --args replaces the JMH options below."
  group = LifecycleBasePlugin.VERIFICATION_GROUP
  javaLauncher = javaToolchains.launcherFor(java.toolchain)
  classpath = sourceSets.main.get().runtimeClasspath
  mainClass = "org.openjdk.jmh.Main"
  // JMH's forks inherit this property.
  systemProperty("iris.formats", formats.path)
  val results = layout.buildDirectory.file("results/jmh.json").get().asFile
  doFirst { results.parentFile.mkdirs() }
  args("-f", "1", "-foe", "true", "-wi", "5", "-w", "2s", "-i", "10", "-r", "1s")
  args("-rf", "json", "-rff", results.path)
}
