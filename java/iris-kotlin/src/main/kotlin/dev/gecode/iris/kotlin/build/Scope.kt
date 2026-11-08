@file:TrackCaller

package dev.gecode.iris.kotlin.build

import dev.gecode.iris.Field
import dev.gecode.iris.build.ArmOption
import dev.gecode.iris.build.Discard
import dev.gecode.iris.build.DynExpr
import dev.gecode.iris.build.DynField
import dev.gecode.iris.build.Expr
import dev.gecode.iris.build.FloatExpr
import dev.gecode.iris.build.FloatField
import dev.gecode.iris.build.FloatType
import dev.gecode.iris.build.IntExpr
import dev.gecode.iris.build.IntField
import dev.gecode.iris.build.IntType
import dev.gecode.iris.build.Iris
import dev.gecode.iris.build.ListReading
import dev.gecode.iris.build.NumExpr
import dev.gecode.iris.build.NumField
import dev.gecode.iris.build.NumType
import dev.gecode.iris.build.Pattern
import dev.gecode.iris.build.RecordName
import dev.gecode.iris.build.SectionClause
import dev.gecode.iris.build.StringExpr
import dev.gecode.iris.build.StringField
import dev.gecode.iris.build.StringType
import dev.gecode.iris.build.TrackCaller
import dev.gecode.iris.build.Type
import dev.gecode.iris.build.Until
import java.util.IdentityHashMap

/**
 * Marks the receivers of the builder, so that a block's lambda cannot reach an outer block's
 * statements without naming that receiver.
 */
@DslMarker @Target(AnnotationTarget.CLASS) public annotation class IrisDsl

/** What the scopes of one builder share. */
@TrackCaller
internal class Session {
    /** The type of each handle that a capture delegate made, by identity. */
    val captures: MutableMap<Field<*>, Type<*>> = IdentityHashMap()

    /** The innermost open receiver. */
    var open: Any? = null

    /** The pattern of [parts], in which a handle from a capture delegate becomes its capture. */
    @Suppress("SpreadOperator") // Java's varargs take the converted array.
    fun pattern(parts: Array<out Any>): Pattern {
        val converted =
            Array(parts.size) { index ->
                val part = parts[index]
                val type = if (part is Field<*>) captures[part] else null
                if (type == null) part else Iris.capture(part as Field<*>, type)
            }
        return Iris.pattern(*converted)
    }

    /**
     * Fails unless [receiver] is the innermost open block of the builder being built on this
     * thread, since a statement on a receiver of an outer builder would go to the inner one.
     */
    fun requireOpen(receiver: Any) {
        check(active.get() === this) {
            "this scope belongs to another builder than the one being built: a statement belongs" +
                " to the block whose lambda makes it"
        }
        check(open === receiver) {
            "this scope is not the innermost open block: a statement belongs to the block whose" +
                " lambda makes it"
        }
    }

    /** Runs [body] with this session as the one being built on this thread. */
    fun <T> activate(body: () -> T): T {
        val previous = active.get()
        active.set(this)
        try {
            return body()
        } finally {
            active.set(previous)
        }
    }

    /**
     * Runs [body] on [inner] while [statement] keeps its block open, and returns what it returns.
     */
    fun <X : Any, T> nest(outer: Any, inner: X, statement: (Runnable) -> Unit, body: X.() -> T): T {
        requireOpen(outer)
        val results = ArrayList<T>(1)
        statement {
            open = inner
            try {
                results.add(inner.body())
            } finally {
                open = outer
            }
        }
        return results.single()
    }

    private companion object {
        /** The session of the builder being built on each thread, innermost first. */
        val active = ThreadLocal<Session?>()
    }
}

/**
 * A block of a spec, the receiver of its lambda. Its statements are those of
 * [dev.gecode.iris.build.Iris], and add to this block.
 *
 * The type parameters fix what the block's definitions read: [I], [F], [N] and [S] are the data
 * view types of an `int`, `float`, `num` and string entry defined here, such as `Long` at depth 0,
 * `LongArray` at depth 1, `Long?` in a conditional block and [dev.gecode.iris.Value] below
 * depth 3. [B] is the receiver of a block at the same depth and presence, [R] of a repeat's body,
 * and [C] of a conditional block.
 *
 * A definition that takes a name declares it, as [Iris.declare], so it fails with a
 * [dev.gecode.iris.build.BuildException] when the builder has already declared the name: a helper
 * called twice with one name is an error. Redefinition goes through the handle, with the overloads
 * that take one, such as `read(handle, type)` and [set].
 */
