@file:TrackCaller

package dev.gecode.iris.kotlin.build

import dev.gecode.iris.Field
import dev.gecode.iris.Spec
import dev.gecode.iris.build.Builder
import dev.gecode.iris.build.Iris
import dev.gecode.iris.build.TrackCaller
import dev.gecode.iris.build.Type
import kotlin.properties.PropertyDelegateProvider
import kotlin.properties.ReadOnlyProperty
import kotlin.reflect.KProperty

/** The top level of a spec: a block at depth 0 that also takes the settings and `instances`. */
@TrackCaller
public class SpecScope internal constructor(session: Session) : Scope0(session) {
    private fun setting(statement: () -> Unit) {
        session.requireOpen(this)
        statement()
    }

    /** `format name`, a name of this format. */
    public fun format(name: String): Unit = setting { Iris.format(name) }

    /** `comment "marker" ...`. */
    public fun comment(marker: String, vararg more: String): Unit = setting {
        Iris.comment(marker, *more)
    }

    /** `stop "marker" ...`. */
    public fun stop(marker: String, vararg more: String): Unit = setting {
        Iris.stop(marker, *more)
    }

    /** `tokens "characters"`. */
    public fun tokens(characters: String): Unit = setting { Iris.tokens(characters) }

    /** `separators "characters"`. */
    public fun separators(characters: String): Unit = setting { Iris.separators(characters) }

    /** `ignore trailing`. */
    public fun ignoreTrailing(): Unit = setting { Iris.ignoreTrailing() }

    /**
     * `instances { ... }`, the statements of one instance.
     *
     * @return what [body] returns
     */
    public fun <T> instances(body: Scope0.() -> T): T =
        session.nest(this, Scope0(session), { Iris.instances(it) }, body)
}

/**
 * A capture for a line pattern, made by `capture` and `optionalCapture` of a scope. As a property
 * delegate it names the entry after the property, or after the name given to `capture`, and
 * declares that name, which must be new in the builder, as [Iris.declare].
 */
@TrackCaller
public class Capture<H : Field<*>>
internal constructor(
    private val session: Session,
    private val type: Type<*>,
    private val name: String?,
    private val handle: (String) -> H,
) : PropertyDelegateProvider<Any?, ReadOnlyProperty<Any?, H>> {
    /** Declares the entry [name] and returns its handle, which prints as a capture in a pattern. */
    public fun named(name: String): H =
        Iris.declare(handle(name)).also { session.captures[it] = type }

    override fun provideDelegate(thisRef: Any?, property: KProperty<*>): ReadOnlyProperty<Any?, H> {
        val made = named(name ?: property.name)
        return ReadOnlyProperty { _, _ -> made }
    }
}

/** A built spec and the handles that read its data. */
public data class IrisSpec<out H>(
    /** The spec. */
    public val spec: Spec,
    /** What the builder's lambda returned, usually its handles. */
    public val handles: H,
)

/** A builder's text, before it is parsed, and the handles that will read its data. */
@TrackCaller
public class IrisBuilder<out H>
internal constructor(
    /** The Java builder. */
    public val builder: Builder,
    /** What the builder's lambda returned, usually its handles. */
    public val handles: H,
) {
    /** Returns the text of the reader spec, as [Builder.source]. */
    public fun source(): String = builder.source()

    /**
     * Checks the whole spec and parses its text, as [Builder.build].
     *
     * @throws dev.gecode.iris.build.BuildException if a check fails
     */
    public fun build(): IrisSpec<H> = IrisSpec(builder.build(), handles)
}

/**
 * Builds a reader spec: [body] adds its statements, and what it returns comes back as the handles.
 * With [format], the spec starts with `format format`.
 *
 * @throws dev.gecode.iris.build.BuildException if a statement fails a check
 */
public fun <H> irisBuilder(format: String? = null, body: SpecScope.() -> H): IrisBuilder<H> {
    val session = Session()
    val results = ArrayList<H>(1)
    val run = Runnable {
        val scope = SpecScope(session)
        session.open = scope
        try {
            results.add(session.activate { scope.body() })
        } finally {
            session.open = null
        }
    }
    val builder = if (format == null) Iris.builder(run) else Iris.builder(format, run)
    return IrisBuilder(builder, results.single())
}

/**
 * Builds and checks a reader spec, as [irisBuilder] followed by [IrisBuilder.build].
 *
 * @throws dev.gecode.iris.build.BuildException if a check fails
 */
public fun <H> irisSpec(format: String? = null, body: SpecScope.() -> H): IrisSpec<H> =
    irisBuilder(format, body).build()
