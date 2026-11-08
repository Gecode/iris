# iris-kotlin

Kotlin extensions for `dev.gecode:iris`, published as `dev.gecode:iris-kotlin`
(JPMS module `dev.gecode.iris.kotlin`). `PLAN.md` §8.3 describes the module;
milestone M4d builds it.

```kotlin
import dev.gecode.iris.Spec
import dev.gecode.iris.kotlin.*

val spec = Spec.parse(source, "sat.iris")
val data = spec.read(input, "sat.cnf", mapOf("instance" to "2").toParams())

val vars = data["vars"].asLong()
val literal = data["clause"][3][0].asLong()
val same = data["clause"][3, 0]
val clause: IntArray = data["clause"][3].toIntArray()
val longest = data["clause"].asSequence().maxOf { it.size() }
val literals = data["clause"].leaves().count()
```

| Extension | Result |
|---|---|
| `value[i, j, ...]` | What `Value.at` selects. A name and a single index need no extension: Kotlin calls the Java `get` methods as operators. |
| `value.toIntArray()` | A new `IntArray`, or `ArithmeticException` for an integer that does not fit an `Int`, as `Value.ints` does. |
| `value.toLongArray()` | A new `LongArray`. |
| `value.toDoubleArray()` | A new `DoubleArray`. An integer becomes the nearest `Double`, as in `Value.doubles`, so one above 2^53 can change. |
| `value.asSequence()` | The elements of an array, made as the sequence reaches them. |
| `value.leaves()` | The single values at any depth, in row order. It keeps its place in a list rather than recursing, so a value of any number of dimensions works. |
| `data.asSequence()` | The entries, in definition order. |
| `map.toParams()` | `Params` in the map's iteration order. |

Misuse throws what the Java API throws, never `IrisException`:
`IllegalStateException` or `IndexOutOfBoundsException`.

The Java API is `@NullMarked`, and the compiler runs with
`-Xjspecify-annotations=strict`, so Kotlin sees results marked `@Nullable`, such
as `Data.find`, as nullable, every other result as non-null, and a nullability
mismatch as an error.
`NullabilityTest` compiles snippets with the module's own compiler arguments to
show this.

## Module

The module has its own `module-info.java`, because `dev.gecode:iris` is a named
module. Without one the jar would be an automatic module, whose name comes from
the file name and which reads everything. The descriptor gives Java and Kotlin
users on the module path a stable name and the exact requirements, and it makes
the Kotlin compiler reject a use of a package `dev.gecode.iris` does not export.
It requires both modules transitively, since signatures here name `Value` and
`Sequence`. `ModuleTest` resolves the built jar.

The module has only two direct dependencies, the Kotlin standard library and
`dev.gecode:iris`. They bring `org.jspecify:jspecify` and
`org.jetbrains:annotations`, which hold only annotations. The task
`checkDependencies`, part of `check`, fails on anything else on the compile or
runtime class path.

## Spec builder

`dev.gecode.iris.kotlin.build` is a type-safe builder over the Java builder
`dev.gecode.iris.build`, with lambdas with receivers marked `@DslMarker`:

```kotlin
import dev.gecode.iris.kotlin.build.*

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
```

Each block's receiver has a class for its depth and optionality, `Scope0` to
`Scope3`, `OptionalScope0` to `OptionalScope3` and `DeepScope` beyond, so a read
returns a handle of the exact view type, such as `IntField<LongArray>` in a
`repeat`. The operators are Kotlin's where they mean the same as in Iris: `+`,
`-`, `*`, `/`, `%`, unary `-` and `!`, and the ranges `lo..hi` and `lo..<hi`.
Comparisons and logic are the infix functions `lt`, `le`, `gt`, `ge`, `eq`,
`ne`, `and` and `or`. `x[field]` is a record's field and `x.at(i)` an index.
Captures are delegated properties, `val n by capture(int())`. A definition
that takes a name, such as `read("n", int())`, `let`, `param` or a capture,
declares it, so a name that the builder has already declared is an error, as
for a helper called twice with one name; redefinition goes through the handle,
as in `read(n, float())`. A helper takes the scope as a context parameter. `irisBuilder` returns the text and the
handles without parsing, and `build()` parses. Every statement calls the Java
builder, so the checks and their errors are Java's, and an error names the
caller's line in Kotlin. `forwardInt`, `forwardDynamic`, `undefined`, `dynamic`
and `.dynamic()` are the dynamic forms.
[docs/builders.md](../../docs/builders.md#kotlin) has the full example.

`iris gen builder --lang kotlin SPEC` prints a program that builds a text spec
with this builder, and `iris gen --lang kotlin SPEC` prints an object with a
typed handle for each entry of a text spec, such as
`public val machine: IntField<List<LongArray>> = int2("machine")`, read as
`data[Jobshop.machine]`. The factories it calls, `int0` to `int3`, `intN`,
`dynamic` and their kin, are in this package. `java/iris-views` compiles the
objects of every format and reads each case through them.

`java/iris-examples` has a builder for each format in `formats/`, in
`src/main/kotlin/examples`, beside the Java ones; `FormatBuildersTest` reads
every case of each format through them.

The kotlinx.serialization decoder of §8.3 comes later.

## Building and testing

In `java/`:

```sh
./gradlew :iris-kotlin:check          # tests, detekt, Dokka and checkDependencies
./gradlew :iris-kotlin:dokkaGenerate  # the API reference, in build/dokka/html
./gradlew :iris-kotlin:spotlessCheck  # ktfmt in kotlinlang style; spotlessApply rewrites
```

The compiler runs in explicit API mode with `allWarningsAsErrors`. detekt uses
`java/detekt.yml` on the default rules, with type resolution, and fails on a
finding of any severity. The versions, and what Kotlin lacks for Java 27, are in
[the Java README](../README.md#pinned-tools).
