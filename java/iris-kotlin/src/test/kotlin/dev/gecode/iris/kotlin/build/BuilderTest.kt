package dev.gecode.iris.kotlin.build

import dev.gecode.iris.IrisException
import dev.gecode.iris.Value
import dev.gecode.iris.build.BuildException
import dev.gecode.iris.build.DynField
import dev.gecode.iris.build.FloatField
import dev.gecode.iris.build.IntField
import dev.gecode.iris.build.NumField
import dev.gecode.iris.build.StringField
import org.junit.jupiter.api.Assertions.assertArrayEquals
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.assertThrows

/** Returns the line after the caller's. */
internal fun next(): Int =
    StackWalker.getInstance().walk { it.skip(1).findFirst().get() }.lineNumber + 1

/** Checks that [failure] names line [line] of this file. */
private fun assertNamesLine(line: Int, failure: BuildException) {
    val site = failure.callSite()
    assertEquals("BuilderTest.kt", site?.fileName, failure.message)
    assertEquals(line, site?.lineNumber, failure.message)
}

/** The line of [matrix] that defines its entry. */
internal var matrixLine = 0

/** A helper: an `n` by `n` matrix named [name], made through the caller's scope. */
context(scope: Scope0)
fun matrix(name: String, n: IntField<Long>): IntField<List<LongArray>> =
    scope.repeat(n) {
        repeat(n) {
            matrixLine = next()
            read(name, int())
        }
    }

class BuilderTest {
    private fun source(body: SpecScope.() -> Unit): String = irisBuilder(body = body).source()

    /** The example of the README. */
    @Test
    fun theReadmeExampleReads() {
        val (spec, handles) =
            irisSpec("pairs") {
                val n = read("n", atLeast(0))
                val (a, b) = repeat(n) { read("a", int()) to read("b", int()) }
                check(sum(a) le 100, "the a column is too large")
                Triple(n, a, b)
            }
        val (n, a, b) = handles
        val data = spec.read("2 1 2 3 4")
        val first: LongArray = data[a]
        assertEquals(2L, data[n])
        assertArrayEquals(longArrayOf(1, 3), first)
        assertArrayEquals(longArrayOf(2, 4), data[b])
    }

    @Test
    fun blocksNestAndTypeTheirDefinitions() {
        val built = irisSpec {
            val n = read("n", atLeast(0))
            val (row, flag) =
                repeat(n) {
                    val row: IntField<LongArray> = read("row", int())
                    val flag =
                        ifThen(n gt 1) {
                            val flag: StringField<List<String>?> = read("flag", word())
                            flag
                        }
                    row to flag
                }
            val deep: IntField<Value> =
                repeat(1) { repeat(1) { repeat(1) { repeat(1) { read("deep", int()) } } } }
            Triple(row, flag, deep)
        }
        val (row, flag, deep) = built.handles
        val data = built.spec.read("2 1 x 0 y 5")
        assertArrayEquals(longArrayOf(1, 0), data[row])
        val flags: List<String>? = data[flag]
        assertEquals(listOf("x", "y"), flags)
        assertEquals(5, data[deep].at(0, 0, 0, 0).asLong())
    }

    @Test
    fun everyBlockPrintsItsStatement() {
        assertEquals(
            EVERY_BLOCK,
            source {
                stop("END")
                separators(";")
                ignoreTrailing()
                val n = read("n", int())
                val records =
                    repeat(n) {
                        read("a", int())
                        record
                    }
                val b: FloatField<Double?> = ifThen(n gt 1) { read("b", float()) }
                val c: NumField<Number?> = elseIf(n gt 0) { read("c", num()) }
                val d: StringField<String?> = otherwise { read("d", word()) }
                repeatUntilEof { skipLine() }
                val end = forwardString("_")
                repeatUntil(until("end").store(end)) { read("e", rest()) }
                val x by capture(int())
                val f: IntField<Long?> =
                    sections(until("STOP"), skipOther()) {
                        arm("A ", x)
                        val f = optionalArm("B") { read("f", int()) }
                        arm("C", last = true)
                        f
                    }
                instances {}
                check(count(records) ge 0)
                listOf(b, c, d, f)
            },
        )
    }

