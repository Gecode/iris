@file:TrackCaller
@file:Suppress(
    "TooManyFunctions"
) // Handle factories and record names, so that a builder needs no import from the Java package.

package dev.gecode.iris.kotlin.build

import dev.gecode.iris.Value
import dev.gecode.iris.build.DynField
import dev.gecode.iris.build.FloatField
import dev.gecode.iris.build.IntField
import dev.gecode.iris.build.Iris
import dev.gecode.iris.build.NumField
import dev.gecode.iris.build.RecordName
import dev.gecode.iris.build.StringField
import dev.gecode.iris.build.TrackCaller

/** An integer handle at depth 0, as [Iris.int0]. */
public fun int0(name: String): IntField<Long> = Iris.int0(name)

/** An integer handle at depth 1, as [Iris.int1]. */
public fun int1(name: String): IntField<LongArray> = Iris.int1(name)

/** An integer handle at depth 2, as [Iris.int2]. */
public fun int2(name: String): IntField<List<LongArray>> = Iris.int2(name)

/** An integer handle at depth 3, as [Iris.int3]. */
public fun int3(name: String): IntField<List<List<LongArray>>> = Iris.int3(name)

/** An integer handle at any [depth], which reads a [Value], as [Iris.intN]. */
public fun intN(name: String, depth: Int): IntField<Value> = Iris.intN(name, depth)

/** A float handle at depth 0, as [Iris.float0]. */
public fun float0(name: String): FloatField<Double> = Iris.float0(name)

/** A float handle at depth 1, as [Iris.float1]. */
public fun float1(name: String): FloatField<DoubleArray> = Iris.float1(name)

/** A float handle at depth 2, as [Iris.float2]. */
public fun float2(name: String): FloatField<List<DoubleArray>> = Iris.float2(name)

/** A float handle at depth 3, as [Iris.float3]. */
public fun float3(name: String): FloatField<List<List<DoubleArray>>> = Iris.float3(name)

/** A float handle at any [depth], which reads a [Value], as [Iris.floatN]. */
public fun floatN(name: String, depth: Int): FloatField<Value> = Iris.floatN(name, depth)

/** A number handle at depth 0, as [Iris.num0]. */
public fun num0(name: String): NumField<Number> = Iris.num0(name)

/** A number handle at depth 1, as [Iris.num1]. */
public fun num1(name: String): NumField<List<Number>> = Iris.num1(name)

/** A number handle at depth 2, as [Iris.num2]. */
public fun num2(name: String): NumField<List<List<Number>>> = Iris.num2(name)

/** A number handle at depth 3, as [Iris.num3]. */
public fun num3(name: String): NumField<List<List<List<Number>>>> = Iris.num3(name)

/** A number handle at any [depth], which reads a [Value], as [Iris.numN]. */
public fun numN(name: String, depth: Int): NumField<Value> = Iris.numN(name, depth)

/** A string handle at depth 0, as [Iris.string0]. */
public fun string0(name: String): StringField<String> = Iris.string0(name)

/** A string handle at depth 1, as [Iris.string1]. */
public fun string1(name: String): StringField<List<String>> = Iris.string1(name)

/** A string handle at depth 2, as [Iris.string2]. */
public fun string2(name: String): StringField<List<List<String>>> = Iris.string2(name)

/** A string handle at depth 3, as [Iris.string3]. */
public fun string3(name: String): StringField<List<List<List<String>>>> = Iris.string3(name)

/** A string handle at any [depth], which reads a [Value], as [Iris.stringN]. */
public fun stringN(name: String, depth: Int): StringField<Value> = Iris.stringN(name, depth)

/** A handle of any type and depth, which reads a [Value], as [Iris.dynamic]. */
public fun dynamic(name: String): DynField = Iris.dynamic(name)

/** A record name, printed with names the builder chooses. */
public fun recordName(): RecordName = Iris.recordName()

/** A record name, printed as `each` [each] and `as` [list]. */
public fun recordName(each: String, list: String): RecordName = Iris.recordName(each, list)
