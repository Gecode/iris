@file:TrackCaller

package dev.gecode.iris.kotlin.build

import dev.gecode.iris.Value
import dev.gecode.iris.build.FloatField
import dev.gecode.iris.build.IntField
import dev.gecode.iris.build.Iris
import dev.gecode.iris.build.NumField
import dev.gecode.iris.build.RecordName
import dev.gecode.iris.build.StringField
import dev.gecode.iris.build.TrackCaller

/** A block at depth 0 whose statements always run, such as the body of `instances`. */
@TrackCaller
public open class Scope0 internal constructor(override val session: Session) :
    Scope<Long, Double, Number, String, Scope0, Scope1, OptionalScope0>() {
    override fun ints(name: String): IntField<Long> = Iris.int0(name)

    override fun floats(name: String): FloatField<Double> = Iris.float0(name)

    override fun nums(name: String): NumField<Number> = Iris.num0(name)

    override fun strings(name: String): StringField<String> = Iris.string0(name)

    override fun block(): Scope0 = Scope0(session)

    override fun repeated(record: RecordName): Scope1 = Scope1(session, record)

    override fun conditional(): OptionalScope0 = OptionalScope0(session)
}

/** A block at depth 0 whose statements may not run, such as the body of `if`. */
@TrackCaller
public class OptionalScope0 internal constructor(override val session: Session) :
    Scope<Long?, Double?, Number?, String?, OptionalScope0, OptionalScope1, OptionalScope0>() {
    override fun ints(name: String): IntField<Long?> = Iris.int0(name).optional()

    override fun floats(name: String): FloatField<Double?> = Iris.float0(name).optional()

    override fun nums(name: String): NumField<Number?> = Iris.num0(name).optional()

    override fun strings(name: String): StringField<String?> = Iris.string0(name).optional()

    override fun block(): OptionalScope0 = OptionalScope0(session)

    override fun repeated(record: RecordName): OptionalScope1 = OptionalScope1(session, record)

    override fun conditional(): OptionalScope0 = OptionalScope0(session)
}

/** The body of a repeat at depth 1, whose records [record] names. */
@TrackCaller
public class Scope1
internal constructor(override val session: Session, public val record: RecordName) :
    Scope<LongArray, DoubleArray, List<Number>, List<String>, Scope1, Scope2, OptionalScope1>() {
    override fun ints(name: String): IntField<LongArray> = Iris.int1(name)

    override fun floats(name: String): FloatField<DoubleArray> = Iris.float1(name)

    override fun nums(name: String): NumField<List<Number>> = Iris.num1(name)

    override fun strings(name: String): StringField<List<String>> = Iris.string1(name)

    override fun block(): Scope1 = Scope1(session, record)

    override fun repeated(record: RecordName): Scope2 = Scope2(session, record)

    override fun conditional(): OptionalScope1 = OptionalScope1(session, record)
}

/** A block at depth 1 whose statements may not run. */
@TrackCaller
public class OptionalScope1
internal constructor(override val session: Session, public val record: RecordName) :
    Scope<
        LongArray?,
        DoubleArray?,
        List<Number>?,
        List<String>?,
        OptionalScope1,
        OptionalScope2,
        OptionalScope1,
    >() {
    override fun ints(name: String): IntField<LongArray?> = Iris.int1(name).optional()

    override fun floats(name: String): FloatField<DoubleArray?> = Iris.float1(name).optional()

    override fun nums(name: String): NumField<List<Number>?> = Iris.num1(name).optional()

    override fun strings(name: String): StringField<List<String>?> = Iris.string1(name).optional()

    override fun block(): OptionalScope1 = OptionalScope1(session, record)

    override fun repeated(record: RecordName): OptionalScope2 = OptionalScope2(session, record)

    override fun conditional(): OptionalScope1 = OptionalScope1(session, record)
}

/** The body of a repeat at depth 2, whose records [record] names. */
@TrackCaller
public class Scope2
internal constructor(override val session: Session, public val record: RecordName) :
    Scope<
        List<LongArray>,
        List<DoubleArray>,
        List<List<Number>>,
        List<List<String>>,
        Scope2,
        Scope3,
        OptionalScope2,
    >() {
    override fun ints(name: String): IntField<List<LongArray>> = Iris.int2(name)

    override fun floats(name: String): FloatField<List<DoubleArray>> = Iris.float2(name)

    override fun nums(name: String): NumField<List<List<Number>>> = Iris.num2(name)

    override fun strings(name: String): StringField<List<List<String>>> = Iris.string2(name)

    override fun block(): Scope2 = Scope2(session, record)

    override fun repeated(record: RecordName): Scope3 = Scope3(session, record)

    override fun conditional(): OptionalScope2 = OptionalScope2(session, record)
}