    @Test
    fun operatorsPrintTheirIrisOperators() {
        val text = source {
            val a = read("a", int())
            val b = read("b", int())
            val f = read("f", float())
            val s = read("s", word())
            check((a + b) * 2 - a / b lt -a % 3)
            check((a le b) and !(a eq 1) or (b ne a))
            check((a ge 0) and (f gt 0.5) and (s eq "x") and (s ne lit("y")))
            check(-f lt f.neg())
            check(a.pow(2) eq -(-b))
            read("r", a..b)
            read("u", 0..<a)
        }
        assertEquals(
            """
            a: int
            b: int
            f: float
            s: word
            check (a + b) * 2 - a / b < -a % 3
            check a <= b && !(a == 1) || b != a
            check a >= 0 && f > 0.5 && s == "x" && s != "y"
            check -f < -f
            check a ^ 2 == --b
            r: a..b
            u: 0..<a

            """
                .trimIndent(),
            text,
        )
    }

    @Test
    fun aHelperCalledTwiceWithTwoNamesMakesTwoEntries() {
        val built = irisSpec {
            val n = read("n", atLeast(0))
            matrix("a", n) to matrix("b", n)
        }
        val (a, b) = built.handles
        val data = built.spec.read("2 1 2 3 4 5 6 7 8")
        assertArrayEquals(longArrayOf(3, 4), data[a][1])
        assertArrayEquals(longArrayOf(5, 6), data[b][0])
    }

    /** Principle 3: a name-based definition of a declared name is a redefinition by mistake. */
    @Test
    fun aHelperCalledTwiceWithOneNameFailsAtTheHelpersLine() {
        val failure =
            assertThrows<BuildException> {
                irisBuilder {
                    val n = read("n", atLeast(0))
                    matrix("a", n)
                    matrix("a", n)
                }
            }
        assertEquals(BuildException.Kind.REDEFINITION, failure.kind(), failure.message)
        assertNamesLine(matrixLine, failure)
    }

    @Test
    fun everyDefinitionByNameTakesOnlyANewName() {
        var line = 0
        val definitions: List<Scope0.() -> Unit> =
            listOf(
                {
                    line = next()
                    read("x", word())
                },
                {
                    line = next()
                    let("x", lit(1))
                },
                {
                    line = next()
                    let("x", lit("1").dynamic())
                },
                {
                    line = next()
                    param("x")
                },
                {
                    line = next()
                    param("x", lit(1.5))
                },
                {
                    line = next()
                    forwardInt("x")
                },
                {
                    line = next()
                    undefined("x")
                },
                {
                    line = next()
                    capture(int()).named("x")
                },
            )
        for (definition in definitions) {
            val failure =
                assertThrows<BuildException> {
                    irisBuilder {
                        read("x", int())
                        definition()
                    }
                }
            assertEquals(BuildException.Kind.REDEFINITION, failure.kind(), failure.message)
            assertNamesLine(line, failure)
        }
    }

    @Test
    fun aRedefinitionGoesThroughTheHandle() {
        val built = irisBuilder {
            val x = read("x", int())
            read(x, float())
            let(x, lit("s"))
        }
        assertEquals("x: int\nx: float\nlet x = \"s\"\n", built.source())
        assertEquals("s", built.build().spec.read("1 2.5")["x"].asString())
    }

    @Test
    fun aHelpersStatementNamesTheHelpersLine() {
        val failure =
            assertThrows<BuildException> { irisBuilder { matrix("1a", read("n", int())) } }
        assertNamesLine(matrixLine, failure)
    }

    @Test
    fun aStatementNamesTheCallersLine() {
        var line = 0
        val failure =
            assertThrows<BuildException> {
                irisBuilder {
                    line = next()
                    read("1x", int())
                }
            }
        assertNamesLine(line, failure)
    }

    @Test
    fun aCheckOfTheWholeSpecNamesTheCallersLine() {
        var line = 0
        val builder = irisBuilder {
            val n = read("n", int())
            ifThen(n gt 0) {
                line = next()
                read(int0("x"), int())
            }
        }
        assertNamesLine(line, assertThrows<BuildException> { builder.build() })
    }

    @Test
    fun aFailedCheckNamesTheCallersLine() {
        var line = 0
        val spec = irisSpec {
            val n = read("n", int())
            line = next()
            check(n gt 0)
        }
        val failure = assertThrows<IrisException> { spec.spec.read("0") }
        assertTrue("(BuilderTest.kt:$line)" in failure.message.orEmpty(), failure.message)
    }

