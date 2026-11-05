# java

The pure-Java library `dev.gecode:iris` (Java 27, module `dev.gecode.iris`)
and the Kotlin extensions module `dev.gecode:iris-kotlin`. `PLAN.md` §8.3
describes both; milestone M4d builds them.

The library parses reader specs, applies every parse-time check of the
Specification, reads input into columnar data under the work budgets of chapter
10, lists the instances of an input, and writes dzn. `iris-kotlin` adds Kotlin
extensions; [its README](iris-kotlin/README.md) shows them.

```java
import dev.gecode.iris.Data;
import dev.gecode.iris.IrisException;
import dev.gecode.iris.Params;
import dev.gecode.iris.Spec;

try {
  Spec spec = Spec.parse(source, "sat.iris");  // byte[] or String
  Data data = spec.read(input, "sat.cnf", Params.of("instance", "2"));
  long vars = data.get("vars").asLong();
  long[] first = data.get("clauses").get(0).longs();  // a copy
  String document = data.toJson();
  String dzn = data.toDzn();
  Data catalog = spec.catalog(input, "sat.cnf");  // the entries instances, name and failed
  long instances = catalog.get("instances").asLong();
} catch (IrisException e) {
  e.code();                  // ErrorCode.EXPECTED_VALUE, whose code() is "E-EXPECTED-VALUE"
  e.origin();                // origin, line and column of the input if the error has them,
  e.line();                  // and otherwise of the reader spec; any of them can be null
  e.column();
  e.path();                  // the indices of a failed check's witness, or null
  e.error().inputLocation(); // origin, line and column of the input
  e.error().specLocation();  // origin, line and, for E-SPEC-UTF8, column of the reader spec
}
```

`read` and `catalog` also take a `Path`, whose text is the origin. They then wrap
an `IOException` in an `UncheckedIOException`. Asking a `Value` for something it
is not throws `IllegalStateException` or `IndexOutOfBoundsException`, never
`IrisException`, as that is a mistake in the calling code. `ints()` throws
`ArithmeticException` for an integer that does not fit an `int`.

`longs()`, `ints()` and `doubles()` return a new array on each call, and
`strings()` an unmodifiable list. Values are equal if they have the same type,
shape and leaves. The comparison walks the levels of an array in a loop, so a
value of any depth can be compared and hashed.

`Value` and `IrisError` are sealed, so a `switch` over them needs no default.
`dev.gecode.iris` and the module are both `@NullMarked`: the package annotation
covers tools that do not read `module-info`, such as those on the class path.

## Layout

| Path | Contents |
|---|---|
| `iris/` | The library. Only `dev.gecode.iris` is exported; `dev.gecode.iris.internal.syntax` holds the lexer, parser and resolver, and `dev.gecode.iris.internal.read` the reader. |
| `iris-driver/` | The conformance driver, which is not published. It implements `parse`, `read`, `catalog` (`--format json|dzn`) and `dzn`. The JVM decodes arguments in the locale's encoding before `main` runs, so outside a UTF-8 locale the driver answers exit status 3 for non-ASCII `--params` instead of reading changed values; JSON escapes work in any locale. |
| `iris-kotlin/` | The Kotlin extensions, in the exported package `dev.gecode.iris.kotlin` of the module of that name. |
| `build-logic/` | The convention plugins `iris.java-conventions`, shared by the modules, and `iris.kotlin-conventions`. |
| `gradle/libs.versions.toml` | The pinned versions. |
| `detekt.yml` | The detekt configuration: what differs from the default rules. |

The legal nesting of §5.12 needs up to about 900 KiB of stack on a JVM, so
`Spec.parse`, `Spec.read` and `Spec.catalog` run on a shared pool of threads
with 4 MiB stacks (`LargeStack`). A legal spec parses and reads whatever stack the caller has.

§5.12 bounds parsing, not the tree. Left operands can chain for tens of
thousands of nodes, as in `(((1+1)+1)+1)`, and aliases can chain to any length.
A walk of an expression therefore loops over `Expr.leftSpine` and recurses only
into the other operands, which hold a counter entry. A use of a named type keeps
its reading kind and refers to its definition by number, so it has no chain to
follow.

Nor does anything bound the dimensions of a value. A value is a view of a
`Table`: flat leaves and one array of offsets per level. Values are made on
access, and nothing that walks them recurses.

