@file:TrackCaller
@file:Suppress("TooManyFunctions") // Types, ranges, pattern parts and section clauses.

package dev.gecode.iris.kotlin.build

import dev.gecode.iris.Field
import dev.gecode.iris.build.Alphabet
import dev.gecode.iris.build.Discard
import dev.gecode.iris.build.FloatType
import dev.gecode.iris.build.IntType
import dev.gecode.iris.build.Iris
import dev.gecode.iris.build.NumType
import dev.gecode.iris.build.NumericExpr
import dev.gecode.iris.build.Part
import dev.gecode.iris.build.Range
import dev.gecode.iris.build.SectionClause
import dev.gecode.iris.build.StringType
import dev.gecode.iris.build.TrackCaller
import dev.gecode.iris.build.Type
import dev.gecode.iris.build.Until

/** `int`. */
public fun int(): IntType = Iris.intType()

/** `float`. */
public fun float(): FloatType = Iris.floatType()

/** `num`. */
public fun num(): NumType = Iris.numType()

/** `word`. */
public fun word(): StringType = Iris.word()

/** `rest`. */
public fun rest(): StringType = Iris.rest()

/** `alphabet "symbols"`, as [Iris.alphabet]. */
public fun alphabet(symbols: String): Alphabet = Iris.alphabet(symbols)

/** The range `lo..`, whose upper bound [Range.to] or [Range.below] adds. */
public fun range(lo: NumericExpr): Range = Iris.range(lo)

/** The range `lo..`, whose upper bound [Range.to] or [Range.below] adds. */
public fun range(lo: Long): Range = Iris.range(lo)

/** `lo..`. */
public fun atLeast(lo: NumericExpr): IntType = Iris.atLeast(lo)

/** `lo..`. */
public fun atLeast(lo: Long): IntType = Iris.atLeast(lo)

/** `..hi`. */
public fun atMost(hi: NumericExpr): IntType = Iris.atMost(hi)

/** `..hi`. */
public fun atMost(hi: Long): IntType = Iris.atMost(hi)

/** `..<hi`. */
public fun below(hi: NumericExpr): IntType = Iris.below(hi)

/** `..<hi`. */
public fun below(hi: Long): IntType = Iris.below(hi)

/** The range `this..hi`, which Kotlin and Iris spell alike. */
public operator fun NumericExpr.rangeTo(hi: NumericExpr): IntType = Iris.range(this).to(hi)

/** The range `this..hi`. */
public operator fun NumericExpr.rangeTo(hi: Long): IntType = Iris.range(this).to(hi)

/** The range `this..hi`. */
public operator fun Long.rangeTo(hi: NumericExpr): IntType = Iris.range(this).to(hi)

/** The range `this..hi`. */
public operator fun Int.rangeTo(hi: NumericExpr): IntType = Iris.range(toLong()).to(hi)

/** The range `this..<hi`, which Kotlin and Iris spell alike. */
public operator fun NumericExpr.rangeUntil(hi: NumericExpr): IntType = Iris.range(this).below(hi)

/** The range `this..<hi`. */
public operator fun NumericExpr.rangeUntil(hi: Long): IntType = Iris.range(this).below(hi)

/** The range `this..<hi`. */
public operator fun Long.rangeUntil(hi: NumericExpr): IntType = Iris.range(this).below(hi)

/** The range `this..<hi`. */
public operator fun Int.rangeUntil(hi: NumericExpr): IntType = Iris.range(toLong()).below(hi)

/** `{name:type}`, a capture through an explicit handle, as [Iris.capture]. */
public fun capture(handle: Field<*>, type: Type<*>): Part = Iris.capture(handle, type)

/** `{$name}`, the value of an already bound name. */
public fun ref(handle: Field<*>): Part = Iris.ref(handle)

/** `{:type}`, a value that is checked and not kept. */
public fun validate(type: Type<*>): Part = Iris.validate(type)

/** `{}`, blanks. */
public fun blank(): Part = Iris.blank()

/** `_`: as a pattern part `{_}`, and as a target a definition that keeps nothing. */
public fun discard(): Discard = Iris.discard()

/** `{_:type}`, a value of [type] that is not kept. */
public fun discard(type: Type<*>): Part = Iris.discard(type)

/** `...`, the rest of the line. */
public fun restOfLine(): Part = Iris.restOfLine()

/** `until "terminator" ...`, whose [Until.store] adds a `store`. */
public fun until(terminator: String, vararg more: String): Until = Iris.until(terminator, *more)

/** `skip other`. */
public fun skipOther(): SectionClause = Iris.skipOther()