    @Test
    fun aKeptScopeIsNotOpen() {
        var kept: Scope0? = null
        val failure =
            assertThrows<IllegalStateException> {
                irisBuilder {
                    instances { kept = this }
                    kept?.check(lit(1))
                }
            }
        assertTrue("innermost open block" in failure.message.orEmpty(), failure.message)
        assertThrows<IllegalStateException> {
            irisBuilder { repeat(1) { this@irisBuilder.skipLine() } }
        }
    }

    @Test
    fun anOuterBuildersScopeIsNotOpenInANestedBuilder() {
        val failure =
            assertThrows<IllegalStateException> {
                irisBuilder {
                    val outer = this
                    irisBuilder { outer.read("x", int()) }
                }
            }
        assertTrue("another builder" in failure.message.orEmpty(), failure.message)
        val outer = irisBuilder {
            val inner = irisBuilder { read("y", int()) }
            read("x", int())
            inner.source()
        }
        assertEquals("x: int\n", outer.source())
        assertEquals("y: int\n", outer.handles)
    }

    @Test
    fun theDynamicFormsPrintAndRead() {
        val built = irisBuilder {
            val x = forwardInt("x")
            read(x, int())
            val again: DynField = read(x, float())
            check(lit("1").dynamic() ne lit(1).dynamic())
            val zz = undefined("zz")
            check(!defined(zz))
            val p = param("p")
            val q: IntField<Long> = param("q", lit(2))
            val ifEntry = dynamic("if")
            ifThen(p eq "yes") { read("y", int()) }
            elseReadIf(ifEntry, int())
            val total = let("total", sum(listOf(q, lit(1))))
            read(discard(), int())
            list(int1("l"), int().until(0))
            set(total, total + 1)
            again to total
        }
        assertEquals(
            """
            x: int
            x: float
            check "1" != 1
            check !defined(zz)
            param p
            param q = 2
            if p == "yes" {
              y: int
            } else if: int
            let total = sum(q, 1)
            _: int
            list l: int until 0
            let total = total + 1

            """
                .trimIndent(),
            built.source(),
        )
        val (spec, handles) = built.build()
        val data =
            spec.read(
                "1 2.5 7 9 3 4 0",
                "input",
                mapOf("p" to "yes").let { dev.gecode.iris.Params.of(it) },
            )
        assertEquals(2.5, data[handles.first]?.asDouble())
        assertNull(data.find("if"))
    }

    @Test
    fun parametersWithHandleDefaultsAreScalars() {
        data class Params(
            val limit: IntField<Long>,
            val number: NumField<Number>,
            val label: StringField<String>,
            val each: IntField<LongArray>,
        )
        val built = irisBuilder {
            val n = read("n", int())
            val x = read("x", num())
            val w = read("w", word())
            Params(
                param("limit", n),
                param("number", x),
                param("label", w),
                repeat(n) { param("each", n + 1) },
            )
        }
        assertEquals(
            """
            n: int
            x: num
            w: word
            param limit = n
            param number = x
            param label = w
            repeat n {
              param each = n + 1
            }

            """
                .trimIndent(),
            built.source(),
        )
        val (spec, handles) = built.build()
        val data = spec.read("1 2.5 abc")
        val limit: Long = data[handles.limit]
        assertEquals(1L, limit)
        assertEquals(2.5, data[handles.number])
        assertEquals("abc", data[handles.label])
        assertArrayEquals(longArrayOf(2), data[handles.each])
        val given =
            spec.read(
                "1 2.5 abc",
                "input",
                dev.gecode.iris.Params.of(
                    mapOf("limit" to "9", "number" to "7.25", "label" to "given")
                ),
            )
        assertEquals(9L, given[handles.limit])
        assertEquals(7.25, given[handles.number])
        assertEquals("given", given[handles.label])
    }
}

/** What [BuilderTest.everyBlockPrintsItsStatement] prints. */
private val EVERY_BLOCK =
    """
    stop "END"
    separators ";"
    ignore trailing
    n: int
    repeat n as records_1 {
      a: int
    }
    if n > 1 {
      b: float
    } else if n > 0 {
      c: num
    } else {
      d: word
    }
    repeat until eof {
      skip line
    }
    repeat until "end" store _ {
      e: rest
    }
    sections until "STOP" skip other {
      "A {x:int}"
      "B" optional {
        f: int
      }
      "C" last
    }
    instances {}
    check count(records_1) >= 0

    """
        .trimIndent()