@IrisDsl
@TrackCaller
@Suppress("TooManyFunctions") // Each statement of Iris is a member.
public sealed class Scope<
    I,
    F,
    N,
    S,
    B : Any,
    R : Any,
    C : Any,
> protected constructor() {
    internal abstract val session: Session

    internal abstract fun ints(name: String): IntField<I>

    internal abstract fun floats(name: String): FloatField<F>

    internal abstract fun nums(name: String): NumField<N>

    internal abstract fun strings(name: String): StringField<S>

    internal abstract fun block(): B

    internal abstract fun repeated(record: RecordName): R

    internal abstract fun conditional(): C

    /** Declares the name of [handle], which must be new in this builder, and defines it. */
    private fun <H : Field<*>> define(handle: H, statement: (H) -> Unit): H {
        session.requireOpen(this)
        statement(Iris.declare(handle))
        return handle
    }

    private fun <T> statement(statement: () -> T): T {
        session.requireOpen(this)
        return statement()
    }

    // Declarations

    /** `name: type`, an integer at this block's depth. */
    public fun read(name: String, type: IntType): IntField<I> =
        define(ints(name)) { Iris.read(it, type) }

    /** `name: type`, a float at this block's depth. */
    public fun read(name: String, type: FloatType): FloatField<F> =
        define(floats(name)) { Iris.read(it, type) }

    /** `name: type`, an integer or float at this block's depth. */
    public fun read(name: String, type: NumType): NumField<N> =
        define(nums(name)) { Iris.read(it, type) }

    /** `name: type`, a string at this block's depth. */
    public fun read(name: String, type: StringType): StringField<S> =
        define(strings(name)) { Iris.read(it, type) }

    /**
     * `x: type` through a handle, for a type with dimensions or a name defined elsewhere, as
     * [Iris.read].
     *
     * @return the handle of the name as this definition leaves it
     */
    public fun read(target: Field<*>, type: Type<*>): DynField = statement {
        Iris.read(target, type)
    }

    /** `_: type`, which reads a value and keeps nothing. */
    public fun read(target: Discard, type: Type<*>): Unit = statement { Iris.read(target, type) }

    /** `x: type until end`, a list, as [Iris.list]. */
    public fun list(target: Field<*>, reading: ListReading): DynField = statement {
        Iris.list(target, reading)
    }

    /** `_: type until end`, which reads a list and keeps nothing. */
    public fun list(target: Discard, reading: ListReading): Unit = statement {
        Iris.list(target, reading)
    }

    // Captures

    /**
     * A capture of an integer, for a line pattern: `val n by capture(atLeast(1))` names the entry
     * after the property, or after [name] if given. The handle in a pattern prints `{n:1..}`.
     */
    public fun capture(type: IntType, name: String? = null): Capture<IntField<I>> =
        Capture(session, type, name, ::ints)

    /** A capture of a float, as the integer one. */
    public fun capture(type: FloatType, name: String? = null): Capture<FloatField<F>> =
        Capture(session, type, name, ::floats)

    /** A capture of an integer or float, as the integer one. */
    public fun capture(type: NumType, name: String? = null): Capture<NumField<N>> =
        Capture(session, type, name, ::nums)

    /** A capture of a string, as the integer one. */
    public fun capture(type: StringType, name: String? = null): Capture<StringField<S>> =
        Capture(session, type, name, ::strings)

    /** A capture of an integer that may be absent, such as one in an optional arm. */
    public fun optionalCapture(type: IntType, name: String? = null): Capture<IntField<I?>> =
        Capture(session, type, name) { ints(it).optional() }

    /** A capture of a float that may be absent. */
    public fun optionalCapture(type: FloatType, name: String? = null): Capture<FloatField<F?>> =
        Capture(session, type, name) { floats(it).optional() }

    /** A capture of an integer or float that may be absent. */
    public fun optionalCapture(type: NumType, name: String? = null): Capture<NumField<N?>> =
        Capture(session, type, name) { nums(it).optional() }

    /** A capture of a string that may be absent. */
    public fun optionalCapture(type: StringType, name: String? = null): Capture<StringField<S?>> =
        Capture(session, type, name) { strings(it).optional() }

    // Computed definitions

    /** `let name = value`, a computed integer at this block's depth. */
    public fun let(name: String, value: IntExpr): IntField<I> =
        define(ints(name)) { Iris.let(it, value) }

    /** `let name = value`, a computed float at this block's depth. */
    public fun let(name: String, value: FloatExpr): FloatField<F> =
        define(floats(name)) { Iris.let(it, value) }

    /** `let name = value`, a computed integer or float at this block's depth. */
    public fun let(name: String, value: NumExpr): NumField<N> =
        define(nums(name)) { Iris.let(it, value) }

    /** `let name = value`, a computed string at this block's depth. */
    public fun let(name: String, value: StringExpr): StringField<S> =
        define(strings(name)) { Iris.let(it, value) }

    /** `let name = value`, whose type and depth the data decide. */
    public fun let(name: String, value: DynExpr): DynField =
        define(Iris.dynamic(name)) { Iris.let(it, value) }

    /**
     * `let x = value` through a handle, as [Iris.let].
     *
     * @return the handle of the name as this definition leaves it
     */
    public fun let(target: Field<*>, value: Expr): DynField = statement { Iris.let(target, value) }

    /** `let _ = value`, which evaluates [value] without storing it. */
    public fun let(target: Discard, value: Expr): Unit = statement { Iris.let(target, value) }

    /** `param name`, whose value and type the caller decides. */
    public fun param(name: String): DynField = define(Iris.dynamic(name)) { Iris.param(it) }

    /** `param name = default`, an integer. */
    public fun param(name: String, default: IntExpr): IntField<I> =
        define(ints(name)) { Iris.param(it, default) }

    /** `param name = default`, a float. */
    public fun param(name: String, default: FloatExpr): FloatField<F> =
        define(floats(name)) { Iris.param(it, default) }

    /** `param name = default`, an integer or float. */
    public fun param(name: String, default: NumExpr): NumField<N> =
        define(nums(name)) { Iris.param(it, default) }

    /** `param name = default`, a string. */
    public fun param(name: String, default: StringExpr): StringField<S> =
        define(strings(name)) { Iris.param(it, default) }

    /** `param p` through a handle, as [Iris.param]. */
    public fun param(target: Field<*>): DynField = statement { Iris.param(target) }

    /** `param p = default` through a handle, as [Iris.param]. */
    public fun param(target: Field<*>, default: Expr): DynField = statement {
        Iris.param(target, default)
    }

    /** `let x = value` through a handle, keeping its type and depth, as [Iris.set]. */
    public fun set(handle: IntField<*>, value: IntExpr): Unit = statement {
        Iris.set(handle, value)
    }

    /** `let x = value` through a handle, keeping its type and depth. */
    public fun set(handle: IntField<*>, value: Long): Unit = statement { Iris.set(handle, value) }

    /** `let x = value` through a handle, keeping its type and depth. */
    public fun set(handle: FloatField<*>, value: FloatExpr): Unit = statement {
        Iris.set(handle, value)
    }

    /** `let x = value` through a handle, keeping its type and depth. */
    public fun set(handle: FloatField<*>, value: Double): Unit = statement {
        Iris.set(handle, value)
    }

    /** `let x = value` through a handle, keeping its type and depth. */
    public fun set(handle: NumField<*>, value: NumExpr): Unit = statement {
        Iris.set(handle, value)
    }

    /** `let x = value` through a handle, keeping its type and depth. */
    public fun set(handle: StringField<*>, value: StringExpr): Unit = statement {
        Iris.set(handle, value)
    }

    /** `let x = value` through a handle, keeping its type and depth. */
    public fun set(handle: StringField<*>, value: String): Unit = statement {
        Iris.set(handle, value)
    }

    /** `let x = value` through a dynamic handle. */
    public fun set(handle: DynField, value: Expr): Unit = statement { Iris.set(handle, value) }

    // Names

    /** Declares an integer entry at depth 0 that a later statement defines, as [Iris.forward]. */
    public fun forwardInt(name: String): IntField<Long> = statement {
        Iris.forward(name, dev.gecode.iris.build.Int::class.java)
    }

    /** Declares a float entry at depth 0 that a later statement defines. */
    public fun forwardFloat(name: String): FloatField<Double> = statement {
        Iris.forward(name, dev.gecode.iris.build.Real::class.java)
    }

    /** Declares an integer or float entry at depth 0 that a later statement defines. */
    public fun forwardNum(name: String): NumField<Number> = statement {
        Iris.forward(name, dev.gecode.iris.build.Num::class.java)
    }

    /**
     * Declares a string entry at depth 0 that a later statement defines; `forwardString("_")` is
     * the entry that `store _` keeps.
     */
    public fun forwardString(name: String): StringField<String> = statement {
        Iris.forward(name, dev.gecode.iris.build.Str::class.java)
    }

    /** Declares an entry of any type and depth that a later statement defines. */
    public fun forwardDynamic(name: String): DynField = statement {
        Iris.forward(name, dev.gecode.iris.build.Dyn::class.java)
    }

    /** Declares a name that is deliberately left undefined, as [Iris.undefined]. */
    public fun undefined(name: String): DynField = statement { Iris.undefined(name) }

    /** `type name = type`, a named integer type. */
    public fun defineType(name: String, type: IntType): IntType = statement {
        Iris.defineType(name, type)
    }

    /** `type name = type`, a named float type. */
    public fun defineType(name: String, type: FloatType): FloatType = statement {
        Iris.defineType(name, type)
    }

    /** `type name = type`, a named number type. */
    public fun defineType(name: String, type: NumType): NumType = statement {
        Iris.defineType(name, type)
    }

    /** `type name = type`, a named string type. */
    public fun defineType(name: String, type: StringType): StringType = statement {
        Iris.defineType(name, type)
    }

    // Statements

    /** `check condition`. */
    public fun check(condition: Expr): Unit = statement { Iris.check(condition) }

    /** `check condition "message"`. */
    public fun check(condition: Expr, message: String): Unit = statement {
        Iris.check(condition, message)
    }

    /** `expect "word" ...`. */
    public fun expect(word: String, vararg more: String): Unit = statement {
        Iris.expect(word, *more)
    }

    /**
     * `line "pattern"`: text, handles from capture delegates, and the parts of [ref], [capture],
     * [validate], [blank], [discard] and [restOfLine].
     */
    public fun line(vararg parts: Any): Unit = statement { Iris.line(session.pattern(parts)) }

    /** `find "pattern"`, with the parts that [line] takes. */
    public fun find(vararg parts: Any): Unit = statement { Iris.find(session.pattern(parts)) }

    /** `skip line`. */
    public fun skipLine(): Unit = statement { Iris.skipLine() }

    /** `skip count lines`. */
    public fun skipLines(count: Expr): Unit = statement { Iris.skipLines(count) }

    /** `skip count lines`. */
    public fun skipLines(count: Long): Unit = statement { Iris.skipLines(count) }

    /** `index position`. */
    public fun index(position: Expr): Unit = statement { Iris.index(position) }

    /** `index position`. */
    public fun index(position: Long): Unit = statement { Iris.index(position) }

    // Blocks

    /**
     * `repeat count { ... }`. The body's receiver is one level deeper, and its `record` is
     * [record], which names the records for projection and comprehensions.
     *
     * @return what [body] returns
     */
    public fun <T> repeat(count: Expr, record: RecordName = Iris.recordName(), body: R.() -> T): T =
        session.nest(this, repeated(record), { Iris.repeat(count, record, it) }, body)

    /** `repeat count { ... }`, as the other `repeat`. */
    public fun <T> repeat(count: Long, record: RecordName = Iris.recordName(), body: R.() -> T): T =
        session.nest(this, repeated(record), { Iris.repeat(count, record, it) }, body)

    /** `repeat until eof { ... }`. */
    public fun <T> repeatUntilEof(record: RecordName = Iris.recordName(), body: R.() -> T): T =
        session.nest(this, repeated(record), { Iris.repeatUntilEof(record, it) }, body)

    /** `repeat until "end" ... { ... }`, with an optional `store`, as [until] makes it. */
    public fun <T> repeatUntil(
        until: Until,
        record: RecordName = Iris.recordName(),
        body: R.() -> T,
    ): T = session.nest(this, repeated(record), { Iris.repeatUntil(until, record, it) }, body)

    /**
     * `sections ... { ... }`, with [until] and [skipOther] clauses in the order given. The body
     * gives the arms.
     */
    public fun <T> sections(vararg clauses: SectionClause, body: Sections<B, C>.() -> T): T {
        val list = clauses.toList()
        return session.nest(
            this,
            Sections(session, ::block, ::conditional),
            { Iris.sections(list, it) },
            body,
        )
    }

    /** `sections until ... skip other { ... }`, with the clauses as arguments. */
    public fun <T> sections(
        vararg until: Until,
        skipOther: Boolean,
        body: Sections<B, C>.() -> T,
    ): T =
        sections(
            clauses = if (skipOther) arrayOf(*until, Iris.skipOther()) else arrayOf(*until),
            body = body,
        )

    /** `if condition { ... }`, whose definitions may be absent. */
    public fun <T> ifThen(condition: Expr, body: C.() -> T): T =
        session.nest(this, conditional(), { Iris.ifThen(condition, it) }, body)

    /** `else if condition { ... }`, right after [ifThen] or another `elseIf`. */
    public fun <T> elseIf(condition: Expr, body: C.() -> T): T =
        session.nest(this, conditional(), { Iris.elseIf(condition, it) }, body)

    /** `else { ... }`, right after [ifThen] or [elseIf]. */
    public fun <T> otherwise(body: C.() -> T): T =
        session.nest(this, conditional(), { Iris.otherwise(it) }, body)

    /**
     * `else if:type`, the declaration of the name `if` as the else branch, as [Iris.elseReadIf].
     */
    public fun elseReadIf(target: Field<*>, type: Type<*>): DynField = statement {
        Iris.elseReadIf(target, type)
    }
}

