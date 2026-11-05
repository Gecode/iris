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

## Using it from another project

`dev.gecode:iris` and `dev.gecode:iris-kotlin` are not on Maven Central yet. To
use them from another project, install both in the local Maven repository:

```sh
./gradlew publishToMavenLocal
```

A Gradle project then needs Java 27 and these lines:

```kotlin
repositories {
    mavenLocal()
    mavenCentral()
}

java { toolchain { languageVersion = JavaLanguageVersion.of(27) } }

dependencies {
    implementation("dev.gecode:iris:0.1.0")
}
```

A Maven project adds this dependency and sets `maven.compiler.release` to 27:

```xml
<dependency>
  <groupId>dev.gecode</groupId>
  <artifactId>iris</artifactId>
  <version>0.1.0</version>
</dependency>
```

For the Kotlin extensions, use `dev.gecode:iris-kotlin` in the same way:

```kotlin
dependencies {
    implementation("dev.gecode:iris-kotlin:0.1.0")
}
```

```xml
<dependency>
  <groupId>dev.gecode</groupId>
  <artifactId>iris-kotlin</artifactId>
  <version>0.1.0</version>
</dependency>
```

Kotlin has no JVM 27 target, so a Kotlin project on Java 27 needs these lines in
Gradle, as this build does (see [Java 27 gaps](#java-27-gaps)):

```kotlin
import org.jetbrains.kotlin.gradle.dsl.JvmTarget
import org.jetbrains.kotlin.gradle.dsl.jvm.JvmTargetValidationMode
import org.jetbrains.kotlin.gradle.tasks.KotlinJvmCompile

kotlin { compilerOptions { jvmTarget = JvmTarget.JVM_26 } }

tasks.withType<KotlinJvmCompile>().configureEach {
    jvmTargetValidationMode = JvmTargetValidationMode.IGNORE
}
```

The [site guide](../site/src/content/docs/using/java.mdx) shows these snippets
and the site check compares them with this file, so change both together.

## Layout

| Path | Contents |
|---|---|
| `iris/` | The library. Only `dev.gecode.iris` is exported; `dev.gecode.iris.internal.syntax` holds the lexer, parser and resolver, and `dev.gecode.iris.internal.read` the reader. |
| `iris-driver/` | The conformance driver, which is not published. It implements `parse`, `read`, `catalog` (`--format json|dzn`) and `dzn`. The JVM decodes arguments in the locale's encoding before `main` runs, so outside a UTF-8 locale the driver answers exit status 3 for non-ASCII `--params` instead of reading changed values; JSON escapes work in any locale. [Windows](#windows) has a narrower guarantee. |
| `iris-examples/` | The examples of the site's [Java and Kotlin guide](../site/src/content/docs/using/java.mdx), which are not published: sources in `src/main/java/examples` and `src/main/kotlin/examples`, each with its recorded `.out` beside it. |
| `iris-jmh/` | The JMH benchmarks, which are not published. Nothing else depends on this module, so JMH stays out of `dev.gecode:iris`. |
| `iris-kotlin/` | The Kotlin extensions, in the exported package `dev.gecode.iris.kotlin` of the module of that name. |
| `build-logic/` | The convention plugins `iris.java-conventions`, shared by the modules, `iris.kotlin-conventions`, `iris.kotlin-docs` for Dokka, and `iris.published` for the two published modules. |
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
./gradlew :iris-kotlin:dokkaGenerate  # the Kotlin API reference, in iris-kotlin/build/dokka/html
./gradlew :iris-examples:test  # run the guide's examples and compare their output
./gradlew :iris:test --tests dev.gecode.iris.BudgetLimitsTest
./gradlew :iris:test --tests dev.gecode.iris.FloatDifferentialTest
./gradlew :iris-kotlin:check  # the Kotlin tests, detekt and the dependency check
./gradlew jmh            # the benchmarks, which take about 3 minutes
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

`:iris-kotlin:dokkaGenerate` also fails on any warning and runs through `check`.
The site build runs both tasks and publishes the result in its own pages (see
[site/README.md](../site/README.md)).

`ExamplesTest` runs every example in `iris-examples` from the repository root, in
a JVM of its own, and requires exit status 0, nothing on standard error, and the
output in the `.out` file beside the source. A new example needs that file and an
entry in `site/scripts/java-examples.mjs`: the site check fails on a file the
guide does not show. `iris-examples` has the same compiler, Error Prone, NullAway,
Spotless, ktfmt and detekt checks as the library, except for Kotlin's explicit
API mode, which a program does not need.

`./gradlew conformance` selects every case in `conformance/cases/0.1` and
`formats`, runs them with the installed driver's classes through the Rust runner, and
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

Run the same command with `../formats` for the format cases. This form needs a
Unix shell script; see [Windows](#windows).

## Continuous integration

[`java.yml`](../.github/workflows/java.yml) runs `./gradlew build` on Ubuntu,
macOS and Windows with Temurin 27 and stable Rust. It runs when `java/`,
`conformance/`, `formats/`, `rust/` or the workflow changes, and it uploads the test
reports of a failed build. Lint it from the root:

```sh
/tmp/iris-zdev/actionlint .github/workflows/java.yml
```

### Windows

As of 2026-11-05 no run of the workflow has been observed on any platform, so
nothing below has run on Windows. It comes from reading the sources of the runner,
the Rust standard library and the JDK launcher.

- **Line endings.** The cases compare bytes, so the job sets `core.autocrlf` to
  `false` before checkout, as `cpp.yml` and `python.yml` do. Spotless asks for LF
  on every platform; by default it would ask for CRLF there.
- **The driver command.** `ConformanceSuite` starts `java` with the installed
  `lib/*` on the class path instead of `bin/iris-driver.bat`. Every `params.json`
  ends in a newline, and Rust refuses to pass a newline to a batch file. So the by-hand
  command above cannot use the batch launcher either. The conformance task runs
  the same command on every platform, so what passes here is what runs there.
- **Verbatim paths.** The runner canonicalizes paths, which gives `\\?\D:\...` on
  Windows. `Path.of` reads that prefix on Java 27, for drives and UNC shares alike,
  so the driver opens these paths as they are. The runner's report spells them with
  forward slashes, and `ConformanceSuite` restores the backslashes.
- **Symbolic links.** Each format case's `spec.iris` is a link, so the job needs a
  Git that checks out links, as the Windows jobs of `cpp.yml` and `python.yml` do.
- **Non-ASCII `--params`.** `java.exe` reads its command line in the ANSI code page,
  and changing that takes a reboot. `chcp` sets only the console's code page and
  `-Dfile.encoding` applies after the arguments are decoded, so neither helps. On a runner with code page 1252,
  `sun.jnu.encoding` is `Cp1252`, and the driver refuses `é` with exit status 3.
  It cannot see `😀`, which the code page has already turned into `??`, and the
  value read is wrong. Either way such a case fails instead of passing. No
  `params.json` in `conformance/cases/0.1` or `formats` has a non-ASCII byte, so the
  suite does not reach this. A new case should spell such a value with JSON
  escapes, which work everywhere. `theLauncherKeepsParamsOrRefusesThem` stays off on
  Windows. The library is not affected: `Params` takes Java strings.

## Benchmarks

`./gradlew jmh` runs `ReadBenchmark`, in about 3 minutes, and writes
`iris-jmh/build/results/jmh.json`. The benchmarks measure and do not gate:
`./gradlew build` compiles `iris-jmh` and runs its tests, which read each input
once and check that every format case that reads has a benchmark, but it runs no
benchmark. `--args` replaces the JMH options:

```sh
./gradlew :iris-jmh:jmh --args="-f 1 -wi 1 -i 1 -p input=SUDOKU_FOUR_BY_FOUR"
```

`ReadBenchmark.read` measures one call of `Spec.read(byte[], String, Params)`,
which includes the handoff to the `LargeStack` pool. The spec is parsed and the input
is loaded in `@Setup`, so parsing and file access are not measured, and neither is
writing JSON or dzn. The inputs are the format cases that read without an error.
They are at most a few hundred bytes, so the numbers show the cost of a small read and
say nothing about throughput on large files.

Error Prone skips the classes that JMH generates, in `jmh_generated`, and only those;
`-Xlint:-processing` applies to this module, as the JMH processor leaves the JSpecify
annotations unclaimed. The default is one fork per input, and `-foe true` makes a
failed benchmark fail the task. A first run with three forks exited 0 with no
results, after JMH lost the link to its forks on this machine; the cause was not
established.

### Baseline

This is a local measurement, not a CI result, taken on 2026-11-05.

- **Machine:** Apple M1 Max, 10 cores, 64 GiB, macOS 27.0.1.
- **JVM:** Homebrew OpenJDK 27.0.1.0.3, 64-Bit Server VM, default options (G1), with
  JMH's compiler blackholes.
- **JMH 1.37:** average time, one thread, one fork, 5 warmup iterations of 2 s,
  10 measurement iterations of 1 s. `./gradlew jmh` ran five times, so each input
  had five JVMs.

The times are microseconds per read. Other jobs shared the machine, with load
averages from 9 to 160, a Gradle build ran during some of the runs, and some runs
took two or three times as long as others. Load, JVM variation and CPU state were
not controlled. The lowest column is the minimum observed for each input under
that load, not undisturbed performance, so a comparison with it is rough: rerun on
a quiet machine before reading anything into a small difference.

| Input | Bytes | Lowest | Runs 1 to 5 |
|---|---:|---:|---|
| `DIMACS_THREE_VARIABLES` | 66 | 9.0 | 9.5, 9.0, 19.9, 12.0, 17.4 |
| `JOBSHOP_JSPLIB` | 119 | 15.7 | 15.7, 15.9, 37.3, 29.9, 15.7 |
| `JOBSHOP_OR_LIBRARY` | 182 | 15.5 | 15.6, 15.5, 44.1, 18.6, 15.9 |
| `NONOGRAM_FIVE_BY_FIVE` | 86 | 16.4 | 17.4, 16.9, 16.4, 42.2, 18.0 |
| `SUDOKU_FOUR_BY_FOUR` | 37 | 8.3 | 8.5, 8.5, 8.5, 19.3, 8.3 |
| `TSPLIB_EUC_2D_FIVE` | 164 | 15.5 | 15.5, 15.7, 40.1, 40.5, 15.6 |
| `TSPLIB_EXPLICIT_FIVE` | 230 | 11.0 | 11.0, 20.8, 25.6, 25.7, 11.0 |
| `TSPTW_THREE_NODES` | 70 | 10.1 | 10.1, 22.4, 10.1, 26.4, 10.8 |

## Pinned tools

Gradle 9.8.2 (wrapper), Spotless 8.11.0 with google-java-format 2.0.0, Error
Prone 2.52.0, NullAway 0.14.3, JSpecify 1.0.1, JUnit 5.14.4, Kotlin 2.4.21, ktfmt
0.64, detekt 2.0.0-alpha.6, Dokka 2.2.0 and JMH 1.37.

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

Dokka 2.2.0, the newest stable release, runs on Java 27 with Kotlin 2.4.21 without
a warning. It cannot resolve the types of `dev.gecode:iris`, which it leaves
unlinked; `site/scripts/prepare-java-api.mjs` links them to the Javadoc pages. It
stays in its own Gradle configurations, so it is not on the compile or runtime
class path that `checkDependencies` checks, nor in the published POM.
