plugins {
    `java-library`
    id("iris.kotlin-conventions")
    id("iris.kotlin-docs")
    id("iris.published")
}

description = "Kotlin extensions for dev.gecode:iris, the Java library of Iris"

dependencies {
    api(project(":iris"))
    api(libs.kotlin.stdlib)
    testImplementation(libs.kotlin.compiler)
}

// javac sees the module's Kotlin classes when it checks module-info.java, the module's only Java.
tasks.compileJava {
    val kotlinClasses = tasks.compileKotlin.flatMap { it.destinationDirectory }
    options.compilerArgumentProviders.add(
        CommandLineArgumentProvider {
            listOf("--patch-module", "dev.gecode.iris.kotlin=${kotlinClasses.get().asFile.path}")
        }
    )
}

tasks.test {
    dependsOn(tasks.jar)
    systemProperty("iris.repository", rootDir.parentFile.path)
    systemProperty("iris.kotlinJar", tasks.jar.get().archiveFile.get().asFile.path)
    systemProperty(
        "iris.kotlinArguments",
        tasks.compileKotlin.get().compilerOptions.freeCompilerArgs.get().joinToString(" "),
    )
}

val checkDependencies =
    tasks.register("checkDependencies") {
        description = "Fails if a dependency other than §8.3's two has been added."
        group = LifecycleBasePlugin.VERIFICATION_GROUP
        val roots =
            listOf(configurations.compileClasspath, configurations.runtimeClasspath).map {
                it.get().incoming.resolutionResult.rootComponent
            }
        doLast {
            for (root in roots) {
                requireAllowedDependencies(root.get())
            }
        }
    }

tasks.check { dependsOn(checkDependencies) }

val allowedDirect = setOf("dev.gecode:iris", "org.jetbrains.kotlin:kotlin-stdlib")

// What those two bring themselves: annotations that are not needed at run time.
val allowedTransitive = setOf("org.jspecify:jspecify", "org.jetbrains:annotations")

fun requireAllowedDependencies(root: ResolvedComponentResult) {
    val direct = root.resolvedDependencies().map(::moduleOf).toSet()
    check(direct == allowedDirect) { "the direct dependencies are $direct, not $allowedDirect" }
    val all = closureOf(root).minus(root).map(::moduleOf).toSet()
    val unexpected = all - allowedDirect - allowedTransitive
    check(unexpected.isEmpty()) { "unexpected dependencies: $unexpected" }
}

fun ResolvedComponentResult.resolvedDependencies(): List<ResolvedComponentResult> =
    dependencies.filterIsInstance<ResolvedDependencyResult>().map { it.selected }

fun moduleOf(component: ResolvedComponentResult): String =
    component.moduleVersion.let { "${it?.group}:${it?.name}" }

fun closureOf(root: ResolvedComponentResult): Set<ResolvedComponentResult> {
    val seen = linkedSetOf(root)
    val pending = ArrayDeque(listOf(root))
    while (pending.isNotEmpty()) {
        val added = pending.removeLast().resolvedDependencies().filter(seen::add)
        pending.addAll(added)
    }
    return seen
}
