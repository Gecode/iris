package dev.gecode.iris.kotlin

import java.io.ByteArrayOutputStream
import java.io.PrintStream
import java.nio.file.Path
import kotlin.io.path.createDirectory
import kotlin.io.path.writeText
import org.jetbrains.kotlin.cli.common.ExitCode
import org.jetbrains.kotlin.cli.jvm.K2JVMCompiler
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.io.TempDir

/**
 * Compiles snippets against `dev.gecode.iris` with the module's own compiler arguments, as what
 * Kotlin sees of `@NullMarked` shows only at compile time. `allWarningsAsErrors` is left out so
 * that the arguments alone must turn a mismatch into an error.
 */
class NullabilityTest {
    @TempDir lateinit var directory: Path

    private class Compilation(val exitCode: ExitCode, val messages: String)

    private fun compile(body: String): Compilation {
        val source = directory.resolve("Snippet.kt")
        source.writeText("import dev.gecode.iris.*\n$body\n")
        val messages = ByteArrayOutputStream()
        val arguments =
            listOf(
                source.toString(),
                "-d",
                directory.resolve("classes").createDirectory().toString(),
                "-classpath",
                System.getProperty("java.class.path"),
                "-jdk-home",
                System.getProperty("java.home"),
                "-no-stdlib",
                "-no-reflect",
            ) + System.getProperty("iris.kotlinArguments").split(' ')
        val exitCode = K2JVMCompiler().exec(PrintStream(messages, true), *arguments.toTypedArray())
        return Compilation(exitCode, messages.toString())
    }

    @Test
    fun aCorrectUseCompilesWithoutMessages() {
        val compilation = compile("public fun f(data: Data): Long? = data.find(\"a\")?.asLong()")
        assertEquals(ExitCode.OK, compilation.exitCode, compilation.messages)
        assertEquals("", compilation.messages.trim())
    }

    @Test
    fun aNullableResultIsNotNonNull() {
        val compilation = compile("public fun f(data: Data): Long = data.find(\"a\").asLong()")
        assertEquals(ExitCode.COMPILATION_ERROR, compilation.exitCode, compilation.messages)
        assertTrue("nullable receiver" in compilation.messages, compilation.messages)
    }

    @Test
    fun aNullableArgumentIsAnError() {
        val compilation = compile("public fun f(name: String?): Spec = Spec.parse(name)")
        assertEquals(ExitCode.COMPILATION_ERROR, compilation.exitCode, compilation.messages)
        assertTrue("String?" in compilation.messages, compilation.messages)
    }

    @Test
    fun resultsAreNotPlatformTypes() {
        val compilation =
            compile(
                """
                public fun f(data: Data): Long? = data.get("a")?.asLong()
                public fun g(data: Data): String? = data.entries().first()?.name()
                """
            )
        val warnings = compilation.messages.lines().filter { "nnecessary safe call" in it }
        assertEquals(2, warnings.size, compilation.messages)
    }
}
