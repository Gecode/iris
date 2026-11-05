package dev.gecode.iris.kotlin

import dev.gecode.iris.Data
import dev.gecode.iris.ErrorCode
import dev.gecode.iris.IrisException
import dev.gecode.iris.Spec
import dev.gecode.iris.Value
import org.junit.jupiter.api.Assertions.assertArrayEquals
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.assertThrows

class ExtensionsTest {
    private fun read(spec: String, input: String): Data = Spec.parse(spec).read(input)

    private val rows: Value = read("repeat 3 {\nn: int\na[n]: int\n}", "2 1 2 0 1 3")["a"]

    @Test
    fun getSelectsByNameAndIndex() {
        val data = read("clause[4]: int", "5 6 7 8")
        assertEquals(8, data["clause"][3].asLong())
    }

    @Test
    fun getFollowsSeveralIndices() {
        val data = read("grid[2][3]: int", "1 2 3 4 5 6")
        assertEquals(6, data["grid"][1, 2].asLong())
        assertEquals(data["grid"].at(1, 2), data["grid"][1, 2])
    }

    @Test
    fun getMisuseIsNotAnIrisException() {
        val data = read("a[2]: int\nb: int", "1 2 3")
        assertThrows<IllegalStateException> { data["missing"] }
        assertThrows<IndexOutOfBoundsException> { data["a"][2] }
        assertThrows<IllegalStateException> { data["a"][0, 0] }
        assertThrows<IllegalStateException> { data["b"][0] }
    }

    @Test
    fun findIsNullForAMissingEntry() {
        val data = read("a: int", "1")
        val missing: Value? = data.find("b")
        assertNull(missing)
    }

    @Test
    fun convertsToPrimitiveArrays() {
        val data = read("i[2]: int\nf[2]: float", "1 2 0.5 1.5")
        assertArrayEquals(intArrayOf(1, 2), data["i"].toIntArray())
        assertArrayEquals(longArrayOf(1, 2), data["i"].toLongArray())
        assertArrayEquals(doubleArrayOf(1.0, 2.0), data["i"].toDoubleArray())
        assertArrayEquals(doubleArrayOf(0.5, 1.5), data["f"].toDoubleArray())
    }

    @Test
    fun convertsARowOfARaggedArray() {
        assertArrayEquals(intArrayOf(1, 2), rows[0].toIntArray())
        assertArrayEquals(intArrayOf(), rows[1].toIntArray())
    }

    @Test
    fun eachArrayIsANewCopy() {
        val value = read("a[2]: int", "1 2")["a"]
        value.toIntArray()[0] = 9
        value.toLongArray()[0] = 9
        value.toDoubleArray()[0] = 9.0
        assertArrayEquals(longArrayOf(1, 2), value.toLongArray())
    }

    @Test
    fun anIntegerAboveTheRangeOfIntIsAnError() {
        val fits = read("a[2]: int", "${Int.MIN_VALUE} ${Int.MAX_VALUE}")["a"]
        assertArrayEquals(intArrayOf(Int.MIN_VALUE, Int.MAX_VALUE), fits.toIntArray())

        val above = Int.MAX_VALUE.toLong() + 1
        val value = read("a[2]: int", "1 $above")["a"]
        assertThrows<ArithmeticException> { value.toIntArray() }
        assertArrayEquals(longArrayOf(1, above), value.toLongArray())
    }

    @Test
    fun anIntegerBecomesTheNearestDouble() {
        val value = read("a[1]: int", "9007199254740993")["a"]
        assertArrayEquals(doubleArrayOf(9007199254740992.0), value.toDoubleArray())
        assertArrayEquals(longArrayOf(9007199254740993), value.toLongArray())
    }

    @Test
    fun arrayMisuseIsNotAnIrisException() {
        val data = read("one: int\ngrid[1][1]: int\nwords[1]: word\nf[1]: float", "1 2 x 0.5")
        for (value in listOf(data["one"], data["grid"], data["words"])) {
            assertThrows<IllegalStateException> { value.toIntArray() }
            assertThrows<IllegalStateException> { value.toLongArray() }
            assertThrows<IllegalStateException> { value.toDoubleArray() }
        }
        assertThrows<IllegalStateException> { data["f"].toLongArray() }
    }

    @Test
    fun asSequenceViewsTheElements() {
        val sizes = rows.asSequence().map { row -> row.size() }.toList()
        assertEquals(listOf(2, 0, 1), sizes)
    }

    @Test
    fun aSequenceCanRunAgainAndStopEarly() {
        val elements = rows.asSequence()
        assertEquals(rows[0], elements.first())
        assertEquals(3, elements.count())
    }

    @Test
    fun aSingleValueHasNoElements() {
        val value = read("a: int", "1")["a"]
        assertThrows<IllegalStateException> { value.asSequence().count() }
    }

    @Test
    fun leavesWalkRaggedArraysInRowOrder() {
        assertEquals(listOf<Long>(1, 2, 3), rows.leaves().map(Value::asLong).toList())
    }

    @Test
    fun leavesOfASingleValueAreTheValue() {
        val value = read("a: int", "7")["a"]
        assertEquals(listOf(value), value.leaves().toList())
    }

    @Test
    fun anEmptyArrayHasNoLeaves() {
        assertEquals(0, read("a[0][2]: int", "")["a"].leaves().count())
    }

    @Test
    fun leavesWalkAValueOfManyDimensions() {
        val depth = 50_000
        val value = read("a" + "[1]".repeat(depth) + ": int", "7")["a"]
        assertEquals(depth, value.depth())
        assertEquals(listOf<Long>(7), value.leaves().map(Value::asLong).toList())
    }

    @Test
    fun dataAsSequenceKeepsDefinitionOrder() {
        val data = read("b: int\na: int", "1 2")
        assertEquals(listOf("b", "a"), data.asSequence().map { entry -> entry.name }.toList())
    }

    @Test
    fun paramsComeFromAMapInItsOrder() {
        val params = mapOf("b" to "1", "a" to "2").toParams()
        assertEquals(listOf("b" to "1", "a" to "2"), params.values().toList())
    }

    @Test
    fun paramsOfAMapReachARead() {
        val spec = Spec.parse("param k = 1\nlet a = k")
        val data = spec.read("", "input", mapOf("k" to "5").toParams())
        assertEquals(5, data["a"].asLong())
    }

    @Test
    fun paramsKeepTheirValuesWhenTheMapChanges() {
        val map = mutableMapOf("k" to "1")
        val params = map.toParams()
        map["k"] = "2"
        assertEquals(mapOf("k" to "1"), params.values())
    }

    @Test
    fun anUnpairedSurrogateIsNotAParameter() {
        assertThrows<IllegalArgumentException> { mapOf("k" to "\uD83D").toParams() }
    }

    @Test
    fun aReadStillFailsWithAnIrisException() {
        val error = assertThrows<IrisException> { read("a: int", "x") }
        assertEquals(ErrorCode.EXPECTED_VALUE, error.code())
    }
}
