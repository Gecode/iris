# java

The pure-Java library `dev.gecode:iris` (Java 27, module `dev.gecode.iris`)
and the Kotlin extensions module `dev.gecode:iris-kotlin`. `PLAN.md` §8.3
describes both; milestone M4d builds them.

So far the library parses reader specs and applies every parse-time check of
the Specification. Reading comes with task `java-002`, and `iris-kotlin` with
`java-005`.

```java
import dev.gecode.iris.IrisException;
import dev.gecode.iris.Spec;

try {
  Spec spec = Spec.parse(source, "sat.iris"); // byte[] or String
} catch (IrisException e) {
  e.code();                 // ErrorCode.SPEC_SYNTAX, whose code() is "E-SPEC-SYNTAX"
  e.error().specLocation(); // origin, line and, for E-SPEC-UTF8, column
}
```

## Layout

| Path | Contents |
|---|---|
| `iris/` | The library. Only `dev.gecode.iris` is exported; `dev.gecode.iris.internal.syntax` holds the lexer, parser and resolver. |
| `iris-driver/` | The conformance driver, which is not published. `parse` is complete; `read`, `catalog` and `dzn` check the spec and then answer exit status 3, as a host failure, until `java-002`. |
| `iris-kotlin/` | The Kotlin module, still empty. |
| `build-logic/` | The convention plugin `iris.java-conventions` shared by the modules. |
| `gradle/libs.versions.toml` | The pinned versions. |

The legal nesting of §5.12 needs up to about 900 KiB of stack on a JVM, so
`Spec.parse` runs the parser and the resolver on a thread of its own with a
4 MiB stack (`LargeStack`). A legal spec parses whatever stack the caller has.

§5.12 bounds parsing, not the tree. Left operands can chain for tens of
thousands of nodes, as in `(((1+1)+1)+1)`, and aliases can chain to any length.
A walk of an expression therefore loops over `Expr.leftSpine` and recurses only
into the other operands, which hold a counter entry. A use of a named type keeps
its reading kind and refers to its definition by number, so it has no chain to
follow.

## Building and testing

The build needs Java 27 as `java` on `PATH` or in `JAVA_HOME`, and stable Rust
with `cargo` on `PATH`, since the conformance test runs the Rust runner. It
fails rather than skips without `cargo`.

```sh
./gradlew build          # compile, spotlessCheck, and all tests
./gradlew spotlessApply  # rewrite the sources
./gradlew :iris:test --tests dev.gecode.iris.ParseConformanceTest
```

`-Xlint:all -Werror` applies to main and test sources. Error Prone and NullAway
run inside `compileJava`, so they have no task of their own: any Error Prone
warning fails the build through `-Werror`, and NullAway reports at error level
for `@NullMarked` code, which is all of `dev.gecode.iris`.

`ParseConformanceTest` selects the `parse` cases in `conformance/cases/0.1` and
`formats`, runs them with the installed driver, and prints the selection and
the totals. It has no driver ID and no exemptions. By hand from the root, after
`./gradlew :iris-driver:installDist` in `java/`:

```sh
(cd rust && python3 -I -c 'import pathlib,tomllib; [print(p.parent) for r in ("../conformance/cases/0.1","../formats") for p in pathlib.Path(r).rglob("case.toml") if tomllib.loads(p.read_text()).get("operation")=="parse"]' | xargs cargo run --locked -p iris-conformance -- run --driver "\"$PWD/../java/iris-driver/build/install/iris-driver/bin/iris-driver\"")
```

## Pinned tools

Gradle 9.8.2 (wrapper), Spotless 8.11.0 with google-java-format 2.0.0, Error
Prone 2.52.0, NullAway 0.14.3, JSpecify 1.0.1 and JUnit 5.14.4.

### Java 27 gaps

As of 2026-11-04 every tool runs on Java 27, and no check is off. The Kotlin
compiler inside Gradle has no JVM 27 target, so compiling `build-logic` warns
and targets JVM 26; this affects only the build logic. Gradle may also print
`Did not receive connection preamble within 10s` during a `--no-daemon` run,
which is noise unless the build fails.