/** A block at depth 2 whose statements may not run. */
@TrackCaller
public class OptionalScope2
internal constructor(override val session: Session, public val record: RecordName) :
    Scope<
        List<LongArray>?,
        List<DoubleArray>?,
        List<List<Number>>?,
        List<List<String>>?,
        OptionalScope2,
        OptionalScope3,
        OptionalScope2,
    >() {
    override fun ints(name: String): IntField<List<LongArray>?> = Iris.int2(name).optional()

    override fun floats(name: String): FloatField<List<DoubleArray>?> = Iris.float2(name).optional()

    override fun nums(name: String): NumField<List<List<Number>>?> = Iris.num2(name).optional()

    override fun strings(name: String): StringField<List<List<String>>?> =
        Iris.string2(name).optional()

    override fun block(): OptionalScope2 = OptionalScope2(session, record)

    override fun repeated(record: RecordName): OptionalScope3 = OptionalScope3(session, record)

    override fun conditional(): OptionalScope2 = OptionalScope2(session, record)
}

/** The body of a repeat at depth 3, whose records [record] names. */
@TrackCaller
public class Scope3
internal constructor(override val session: Session, public val record: RecordName) :
    Scope<
        List<List<LongArray>>,
        List<List<DoubleArray>>,
        List<List<List<Number>>>,
        List<List<List<String>>>,
        Scope3,
        DeepScope,
        OptionalScope3,
    >() {
    override fun ints(name: String): IntField<List<List<LongArray>>> = Iris.int3(name)

    override fun floats(name: String): FloatField<List<List<DoubleArray>>> = Iris.float3(name)

    override fun nums(name: String): NumField<List<List<List<Number>>>> = Iris.num3(name)

    override fun strings(name: String): StringField<List<List<List<String>>>> = Iris.string3(name)

    override fun block(): Scope3 = Scope3(session, record)

    override fun repeated(record: RecordName): DeepScope = DeepScope(session, DEEP, record)

    override fun conditional(): OptionalScope3 = OptionalScope3(session, record)
}

/** A block at depth 3 whose statements may not run. */
@TrackCaller
public class OptionalScope3
internal constructor(override val session: Session, public val record: RecordName) :
    Scope<
        List<List<LongArray>>?,
        List<List<DoubleArray>>?,
        List<List<List<Number>>>?,
        List<List<List<String>>>?,
        OptionalScope3,
        OptionalDeepScope,
        OptionalScope3,
    >() {
    override fun ints(name: String): IntField<List<List<LongArray>>?> = Iris.int3(name).optional()

    override fun floats(name: String): FloatField<List<List<DoubleArray>>?> =
        Iris.float3(name).optional()

    override fun nums(name: String): NumField<List<List<List<Number>>>?> =
        Iris.num3(name).optional()

    override fun strings(name: String): StringField<List<List<List<String>>>?> =
        Iris.string3(name).optional()

    override fun block(): OptionalScope3 = OptionalScope3(session, record)

    override fun repeated(record: RecordName): OptionalDeepScope =
        OptionalDeepScope(session, DEEP, record)

    override fun conditional(): OptionalScope3 = OptionalScope3(session, record)
}

private const val DEEP = 4

/** The body of a repeat at [depth] 4 or more, whose definitions read a [Value]. */
@TrackCaller
public class DeepScope
internal constructor(
    override val session: Session,
    public val depth: Int,
    public val record: RecordName,
) : Scope<Value, Value, Value, Value, DeepScope, DeepScope, OptionalDeepScope>() {
    override fun ints(name: String): IntField<Value> = Iris.intN(name, depth)

    override fun floats(name: String): FloatField<Value> = Iris.floatN(name, depth)

    override fun nums(name: String): NumField<Value> = Iris.numN(name, depth)

    override fun strings(name: String): StringField<Value> = Iris.stringN(name, depth)

    override fun block(): DeepScope = DeepScope(session, depth, record)

    override fun repeated(record: RecordName): DeepScope = DeepScope(session, depth + 1, record)

    override fun conditional(): OptionalDeepScope = OptionalDeepScope(session, depth, record)
}

/** A block at [depth] 4 or more whose statements may not run. */
@TrackCaller
public class OptionalDeepScope
internal constructor(
    override val session: Session,
    public val depth: Int,
    public val record: RecordName,
) :
    Scope<
        Value?,
        Value?,
        Value?,
        Value?,
        OptionalDeepScope,
        OptionalDeepScope,
        OptionalDeepScope,
    >() {
    override fun ints(name: String): IntField<Value?> = Iris.intN(name, depth).optional()

    override fun floats(name: String): FloatField<Value?> = Iris.floatN(name, depth).optional()

    override fun nums(name: String): NumField<Value?> = Iris.numN(name, depth).optional()

    override fun strings(name: String): StringField<Value?> = Iris.stringN(name, depth).optional()

    override fun block(): OptionalDeepScope = OptionalDeepScope(session, depth, record)

    override fun repeated(record: RecordName): OptionalDeepScope =
        OptionalDeepScope(session, depth + 1, record)

    override fun conditional(): OptionalDeepScope = OptionalDeepScope(session, depth, record)
}
