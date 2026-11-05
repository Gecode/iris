package dev.gecode.iris.kotlin

import dev.gecode.iris.Data
import dev.gecode.iris.Params
import dev.gecode.iris.Value

/**
 * Follows indices into this value, as in `data["grid"][1, 2]`.
 *
 * A single index needs no extension: Kotlin calls [Value.get], and [Data.get] for a name, as
 * operators.
 *
 * @throws IllegalStateException if a level is not an array
 * @throws IndexOutOfBoundsException if an index is out of range
 */
public operator fun Value.get(vararg path: Int): Value = at(*path)

/**
 * Returns a new array of the integers of this one-dimensional array.
 *
 * @throws ArithmeticException if an integer does not fit an [Int]
 * @throws IllegalStateException if this is not a one-dimensional array of integers
 */
public fun Value.toIntArray(): IntArray = ints()

/**
 * Returns a new array of the integers of this one-dimensional array.
 *
 * @throws IllegalStateException if this is not a one-dimensional array of integers
 */
public fun Value.toLongArray(): LongArray = longs()

/**
 * Returns a new array of the numbers of this one-dimensional array.
 *
 * An integer becomes the nearest [Double], as in [Value.doubles], so one above 2^53 can change.
 *
 * @throws IllegalStateException if this is not a one-dimensional array of numbers
 */
public fun Value.toDoubleArray(): DoubleArray = doubles()

/**
 * Returns the elements of this array, which are made as the sequence reaches them.
 *
 * @throws IllegalStateException if this is a single value
 */
public fun Value.asSequence(): Sequence<Value> =
    (0 until size()).asSequence().map { index -> this[index] }

/**
 * Returns the single values of this value in row order, at any depth: this value itself if it is
 * single.
 *
 * Dimensions have no bound, so the walk keeps its place in a list of open arrays instead of
 * recursing.
 */
public fun Value.leaves(): Sequence<Value> = sequence {
    val open = ArrayDeque(listOf(sequenceOf(this@leaves).iterator()))
    while (open.isNotEmpty()) {
        val elements = open.last()
        if (elements.hasNext()) {
            val element = elements.next()
            if (element.depth() == 0) yield(element)
            else open.addLast(element.asSequence().iterator())
        } else {
            open.removeLast()
        }
    }
}

/** Returns the entries of this data, in definition order. */
public fun Data.asSequence(): Sequence<Data.Entry> = entries().asSequence()

/**
 * Makes parameters from this map, in its iteration order, which for `mapOf(...)` is the order
 * written.
 *
 * @throws IllegalArgumentException if a name or value has an unpaired surrogate, which is not text
 */
public fun Map<String, String>.toParams(): Params = Params.of(this)
