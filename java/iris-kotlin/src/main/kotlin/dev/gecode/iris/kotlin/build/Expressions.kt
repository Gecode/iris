@file:TrackCaller
@file:Suppress("TooManyFunctions") // Literals, functions, comprehensions and operators.

package dev.gecode.iris.kotlin.build

import dev.gecode.iris.build.Domain
import dev.gecode.iris.build.DynExpr
import dev.gecode.iris.build.Expr
import dev.gecode.iris.build.FloatExpr
import dev.gecode.iris.build.IntExpr
import dev.gecode.iris.build.IntegralExpr
import dev.gecode.iris.build.Iris
import dev.gecode.iris.build.NumExpr
import dev.gecode.iris.build.NumericExpr
import dev.gecode.iris.build.Over
import dev.gecode.iris.build.RecordExpr
import dev.gecode.iris.build.RecordList
import dev.gecode.iris.build.RecordName
import dev.gecode.iris.build.StringExpr
import dev.gecode.iris.build.TextualExpr
import dev.gecode.iris.build.TrackCaller

/** An integer literal. */
public fun lit(value: Long): IntExpr = Iris.lit(value)

/** A float literal. */
public fun lit(value: Double): FloatExpr = Iris.lit(value)

/** A string literal. */
public fun lit(value: String): StringExpr = Iris.lit(value)

/** `sum(...)`. */
public fun sum(first: Expr, vararg more: Expr): IntExpr = Iris.sum(first, *more)

/** `sum(...)`. */
public fun sum(arguments: List<Expr>): IntExpr = Iris.sum(arguments)

/** `min(...)`. */
public fun min(first: Expr, vararg more: Expr): IntExpr = Iris.min(first, *more)

/** `min(...)`. */
public fun min(arguments: List<Expr>): IntExpr = Iris.min(arguments)

/** `max(...)`. */
public fun max(first: Expr, vararg more: Expr): IntExpr = Iris.max(first, *more)

/** `max(...)`. */
public fun max(arguments: List<Expr>): IntExpr = Iris.max(arguments)

/** `distinct(...)`. */
public fun distinct(first: Expr, vararg more: Expr): IntExpr = Iris.distinct(first, *more)

/** `distinct(...)`. */
public fun distinct(arguments: List<Expr>): IntExpr = Iris.distinct(arguments)

/** `sum(v in domain: body)`. */
public fun <E : Any> sum(domain: Domain<E>, body: (E) -> Expr): IntExpr =
    Iris.sum(domain) { body(it) }

/** `all(v in domain: body)`. */
public fun <E : Any> all(domain: Domain<E>, body: (E) -> Expr): IntExpr =
    Iris.all(domain) { body(it) }

/** `any(v in domain: body)`. */
public fun <E : Any> any(domain: Domain<E>, body: (E) -> Expr): IntExpr =
    Iris.any(domain) { body(it) }

/** `count(v in domain: body)`. */
public fun <E : Any> count(domain: Domain<E>, body: (E) -> Expr): IntExpr =
    Iris.count(domain) { body(it) }

/** `count(domain)`. */
public fun count(domain: Domain<*>): IntExpr = Iris.count(domain)

/** `size(domain)`. */
public fun size(domain: Domain<*>): IntExpr = Iris.size(domain)

/** `defined(name)`. */
public fun defined(name: Expr): IntExpr = Iris.defined(name)

/** `defined(record)`. */
public fun defined(record: RecordName): IntExpr = Iris.defined(record)

/** `defined(records)`. */
public fun defined(records: RecordList): IntExpr = Iris.defined(records)

/** `defined(record)`. */
public fun defined(record: RecordExpr): IntExpr = Iris.defined(record)

/** A comprehension domain whose variable [Over.named] names. */
public fun <E : Any> over(domain: Domain<E>): Over<E> = Iris.over(domain)

// Operators: Kotlin's arithmetic operators come from the Java methods `plus`, `minus`, `times`,
// `div` and `not`.