/** The arms of `sections`, the receiver of its lambda. */
@IrisDsl
@TrackCaller
public class Sections<B : Any, C : Any>
internal constructor(
    private val session: Session,
    private val block: () -> B,
    private val conditional: () -> C,
) {
    private fun options(optional: Boolean, last: Boolean) =
        listOfNotNull(Iris.optional().takeIf { optional }, Iris.last().takeIf { last })

    /** An arm with no body; its parts are those that [Scope.line] takes. */
    public fun arm(vararg parts: Any, optional: Boolean = false, last: Boolean = false) {
        session.requireOpen(this)
        val pattern = session.pattern(parts)
        val options = options(optional, last)
        when (options.size) {
            0 -> Iris.arm(pattern)
            1 -> Iris.arm(pattern, options[0])
            else -> Iris.arm(pattern, options[0], options[1])
        }
    }

    /** An arm whose body always runs when the arm is required. */
    public fun <T> arm(vararg parts: Any, last: Boolean = false, body: B.() -> T): T =
        armWith(session.pattern(parts), options(false, last), block(), body)

    /** An `optional` arm, whose definitions may be absent. */
    public fun <T> optionalArm(vararg parts: Any, last: Boolean = false, body: C.() -> T): T =
        armWith(session.pattern(parts), options(true, last), conditional(), body)

    private fun <X : Any, T> armWith(
        pattern: Pattern,
        options: List<ArmOption>,
        inner: X,
        body: X.() -> T,
    ): T =
        session.nest(
            this,
            inner,
            { run ->
                when (options.size) {
                    0 -> Iris.arm(pattern, run)
                    1 -> Iris.arm(pattern, options[0], run)
                    else -> Iris.arm(pattern, options[0], options[1], run)
                }
            },
            body,
        )
}
