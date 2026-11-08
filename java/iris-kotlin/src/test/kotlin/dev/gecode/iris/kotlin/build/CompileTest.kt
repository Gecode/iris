package dev.gecode.iris.kotlin.build

import java.io.ByteArrayOutputStream
import java.io.PrintStream
import java.nio.file.Path
import kotlin.io.path.createTempDirectory
import kotlin.io.path.writeText
import org.jetbrains.kotlin.cli.common.ExitCode
import org.jetbrains.kotlin.cli.jvm.K2JVMCompiler
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.io.TempDir

/**
 * Compiles user code against the builder with the module's own compiler arguments: typed reads have
 * the handles' types, and misuse fails to compile.
 */
class CompileTest {
    @TempDir lateinit var directory: Path

    private class Compilation(val exitCode: ExitCode, val messages: String)

    private fun compile(body: String): Compilation {
        val source = directory.resolve("Snippet.kt")
        source.writeText(
            """
            import dev.gecode.iris.Data
            import dev.gecode.iris.build.FloatField
            import dev.gecode.iris.build.IntField
            import dev.gecode.iris.kotlin.build.*

            """
                .trimIndent() + body
        )
        val messages = ByteArrayOutputStream()
        val arguments =
            listOf(
                source.toString(),
                "-d",
                createTempDirectory(directory).toString(),
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

    private fun assertFails(body: String, message: String) {
        val compilation = compile(body)
        assertEquals(ExitCode.COMPILATION_ERROR, compilation.exitCode, compilation.messages)
        assertTrue(message in compilation.messages, compilation.messages)
    }

    private val handles =
        """
        private val built = irisSpec {
            val n = read("n", int())
            val x = repeat(n) { read("x", float()) }
            val w = ifThen(n gt 0) { read("w", word()) }
            Triple(n, x, w)
        }
        private val n = built.handles.first
        private val x = built.handles.second
        private val w = built.handles.third

        """
            .trimIndent()

    @Test
    fun readsHaveTheHandlesTypesWithoutPlatformTypes() {
        val compilation =
            compile(
                handles +
                    """
                    private fun f(data: Data): Long = data[n]
                    private fun g(data: Data): DoubleArray = data[x]
                    private fun h(data: Data): String? = data[w]
                    private fun i(data: Data): Long? = data[n]?.plus(1)
                    """
                        .trimIndent()
            )
        assertEquals(ExitCode.OK, compilation.exitCode, compilation.messages)
        val warnings =
            compilation.messages.lines().filter { it.startsWith("warning:") || ": warning:" in it }
        assertEquals(1, warnings.size, compilation.messages)
        assertTrue("nnecessary safe call" in warnings.single(), compilation.messages)
    }

    @Test
    fun anOptionalEntryIsNotNonNull() {
        assertFails(handles + "private fun h(data: Data): String = data[w]", "actual 'String?'")
    }

    @Test
    fun aHandleOfAnotherTypeIsRejected() {
        assertFails(
            "private val wrong = irisSpec { val n: FloatField<Double> = read(\"n\", int()); n }",
            "expected 'FloatField<Double>', actual 'IntField<Long>'",
        )
        assertFails(
            "private val deeper = irisSpec { repeat(2) { val n: IntField<Long> = read(\"n\", int()); n } }",
            "actual 'IntField<LongArray>'",
        )
    }

    @Test
    fun aComparisonOfAStringWithANumberIsRejected() {
        assertFails(
            "private val bad = irisSpec { check(read(\"s\", word()) lt 1) }",
            "none of the following candidates is applicable",
        )
    }

    @Test
    fun anInnerBlockCannotReachAnOuterBlocksStatements() {
        assertFails(
            "private val nested = irisSpec { instances { comment(\"#\") } }",
            "cannot be called in this context with an implicit receiver",
        )
    }
}
