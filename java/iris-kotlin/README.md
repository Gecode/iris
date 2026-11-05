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

Delegated properties and the kotlinx.serialization decoder of §8.3 come later.

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