/** `this % other`, which truncates toward zero in Kotlin and Iris alike. */
public operator fun IntegralExpr.rem(other: IntegralExpr): IntExpr = mod(other)

/** `this % other`. */
public operator fun IntegralExpr.rem(other: Long): IntExpr = mod(other)

/** `-this`. */
public operator fun IntExpr.unaryMinus(): IntExpr = neg()

/** `-this`. */
public operator fun FloatExpr.unaryMinus(): FloatExpr = neg()

/** `-this`. */
public operator fun NumExpr.unaryMinus(): NumExpr = neg()

/** `-this`. */
public operator fun DynExpr.unaryMinus(): DynExpr = neg()

/** `-this`. */
public operator fun NumericExpr.unaryMinus(): NumericExpr = neg()

/** `this < other`. */
public infix fun NumericExpr.lt(other: NumericExpr): IntExpr = lt(other)

/** `this < other`. */
public infix fun NumericExpr.lt(other: Long): IntExpr = lt(other)

/** `this < other`. */
public infix fun NumericExpr.lt(other: Double): IntExpr = lt(other)

/** `this <= other`. */
public infix fun NumericExpr.le(other: NumericExpr): IntExpr = le(other)

/** `this <= other`. */
public infix fun NumericExpr.le(other: Long): IntExpr = le(other)

/** `this <= other`. */
public infix fun NumericExpr.le(other: Double): IntExpr = le(other)

/** `this > other`. */
public infix fun NumericExpr.gt(other: NumericExpr): IntExpr = gt(other)

/** `this > other`. */
public infix fun NumericExpr.gt(other: Long): IntExpr = gt(other)

/** `this > other`. */
public infix fun NumericExpr.gt(other: Double): IntExpr = gt(other)

/** `this >= other`. */
public infix fun NumericExpr.ge(other: NumericExpr): IntExpr = ge(other)

/** `this >= other`. */
public infix fun NumericExpr.ge(other: Long): IntExpr = ge(other)

/** `this >= other`. */
public infix fun NumericExpr.ge(other: Double): IntExpr = ge(other)

/** `this == other`. */
public infix fun NumericExpr.eq(other: NumericExpr): IntExpr = eq(other)

/** `this == other`. */
public infix fun NumericExpr.eq(other: Long): IntExpr = eq(other)

/** `this == other`. */
public infix fun NumericExpr.eq(other: Double): IntExpr = eq(other)

/** `this != other`. */
public infix fun NumericExpr.ne(other: NumericExpr): IntExpr = ne(other)

/** `this != other`. */
public infix fun NumericExpr.ne(other: Long): IntExpr = ne(other)

/** `this != other`. */
public infix fun NumericExpr.ne(other: Double): IntExpr = ne(other)

/** `this == other`, of strings. */
public infix fun TextualExpr.eq(other: TextualExpr): IntExpr = eq(other)

/** `this == other`, of strings. */
public infix fun TextualExpr.eq(other: String): IntExpr = eq(other)

/** `this != other`, of strings. */
public infix fun TextualExpr.ne(other: TextualExpr): IntExpr = ne(other)

/** `this != other`, of strings. */
public infix fun TextualExpr.ne(other: String): IntExpr = ne(other)

/** `this == other`, of values whose type the data decide. */
public infix fun DynExpr.eq(other: DynExpr): IntExpr = eq(other)

/** `this != other`, of values whose type the data decide. */
public infix fun DynExpr.ne(other: DynExpr): IntExpr = ne(other)

/** `this && other`. */
public infix fun IntegralExpr.and(other: IntegralExpr): IntExpr = and(other)

/** `this && other`. */
public infix fun IntegralExpr.and(other: Long): IntExpr = and(other)

/** `this || other`. */
public infix fun IntegralExpr.or(other: IntegralExpr): IntExpr = or(other)

/** `this || other`. */
public infix fun IntegralExpr.or(other: Long): IntExpr = or(other)