`Budget` keeps the units left of the two limits of §10.1, which stay below 2^36.
A charge is compared whole with what is left, so no charge can overflow a
`long`. Every site of §10.2 to §10.4 charges before it allocates what the charge
pays for, so a huge repeat or array of empty rows fails at the limit instead of
running out of time or memory.

## Building and testing

The build needs Java 27 as `java` on `PATH` or in `JAVA_HOME`, and stable Rust
with `cargo` on `PATH`, since the conformance test runs the Rust runner. It
fails rather than skips without `cargo`.

```sh
./gradlew build          # compile, spotlessCheck, detekt, all tests, conformance, and javadoc
./gradlew spotlessApply  # rewrite the Java and Kotlin sources
./gradlew conformance    # every conformance case, through the installed driver
./gradlew :iris:javadoc  # the API reference, in iris/build/docs/javadoc
./gradlew :iris:test --tests dev.gecode.iris.BudgetLimitsTest
./gradlew :iris:test --tests dev.gecode.iris.FloatDifferentialTest
./gradlew :iris-kotlin:check  # the Kotlin tests, detekt and the dependency check
```

`-Xlint:all -Werror` applies to main and test sources. Error Prone and NullAway
run inside `compileJava`, so they have no task of their own: any Error Prone
warning fails the build through `-Werror`, and NullAway reports at error level
for `@NullMarked` code, which is all of `dev.gecode.iris`.

`iris-kotlin` compiles with `allWarningsAsErrors`, and detekt and ktfmt run in
`check`; [its README](iris-kotlin/README.md) has the details.

`:iris:javadoc` documents only the exported package and fails on any warning, a
missing comment included. The build runs it through `check`. `ApiTest` checks the
module descriptor, that public signatures name no internal type, and the error
fields.

`./gradlew conformance` selects every case in `conformance/cases/0.1` and
`formats`, runs them with the installed driver through the Rust runner, and
prints the counts per operation and the totals. It has no driver ID and no
exemptions, and it fails on any failing case or an operation without cases.
`./gradlew test` leaves it out and `./gradlew build` runs it once. Add `--rerun`
to repeat it when nothing changed.

`BudgetLimitsTest` charges a known cost at each site of §10.2 to §10.4 and uses
up the rest of the limit: the spec that ends exactly on the limit must pass for
`read` and `catalog`, and the one with a unit more must fail with `E-BUDGET`.

`FloatDifferentialTest` compares `Spec.read` with the Rust `iris read --json`
and `iris read --dzn` byte for byte on a million floats from a fixed seed, and
checks that each printed value parses to the same double. `-PirisFloatValues=N` sets the number
and `-PirisCli=PATH` runs a given `iris` binary instead of `cargo run`.

By hand from the root, after `./gradlew :iris-driver:installDist` in `java/`:

```sh
(cd rust && cargo run --locked -p iris-conformance -- run --driver "\"$PWD/../java/iris-driver/build/install/iris-driver/bin/iris-driver\"" ../conformance/cases/0.1)
```

Run the same command with `../formats` for the format cases.

## Pinned tools

Gradle 9.8.2 (wrapper), Spotless 8.11.0 with google-java-format 2.0.0, Error
Prone 2.52.0, NullAway 0.14.3, JSpecify 1.0.1, JUnit 5.14.4, Kotlin 2.4.21, ktfmt
0.64 and detekt 2.0.0-alpha.6.

### Java 27 gaps

As of 2026-11-04 every tool runs on Java 27, and no check is off. The Kotlin
compiler inside Gradle has no JVM 27 target, so compiling `build-logic` warns
and targets JVM 26; this affects only the build logic. Gradle may also print
`Did not receive connection preamble within 10s` during a `--no-daemon` run,
which is noise unless the build fails.

`iris-kotlin` has the same gap. Kotlin 2.4.21 has no JVM 27 target, so its
classes target JVM 26 (class file version 70), while `module-info.class` and
`dev.gecode:iris` are version 71. The build turns off the Gradle plugin's check
that the Java and Kotlin targets match. The module still needs Java 27, as
`dev.gecode:iris` does. The compiler reads the Java 27 classes and the
`module-info` of `dev.gecode.iris`, and rejects a use of a package that module
does not export.

detekt has no stable release for Kotlin 2.4: the 1.x line stops at 1.23.8, so
`2.0.0-alpha.6`, the newest release, is pinned. It is built against Kotlin
2.4.10 and analyzes this code, with type resolution, without complaint.
