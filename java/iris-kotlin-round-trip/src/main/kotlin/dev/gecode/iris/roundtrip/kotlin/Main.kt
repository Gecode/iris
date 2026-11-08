/**
 * Runs the builder programs of the Kotlin round trip. Each program is a file
 * `rt/NAME/SpecBuilder.kt` of the package `rt.NAME` with a function `specBuilder()`; the runner
 * writes the spec that it builds to `OUT/NAME.iris`, or why it built none to `OUT/NAME.error`.
 */
package dev.gecode.iris.roundtrip.kotlin

import dev.gecode.iris.kotlin.build.IrisBuilder
import java.lang.reflect.InvocationTargetException
import java.nio.file.Path
import kotlin.io.path.createDirectories
import kotlin.io.path.listDirectoryEntries
import kotlin.io.path.name
import kotlin.io.path.writeText

/** Runs every program under `args[0]` and writes its result to `args[1]`. */
fun main(args: Array<String>) {
    val programs = Path.of(args[0]).resolve("rt")
    val out = Path.of(args[1]).createDirectories()
    for (name in programs.listDirectoryEntries().map { it.name }.sorted()) {
        runCatching { source(name) }
            .onSuccess { out.resolve("$name.iris").writeText(it) }
            .onFailure { failure ->
                val cause = (failure as? InvocationTargetException)?.cause ?: failure
                out.resolve("$name.error").writeText(cause.toString())
            }
    }
}

/** The spec that the program `rt.NAME` builds. */
private fun source(name: String): String {
    val method = Class.forName("rt.$name.SpecBuilderKt").getDeclaredMethod("specBuilder")
    return (method.invoke(null) as IrisBuilder<*>).source()
}
