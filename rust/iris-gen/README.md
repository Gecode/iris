# gecode-iris-gen

Generates spec builder programs and typed views from reader specs, as described
in [docs/builders.md](../../docs/builders.md). It belongs to the builder work of
[PLAN.md §8.15](../../PLAN.md#815-spec-builders). It depends only on `gecode-iris-syntax`
and `gecode-iris-sema`, and `iris gen builder` and `iris gen` are its commands.

`lower` turns a spec that `iris check` accepts into a `Program`, a flat list of
events with expressions in postfix form. Nothing in a `Program` belongs to one
language, and neither lowering nor rendering recurses, so generation handles
every nesting that the language allows. A `Backend` renders a `Program` as source.

```rust
use gecode_iris_gen::{Language, builder_code};

let parsed = gecode_iris_syntax::parse(b"x: int\n", "x.iris", gecode_iris_syntax::LanguageVersion::V0_1)?;
let analysis = gecode_iris_sema::analyze(&parsed);
let code = builder_code(&parsed, &analysis, Language::Rust)?;
```

A new language adds a `Language` variant, a backend module and one match arm in
`builder_code`. There are backends for Rust, Python, C++, Java, Kotlin and
TypeScript. `Language::has_view` says whether a language also has a view
printer: Rust, Python and C++ have one, and Java, Kotlin and TypeScript have
none yet.

## The Rust backend

The program is a complete file that uses `gecode_iris::build`. Its `builder`
function returns the `SpecBuilder`, and its `main` prints the spec. The output
passes `rustfmt` and `clippy -D warnings`, with these exceptions and allowances:

- A program nested more than 10 closures deep carries `#[rustfmt::skip]` on
  `builder`. rustfmt's run time grows too fast on such code, and from there its
  layout no longer follows from widths alone. No spec in the repository's corpus
  reaches that depth.
- A program nested more than 100 levels deep, counting the branches of its
  longest `if`, sets `#![recursion_limit = "4096"]`, because closures and the
  type of an `else_if` chain nest in the compiler.
- `builder` allows `clippy::approx_constant` when a float literal is close to a
  mathematical constant, and `clippy::eq_op` when the two operands of `-`, `/`,
  `||` or `&&` are the same call-free expression. Both are what the reader spec
  says. The backend compares operands by their shape in the postfix expression,
  not by their text, and a value that went into a `let` has no shape.

Each name gets a snake-case identifier that no keyword, generated name or other
name has, with a numeric suffix when it needs one; `b.forward("fn")` binds
`fn_2`. A repeat that something refers to but that has no `each` name gets an
invented one, which changes nothing that a spec can observe.

The layout holds for names and text of any length, because a value too wide for
its place goes into a `let`, and only the `let` has to match rustfmt exactly:

- A long name or string becomes `let e1 = name;` or `let e1 = "text";`.
- A comprehension variable is a closure parameter, which no `let` can shorten,
  so its identifier stops at 16 columns. Its spelling in the spec stays whole in
  `.named("...")`.
- String literals are ASCII, with everything else as `\u{...}`. rustfmt measures
  display columns, and only in ASCII is a length a width. It takes a table of
  Unicode widths to do better, which this crate does not have.

## The Python backend

The program is a complete module that imports `gecode_iris.build as ib`. Its
`builder` function returns the `SpecBuilder`, and running the module prints the
spec. It targets Python 3.10 and the `gecode_iris.build` stubs, so blocks are
`with` statements and line patterns are tuples, never t-strings. The output
passes, unchanged, `ruff format --check`, `ruff check` with ruff's defaults and
the B, I and UP rules (88 columns, `--target-version py310`), and `ty check`.
It writes ruff's layout itself, so nothing has to run ruff on it.

- Nested blocks with one statement each join into one `with` of several items,
  as ruff's SIM117 asks.
- Python allows 20 nested blocks in one function, and each item of a `with`
  counts. A block deeper than 10 indentation levels or 16 `with` items becomes a
  helper function, such as `def block_1(s17: ib.ScopeN[None]) -> None:`, at the top of
  `builder`. Helpers see the variables of `builder` but not each other's, so a
  helper takes each type of another helper that it uses as a parameter, such as
  `a: ib.IntType[int]`, after its scope.
- A comprehension body is a lambda when it needs no statement and fits on its
  line, and otherwise a local function that takes the comprehension variable
  and any outer variables it uses.
- An operand that the stubs refuse goes through `.dynamic()`, and a Python
  number or string on the left of an operator through `ib.lit`. `-(-x)` and a
  comparison of a name with itself also get `.dynamic()`, which keeps B002 and
  PLR0124 quiet without changing the spec.

Identifiers follow the Rust backend's rules with Python's keywords, its soft
keywords (`match`, `case`, `type` and `_`), the names in `builtins` of Python
3.10 and 3.15, and the program's names (`b`, `ib`, `builder`, numbered names
such as `e1` and `s2`, and E741's `l`, `O` and `I`). Python has one namespace for values and types, so
`type b` binds `b_`. A name stops at 24 columns and a comprehension variable at
12; the spelling in the spec stays whole in the string that the builder prints.
String literals are ASCII, with everything else as `\xNN`, `\uNNNN` or
`\UNNNNNNNN`, for the same reason as in Rust.

The Python round trip needs an interpreter with `gecode_iris`, ruff and ty, such
as `python/.venv` after the `python/` setup in AGENTS.md. It generates every
builder, checks them with ruff and ty, runs them and compares the specs they
print:

```sh
IRIS_PYTHON="$PWD/../python/.venv/bin/python" cargo test --locked -p gecode-iris --test python_round_trip -- --ignored --nocapture
```

## The C++ backend

The program is a complete file that includes `<iris/build.hpp>`. Its
`builder` function, in an anonymous namespace with `using namespace
iris::build`, returns the `SpecBuilder`, and its `main` prints the spec, or the
error and exit status 1. It targets C++23 and compiles without warnings under
the flags of `cpp/`. The output passes, unchanged, clang-format 22.1.8 with
`cpp/.clang-format` and clang-tidy 22.1.8 with `cpp/.clang-tidy`. It writes
clang-format's layout itself: each statement's candidate layouts are measured,
and the one with the fewest lines wins, as in clang-format's penalties for this
code.

- Every handle is declared first, as `auto x = b.forward<DynField>("x");` or
  `b.forward_record("job", "jobs")`, so that a statement may name a handle
  defined later in the spec.
- Blocks are lambdas taking `auto &` scopes. A block whose statements would be
  indented deeper than 10 levels becomes a helper lambda, such as
  `auto block_1 = [&](Scope<10, Presence::Required> &s10) { ... };`, before the
  statements of `builder`. A helper takes each type of another helper that it
  uses as a `const auto &` parameter after its scope.
- An `if` with an `else` binds its chain, as in `auto c1 = s1.if_(...)`, and
  continues with `c1.else_if`, `c1.else_` or `c1.else_read_if`.
- A comprehension body is a lambda that returns it, and a body too deep for its
  line becomes a helper lambda `body_1`.
- A literal on the left of an operator goes through `lit`, and `pow` is
  written `iris::build::pow`.
- A value too wide for its place goes into `auto e1 = ...;`, and a long string
  into a `constexpr std::string_view` constant `t1` at the top of the anonymous
  namespace, split into pieces that each fit a line.

Identifiers are lower snake case, as clang-tidy's naming check asks. A name that
is a C++ keyword, a name of the program or of `iris::build`, a common macro of the
C and POSIX headers, a numbered name such as `e1` or `block_1`, or the identifier
of another name gets a numeric suffix: `if` binds `if_2`. C++ has one namespace for
local variables, so `type b` binds `b_2`. A name stops at 24 columns and a
comprehension variable at 12; the spelling in the spec stays whole in the string
that the builder prints. String literals are ASCII, with every other byte as a
three-digit octal escape, and a string with a NUL is a `std::string_view`
literal.

The C++ round trip needs clang-format 22.1.8, CMake 3.28 or newer, and a C++23
compiler. It generates every builder, checks it with clang-format and, when
`IRIS_CLANG_TIDY` names clang-tidy 22.1.8, with clang-tidy, builds them against
`cpp/` with the warnings as errors, runs them and compares the specs they print.
CMake finds the compiler, so `CXX` and `CMAKE_GENERATOR` choose it, and
`IRIS_CMAKE` names CMake. `IRIS_CXX_FLAGS` and `IRIS_EXE_LINKER_FLAGS`, when
set, replace the build's compile and link flags, those of its Debug
configuration included. From `rust/`, with uv:

```sh
IRIS_CLANG_FORMAT="$(uvx --from clang-format==22.1.8 which clang-format)" \
IRIS_CLANG_TIDY="$(uvx --from clang-tidy==22.1.8 which clang-tidy)" \
  cargo test --locked -p gecode-iris --test builder_round_trip_cpp -- --ignored --nocapture
```

On macOS, export `SDKROOT="$(xcrun --show-sdk-path)"` for clang-tidy. The
programs build in shards of 24 under `target/tmp/cpp-round-trip`, in one build
directory per compiler and set of flags, so a second run compiles only what
changed. The `builder-round-trip` test of `cpp/` runs it with that build's
compiler and its compile and link flags, those of its configuration
included, so that the `asan` preset builds the programs under ASan and
UBSan.

## The Java backend

The program is a complete file without a package declaration, which the caller
adds. It holds `final class SpecBuilder`, whose `static Builder specBuilder()`
returns the builder and whose `main` prints the spec. It uses the static
methods of `dev.gecode.iris.build.Iris`, compiles under the flags of `java/`
(`-Xlint:all -Werror`, Error Prone and NullAway) and passes google-java-format
unchanged, as Spotless runs it in `java/`. It writes google-java-format's
layout itself: a call goes on one line if it fits, and otherwise breaks after
its opening parenthesis and puts its arguments on one line, fills lines with
them when each is at most 9 columns, or puts each on a line of its own. An
assignment breaks after `=` first, and a block lambda that is the only argument
stays on the call's line.

- Every handle is a field, such as
  `private static final DynField x = dynamic("x");` or
  `private static final RecordName job = recordName("job", "jobs");`, so that a
  statement may name a handle defined later in the spec. A name that the spec
  never defines is a local `var x = undefined("x");`, and `_` as a name is
  `forward("_", Dyn.class)`.
- Blocks are `Runnable` lambdas. A block whose statements would start beyond
  column 36 becomes a method, such as `private static void block1(IntType t)`,
  that takes each type and undefined name from an enclosing method that it
  uses.
- A type that a statement uses is a local `var t = defineType("t", ...);`.
- An `if` chain is a sequence of statements: `ifThen`, then `elseIf`,
  `otherwise` or `elseReadIf`.
- A comprehension is `sum(over(xs).named("v"), v -> body)`, with `_ ->` when the
  body does not use its variable. A body that does not fit on its line becomes
  a method `private static Expr body1(DynExpr v)` that takes the
  comprehension variable and any outer variables it uses.
- A Java number or string on the left of an operator goes through `lit`, and an
  operand that the types of `dev.gecode.iris.build` refuse goes through
  `.dynamic()`, such as both sides of `1 == "b"`. `gecode.int_max` is
  `Gecode.intMax`. More than three `sections` clauses go in a `List.of`.
- A value too wide for its place goes into `var e1 = ...;`, and a long string
  into a constant `private static final String T1`, split into pieces that each
  fit a line.

Identifiers are lower camel case, as the Google Java style asks: `n_jobs` binds
`nJobs`. A name that is a Java keyword, literal, contextual keyword or `_`, the
program's `args`, or the identifier of another name gets a numeric suffix: `if`
binds `if2`. A name like a temporary gets a `v` first, so `e1` binds `e1v`.
Fields and locals share the suffixes, so `type record` binds `record2`. A name
stops at 24 columns and a comprehension variable at 12; the spelling in the
spec stays whole in the string that the builder prints. String literals are
ASCII, with everything else as `\uNNNN` escapes of UTF-16 units.

The Java round trip needs Java 27, on `PATH` or in `JAVA_HOME`. It writes every
builder in a package of its own under `target/tmp/java-round-trip` and passes
them to the `iris-round-trip` module of `java/`, whose Gradle build checks them
with Spotless, compiles them in one javac run with the runner, and runs the
runner, which writes what each program prints. From `rust/`:

```sh
cargo test --locked -p gecode-iris --test builder_round_trip_java -- --ignored --nocapture
```

Gradle's output goes to `target/tmp/java-round-trip/gradle.log`, which a
failure prints.

## The Kotlin backend

The program is a complete file without a package declaration, which the caller
adds. Its `fun specBuilder(): IrisBuilder<Unit>` returns the builder of
`dev.gecode.iris.kotlin.build`, and its `main` prints the spec. It compiles under
the flags of `java/`, warnings as errors, and passes ktfmt in kotlinlang style
unchanged, as Spotless runs it in `java/`. It writes ktfmt's layout itself: a
call goes on one line if it fits, and otherwise breaks after its opening
parenthesis and puts each argument on a line of its own, with a trailing comma
when there are several; ktfmt never fills a line with arguments. An assignment
breaks after `=` first, and a block is a trailing lambda, `repeat(n) { ... }`.

- Every handle is a top-level property, such as
  `private val x = dynamic("x")` or
  `private val job = recordName("job", "jobs")`, so that a statement may name a
  handle defined later in the spec. A name that the spec never defines is a
  local `val x = undefined("x")`, and `_` as a name is `forwardDynamic("_")`,
  since both are statements of the open scope.
- A block whose statements would start beyond column 36 becomes an extension
  function on the class of its scope, such as
  `private fun DeepScope.block1(t: IntType)`, that takes each type and
  undefined name from an enclosing function that it uses. The arms of
  `sections` stay where they are.
- A type that a statement uses is a local `val t = defineType("t", ...)`.
- An `if` chain is a sequence of statements: `ifThen`, then `elseIf`,
  `otherwise` or `elseReadIf`. An arm's options are named arguments,
  `arm("b", optional = true, last = true)`, and an optional arm with a block is
  `optionalArm`.
- Operators are Kotlin's: `+ - * / %` and unary `-` as operators, comparisons
  and `&& || == !=` as the infix functions `lt`, `and`, `eq` and the others, `!`
  as an operator, `^` as `pow`, and `lo..hi` and `range(lo).below(hi)` as ranges,
since ktfmt 0.64 removes the import of `rangeUntil` that `lo..<hi` needs. Every
  infix call has the same precedence in Kotlin and associates to the left, so
  an infix operand that is itself an infix call goes in parentheses. A record
  field is `x[field]` and an index `x.at(i)`.
- A comprehension is `sum(over(xs).named("v")) { v -> body }`, with `_ ->` when
  the body does not use its variable, and no parameter for a variable named
  `it`, which is the lambda's own. A body that does not fit on its line
  becomes a function `private fun body1(v: DynExpr): Expr` that takes the
  comprehension variable and any outer variables it uses.
- A Kotlin number or string on the left of an operator goes through `lit`, and
  an operand that the types refuse goes through `.dynamic()`, as in Java.
  `gecode.int_max` is `Gecode.intMax`.
- A value too wide for its place goes into `val e1 = ...`, and a long string
  into a constant `private const val T1`, which keeps it whole, as ktfmt does.

The program imports each declaration of `dev.gecode.iris.kotlin.build` that it
names, the operator functions of `%`, unary `-` and `..` among them,
since detekt refuses a wildcard import and ktfmt removes an unused one.

Identifiers are lower camel case. A name that is a Kotlin hard keyword or `_`,
a property of every scope (`depth`, `record`, `session`), or the identifier of
another name gets a numeric suffix: `val` binds `val2`. Temporaries, lengths and
other rules are Java's. String literals are ASCII, with `$` escaped and
everything else as `\uNNNN` escapes of UTF-16 units.

The Kotlin round trip needs Java 27, like the Java one. It writes every builder
in a package of its own under `target/tmp/kotlin-round-trip` and passes them to
the `iris-kotlin-round-trip` module of `java/`, whose Gradle build checks them
with Spotless and detekt, compiles them in one kotlinc run with the runner, and
runs the runner. detekt uses the build's rules, except four that measure the
spec rather than the generated code, such as `MagicNumber`; the module's
`detekt.yml` lists them and says why. From `rust/`:

```sh
cargo test --locked -p gecode-iris --test builder_round_trip_kotlin -- --ignored --nocapture
```

Gradle's output goes to `target/tmp/kotlin-round-trip/gradle.log`.

## The TypeScript backend

The program is a complete ES module that imports `gecode-iris/build`. Its
`export function builder(): SpecBuilder` returns the builder, and run as the
main module it prints the spec. It passes oxfmt, oxlint with `--type-aware
--deny-warnings`, and tsc in strict mode unchanged, each with the settings of
`typescript/`. It writes oxfmt's layout itself, at width 100: a call goes on
one line if it fits, and otherwise puts each argument on a line of its own
with a trailing comma, unless only its last argument, an object, an array or
an arrow function with a block, breaks. A chain of two calls stays on one
line, and a longer one puts each call after the first on a line of its own.

- Every handle is a local at the top of `builder`, such as
  `const x = b.forward("x")`, so that a statement may name a handle defined
  later in the spec. A name that the spec never defines is
  `const q = b.undefined("q")`, and a record that a check outside its block
  uses is `const r = b.forwardRecord("r", "rs")`; the block of any other
  record names it with `eachName` and `asName`.
- A block is an arrow function whose parameter is numbered by depth, `s1`
  for a scope and `a1` for the arms of `sections`. An empty block is left
  out where the builder allows it, and is `() => {}` where it does not.
- A block deeper than 9, or whose statements would start beyond column 36,
  becomes a module function, such as
  `function block1(s1: NestedScope<HandleDepth, Presence>, x: DynField, t: Type<"int", 0>): void`,
  that takes each handle, record and type that it uses. Every function is at
  module level, since oxlint's `unicorn/consistent-function-scoping` refuses
  a nested one that captures nothing.
- A type that a statement uses is `const t = b.defineType("t", ...)`.
- An `if` chain is one statement, `b.if(c, ...).elseIf(...).else(...)` or
  `.elseReadIf(if2, int)`. The clauses of `sections` are an options object,
  `{ until: "X", store: a, skipOther: true }`, or, in any other order, an
  array of them. An arm's options are an object, `{ optional: true }`.
- A pattern is a `p` template, with `bind(x)`, `bind(x, int)`, `ref(x)`,
  `validate(int)`, `blank()` and `discardAs(int)` as its parts.
- Operators are methods: `plus`, `minus`, `times`, `neg`, comparisons as `lt`,
  `eq` and the others, and `and`, `or` and `not`. `/`, `%` and `^` are the
  functions `div`, `rem` and `pow`, and an index is `x.at(i)` and a record
  field `r.get(k)`. A number or string on the left of an operator goes
  through `lit`, or `floatLit` for a float, an integer that is not a safe
  integer is a bigint such as `9007199254740993n`, and an operand that the
  types refuse goes through `.dynamic()`. `gecode.int_max` is
  `gecode.intMax`.
- A comprehension is `const e1 = sum(over(xs, "v"), (v) => body)`, with `()`
  when the body does not use its variable. A body that does not fit on its
  line becomes a function `function body1(v: Lookup<"dyn">): Expr` that takes
  the comprehension variable and any outer variables and handles it uses.
- A value too wide for its place goes into `const e1 = ...`, with `as const`
  for an object or an array so that tsc keeps its literal types, and a long
  string into a module constant `const t1 = ...`, which keeps it whole, as
  oxfmt does.

The program has one import, which names each value of `gecode-iris/build`
that it uses and, as `type`, each type in its functions' signatures, since
oxlint refuses an unused import.

Identifiers keep the spec's spelling, cut to 24 characters, or 12 for a
comprehension variable, and without an underscore at either end, which
oxlint's `no-underscore-dangle` refuses. A name that JavaScript reserves, that
`gecode-iris/build` exports or the program uses, such as `b` or `builder`,
or that another name has gets a numeric suffix: `if` binds `if2`. Iris keeps
values, types and comprehension variables apart, but in TypeScript they share
one scope and oxlint's `no-shadow` refuses an inner one that hides an outer
one, so each gets an identifier of its own. A name of the form of the
program's numbered names, such as `e1`, gets a `v`: `e1v`. String literals are
ASCII, in double quotes unless the text has more of them than single quotes,
with everything else as `\uNNNN` escapes of UTF-16 units.

The TypeScript round trip needs Node 22.18 or later and `npm ci` and
`npm run build` in `typescript/`. It writes every builder to
`target/tmp/typescript-round-trip/src`, beside a `tsconfig.json` that extends
the one of `typescript/` and a link to it as `gecode-iris`. It checks them in
batches with oxfmt and oxlint and in one tsc run, and runs them all in one
Node process with `typescript/test/round-trip-runner.ts`, which the package
does not ship. From `rust/`:

```sh
cargo test --locked -p gecode-iris --test builder_round_trip_typescript -- --ignored --nocapture
```

## Views

`view` turns a spec that `iris check` accepts into a `View`, a list of handles
that belongs to no language, and `view_code` prints it. Each `Handle` holds the
entry's name and its `Access`: `Typed` with a value type, a depth and whether the
entry may be absent, or `Dynamic`. Every fact comes from
`SemanticModel::output_entries`; this crate infers nothing. A typed handle is a
promise that `get` cannot fail on data that a read produces, so an entry is
`Dynamic` unless its facts leave one value type and one depth. `Num` needs each
definition to leave both int and float open, so `x:int x:float` is `Dynamic`.

The Rust printer writes one `pub const` per entry, with handle types named as
text, so this crate does not link `gecode-iris-build`. The header is a plain
comment and each constant has its own `#[allow]`, because `include!` rejects inner
attributes and inner doc comments. It maps a depth above 8 to
`DynField`, because `gecode_iris::build` stops at `D8`; the `View` keeps the real
depth for languages without that limit. Identifiers follow the builder's rules,
but only keywords and `spec` are reserved, since a view has no generated names
beside its own: `spec` is the field of the `iris!` macro that holds the `Spec`,
so an entry `spec` is `spec_2` in both. An identifier stops at 40 columns, which
gives a declaration three layouts. A new language adds a printer, not a change
to `View`.

The Python printer writes a module that imports `gecode_iris.build as ib` and
declares one handle per entry with `ib.declared(name, handle type)`, such as
`machine = ib.declared("machine", ib.IntField[list[list[int]]])`, with
`| None` inside the brackets for an entry that may be absent. It also maps a
depth above 8 to `ib.DynField`, because the widest handle type one level deeper
no longer fits on a line of its own. Identifiers follow the builder program's
rules, including its reserved names and its 24-column cut, without a leading
underscore, which would make a handle private to the module. A declaration has
the three layouts `ruff format` gives a call of two arguments. The output
passes `ruff format --check`, `ruff check` with B, I and UP, and ty, and its
handles type-check under pyright and mypy.

The C++ printer writes a header that includes `<iris/handle.hpp>` and declares
one `inline constexpr` handle per entry, such as
`inline constexpr iris::Field<std::int64_t, 2> machine{"machine"};`, in a
namespace named after the file name of the spec's origin, so `jobshop.iris`
gives `jobshop::machine`. Templates take any depth, so every depth has a typed
handle. Identifiers are lower snake case with the builder program's 24-column
cut, and avoid only keywords, macros and the namespaces `iris` and `std`, which
a declaration names. A declaration takes the layout clang-format gives it in the
style of `cpp/.clang-format`; a literal too wide for every layout would make
clang-format split it, so that declaration stays on one line between
`// clang-format off` and `// clang-format on`.

`rust_view` gives the Rust declarations as data, a `RustView` of the imports
from `gecode_iris::build` and one `RustHandle` per entry, with its identifier,
type text and entry name. The printer writes it, and the `iris!` macro of
gecode-iris declares the same handles as struct fields, so the two cannot
differ.

From `rust/`:

```sh
cargo test --locked -p gecode-iris-gen
cargo test --locked -p gecode-iris --test view_formats --test view_soundness -- --nocapture
cargo test --locked -p gecode-iris --test builder_round_trip -- --nocapture
```

`tests/python_view.rs` checks the Python text of each handle type, the
identifiers and the layouts. With `IRIS_PYTHON` naming an interpreter with
`gecode_iris` and its lint group installed, as for the Python round trip,

```sh
IRIS_PYTHON="$PWD/../python/.venv/bin/python" cargo test --locked -p gecode-iris --test python_views -- --ignored --nocapture
```

writes Python views of every format and of specs that exercise redefinitions,
reserved names, depths and line widths, checks them with ruff and ty, checks
`data[handle]` of every handle with pyright and mypy in strict mode, and reads
each format case and a case of each spec through its view, comparing every
entry with untyped access. It prints `Python views: M modules, R reads, E
entries compared`.

`tests/cpp_view.rs` checks the C++ text of each handle type, the namespace,
the identifiers and the layouts. With CMake 3.28+ and a C++23 compiler,

```sh
cargo test --locked -p gecode-iris --test cpp_views -- --ignored --nocapture
```

writes C++ views of every format and of specs that exercise redefinitions,
reserved names, a depth of 9 and line widths, and builds one program that
includes them all with the warnings of `cpp/` as errors. The program pins the
type of `data.get(handle)` for every handle with `static_assert`, reads each
format case and a case of each spec, compares every entry with untyped access
and checks that every entry has a handle. With `IRIS_CLANG_FORMAT` naming
clang-format 22.1.8 the views must be what it writes, and with
`IRIS_CLANG_TIDY` clang-tidy 22.1.8 must accept them. It prints `C++ views: V
views, R reads, E entries compared`, and runs as the CTest test
`generated-views` of `cpp/` with that build's compiler and flags.

Both round trips print `compared C, skipped S, extra E, differences D`. They
fail on any difference, when compared and skipped specs do not add up to the
corpus, or when fewer than 770 specs are compared. `tests/round_trip/mod.rs` in
gecode-iris holds what they share: the corpus, the extra specs of the layout
and lint rules, and the limit specs.
