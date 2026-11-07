# Spec builders

This guide is the design brief for **spec builders**: APIs that construct a
reader spec in host-language code instead of in `.iris` text. Every
implementation gets one: Rust, Python, C++, Java, Kotlin, and TypeScript.
The TypeScript package itself is still being planned, so its section covers
only the builder.

People ask for builders to get type safety. A misspelled entry name or a
wrong value type should fail when the program is compiled, or when its
type checker runs. It should not wait until data is read. Builders also
bring the host language's abstraction to reader specs: a function can
produce a fragment, and a loop can produce fields.

The Rust builder exists, as the `gecode-iris-build` crate re-exported by
`gecode_iris::build`, and its section below is compiled as a test. The
Python builder exists too, as the `gecode_iris.build` module, and its section
shows its API. The other languages' APIs do not exist yet: their code is
sketches, and the names in it are illustrative. The principles are the part to follow. Each implementation
chooses final names in its own idiom, and its API guide on the site documents
them. [PLAN.md §8.15](../PLAN.md#815-spec-builders)
gives the deliverables and the milestone.

## Principles

### 1. A builder writes a reader spec

A builder emits Iris text for one language version. It then parses and
checks that text with the implementation's ordinary `Spec::parse`. The
result is the same `Spec` type a text spec produces, plus typed handles.
Every builder also returns its text (`source()` or the language's
equivalent). The text can be saved as a `.iris` file, run with `iris read`,
opened in the playground, or shown to the agent skill.

Text is the contract because it already is one. The Specification defines
it, the conformance suite tests it, and every implementation parses it. A
shared syntax-tree format for builders would be a second contract between
implementations. It would need its own tests and would drift. A reader spec
is small, so printing it and parsing it again costs nothing that matters.

It follows that:

- Builders add no semantics. Each builder construct prints to exactly one
  syntactic form of the language version. There are no builder-only
  features and no shortcuts past the checker.
- A builder never reimplements name resolution, typing, or the static
  checks. `build()` runs the implementation's checker, and its errors are
  the language's errors with the language's codes.
- A builder covers the whole language version. A construct the builder
  cannot express makes the builder incomplete, so builders get no raw-text
  escape hatch.
- The same calls give byte-identical text. That supports snapshot tests,
  caching, and diffs. Builders print one statement per line with two-space
  indentation, as `iris fmt` does. They add only the parentheses that
  precedence requires.

### 2. Typed handles name entries

Every statement that defines an entry returns a **typed handle**. A handle
carries the entry's name, value type, depth, and whether it can be absent.
It serves two purposes:

- In the builder it is an expression. Inside a repeat it refers to the
  current record's value, and outside it refers to the whole array, as in
  Iris itself.
- After reading it is a key into the data. `data.get(handle)`, or the
  language's indexing form, returns the host type for that value type and
  depth. It cannot fail on a name or a type, unless the handle is stale
  (see below) or Rust's `.ex()` asserted a scalar that was not one.

The same handle types also serve typed views generated from text specs
(see [Type safety for text specs](#type-safety-for-text-specs)). The typed
data layer is shared, and builders are one way to obtain handles.

**Depth comes from the scope.** A scope knows how many repeats enclose it.
A field defined in a scope has a data depth equal to that number plus the
declaration's dimensions. The handle's type is then its data type as soon
as it is created. Nothing has to be lifted when it leaves a repeat. Each
language encodes this as cheaply as it can:

| Language | Encoding |
|---|---|
| Rust | Phantom depth types `D0`, `D1`, ... with a successor trait. Impls go up to a fixed depth, and deeper entries get a dynamic handle. |
| C++ | A `std::size_t` template parameter: `Field<std::int64_t, 2>`. |
| TypeScript | A depth parameter, `Field<"int", 2>`, mapped to the data type by conditional types. Keep the type-level arithmetic shallow for checker speed. |
| Java, Kotlin | The data view type as the parameter, `IntField<List<long[]>>`, with one generic `get`. Java declares handles with factories that fix the depth (`int2`), and `build()` checks them. Kotlin scopes come in depth-specific receiver classes up to depth 3, and deeper entries are dynamic. |
| Python | The data view type as the parameter, `IntField[list[list[int]]]`, in the type stubs. Scopes come in depth-specific classes, `Scope0` to `Scope3`, and deeper entries are `DynField`. |

**Absence is in the type.** A definition that may not execute gives an
optional handle: Rust `Option`, C++ `std::optional`, Java `@Nullable`,
Kotlin `T?`, Python `T | None`, TypeScript `T | undefined`. Such
definitions are those inside `if` and `else`, and those in `optional`
sections arms. A name with a conditional definition and an unconditional
one later is not optional. The builder does not attempt more flow analysis
than that. Where it cannot prove presence, the handle stays optional, and
the user unwraps it.

**A handle's type is fixed at creation and checked at `build()`.** Most
handles are created by the statement that defines them, so the builder
knows their position. A capture is the exception: it is created before the
pattern that binds it, so that the pattern can refer to it. Its scope and
optionality are declared at creation (`capture` or `optionalCapture`).
`build()` reports a capture that is bound in a different scope, bound
twice, or never bound. The error says which form to use instead.

**Names whose type is open stay dynamic.** These are `num` entries (int or
float, decided by the data), parameters without a default, and a name
redefined with another value type. Their handles return the language's
existing dynamic value (`ValueRef`, `Value`, and so on), or a numeric union
for `num`. Typed parameter handles also set parameters:
`params.set(scale, 3)` formats the value as the conversion in
[§9.2](../spec/0.1/09-params-and-instances.md#92-parameter-evaluation-and-conversion)
expects.

**Dynamic forms cover what static types refuse.** A valid spec can use a
name before its definition, compare values of different types, or redefine a
name with another type. A builder must be able to write all of them, so that
every spec `iris check` accepts can be built. Four forms step outside the
static types, in every language:

- **A forward handle** names an entry before the statement that defines it.
  Its type is declared when it is created. A later defining statement takes
  the handle as its target, and so does a pattern, a `store`, or a capture.
  A definition that agrees with the declared type binds the handle.
  `build()` reports a forward handle that is never bound, and a definition
  of another value type or depth when no binding always comes before it. A
  later definition after such a binding is an explicit redefinition: it
  returns a dynamic handle, and the forward handle may go stale, as below.
  The builder certifies a binding only from what it knows. A value with no
  static shape, such as a `.dynamic()` expression, an element access, or a
  bare handle inside a repeat, cannot bind a statically typed forward handle;
  declare a dynamic forward handle for it. A required forward handle needs at
  least one binding that always executes.
- **`.dynamic()`** turns an expression into one with no static type, which
  every operator and statement accepts.
- **Redefinition through a handle.** Any defining statement, not only `let`,
  takes an existing handle as its target and returns a dynamic handle, since
  the value type or depth may have changed. `handle.set(expr)` is the form
  that keeps the type: a value of another type or depth is an error, at
  compile time where the host's types encode depth.
- **`undefined(name)`** gives a dynamic handle for a name deliberately left
  undefined, as in `check !defined(zz)`. Defining that name is an error.
  Forward handles stay strict: a forward handle that nothing binds is a
  mistake, and `undefined` is how a program says it is not.

**A redefinition can leave a stale handle.** In a language with static
handles, the old handle keeps the type it was created with. If the
redefinition kept the value type and depth, the old handle remains right,
and the user can keep using it. If either changed, the old handle is stale:
use the handle the redefinition returned. Reading through a stale handle
fails as a mismatch between handle and data, not as a wrong value.

### 3. Catch statically what is cheap, and the rest at `build()`

The host type system checks what it can express without contortions:
expression value types (an `int` compared with a `string`), handle kinds (a
record where a field is expected), and the data types of accesses.
Everything else is the checker's job at `build()`. Builders add only these
checks of their own, each reported at `build()` or earlier:

- **Scope discipline.** A scope accepts statements only while it is the
  innermost open block. Kotlin enforces this statically with `@DslMarker`,
  and Java by construction, since its statements always go to the bound
  block. The other languages check it at runtime, because a statement added
  to an outer scope would land in the wrong place in the text.
- **Provenance.** A handle from another builder is an error.
- **Explicit redefinition.** Iris lets a later definition replace a name.
  In a builder that is usually a helper called twice by mistake, so
  defining a name that is already defined is an error. Redefinition goes
  through the existing handle (`handle.set(expr)`, printed as `let`).
- **Printable text.** Some values have no spelling: a pattern literal
  containing `{` or `...`, an entry named `format` at the top level, a
  definition starting with `gecode.`, a name that is not an identifier, a
  non-finite float, a range with no upper bound directly
  before a `format` statement (`x:0..` would take `format` as its bound),
  and the integer -9223372036854775808. The builder rejects them early, or
  prints the standard workaround: `(-9223372036854775807 - 1)` for the
  minimum integer.
- **The name `_`.** Its meaning depends on the construct, as the
  Specification defines. A `let`, parameter, named type, or comprehension
  binding cannot have it. A declaration, list, or capture named `_` keeps
  nothing, so it is written `discard()` and gives no handle. `store _` does
  keep an entry at the top level, and in an expression `_` is an ordinary
  lookup, or the records of `as _`. So `forward("_")`, `undefined("_")`,
  and the `as` and `each` names accept it, and only a `store` binds a
  forward `_`. Inside a repeat that entry ends with its iteration: no
  column collects it, so no array handle can read it, and a handle of
  depth 0 reads it as absent afterwards.

### 4. Users choose entry names; the builder chooses internal names

Entry names appear in the data, in JSON and dzn output, and in other
programs, so the user supplies them. Builders never transform them: no
case conversion, and no prefixes the user did not ask for. Kotlin can take
an entry name from a delegated property. That is the property's name
verbatim, with an override for other spellings.

Names that never reach the data are the builder's business: `each` record
names, `as` record-list names, and comprehension variables. The builder
emits `each` or `as` only when a handle is used where Iris needs one. It
generates deterministic fresh names, unless the user supplies names for
readable output. Users therefore work with one record handle per repeat,
used both for projection (`job[machine]`, printed `job.machine`) and for
comprehensions (`sum(job, j -> ...)`, printed `j in jobs`).

### 5. Blocks use the host's block idiom

`repeat`, `if`, `sections` arms, and `instances` open blocks. Each language
uses the form its users expect for nested structure:

| Language | Blocks | Getting handles out |
|---|---|---|
| Rust | Closures taking `&mut Scope<D>` | Return them; handles are `Copy` |
| C++ | Lambdas taking `auto&` scopes | Return a `std::tuple`, bind with structured bindings |
| Python | `with` statements | Python has no block scope; names stay visible |
| Kotlin | Lambdas with receivers, marked with `@DslMarker` | Return them and destructure |
| Java | Lambdas without parameters; the open block is a scoped value | Declared handles (see the Java section) |
| TypeScript | Callbacks | Return an object and destructure |

Settings such as an arm's `optional` and `last`, or a repeat's terminator,
take the language's form for options: builder methods in Rust, designated
initializers in C++, keyword arguments in Python, named arguments in
Kotlin, option values as varargs in Java, and an options object in
TypeScript.

### 6. Operators only where they mean the same thing

A host operator may build an Iris operator only when the two agree on
every value both accept. Elsewhere a named function makes the difference
visible. Integer `/` and `%` truncate toward zero in Iris
([§8.4](../spec/0.1/08-expressions-and-checks.md#84-integer-arithmetic)). Rust, C++, Kotlin, and
Java do the same, but Python's `//` and `%` floor. Rust's `PartialOrd` and
Kotlin's `compareTo` must return a primitive, so comparisons are methods
there.

| Iris | Rust | C++ | Python | Kotlin | Java | TypeScript |
|---|---|---|---|---|---|---|
| `+ - *`, unary `-` | operators | operators | operators | operators | methods | methods |
| `/ %` | operators | operators | `div()`, `rem()` | operators | methods | `div()`, `rem()` |
| `^` | `pow()` | `pow()` | `pow()` | `pow()` | `pow()` | `pow()` |
| `< <= > >=` | `lt()` ... | operators | operators | infix `lt` ... | methods | methods |
| `== !=` | `eq()`, `ne()` | operators | operators | infix `eq`, `ne` | methods | methods |
| `&& \|\| !` | `& \| !` | operators | `& \| ~` | infix `and`, `or`, `!` | methods | methods |
| `x[i]` | `at()` | `[]` | `[]` | `[]` | `at()` | `at()` |
| record field | `get()` | `get()` | `[]` | `[]` | `get()` | `get()` |

Language notes:

- **Python** follows SQLAlchemy and pandas. `__bool__` raises a `TypeError`
  that explains the fix, which catches `if n > 3:` and the chained
  `0 <= x < n`. `&` and `|` bind tighter than comparisons, so their
  operands need parentheses. The error message for a bare `and` says so.
  Handles hash by identity, so `==` can build expressions.
- **C++** overloads of `&&` and `||` lose short-circuiting. That does not
  matter while building a tree, but the API guide says so.
- **Kotlin** members win over top-level functions, so receiver methods may
  be named `check` and `repeat`, after the Iris statements, even though the
  standard library has functions of those names.

Ranges follow the same rule. Kotlin's `lo..hi` and `lo..<hi` mean exactly
Iris's `lo..hi` and `lo..<hi`, so Kotlin overloads `rangeTo` and
`rangeUntil` for expressions. Rust's `lo..hi` excludes `hi`, so it prints as
Iris `lo..<hi`, and Rust's `lo..=hi` prints as Iris `lo..hi`. That is right
for Rust users, but the API guide must point out that the spellings
cross. The other languages name the bound: `atLeast(lo)` and `range(lo).below(hi)`
in Java and TypeScript, `int_range(lo, below=hi)` in Python, and
`Range{.from = lo, .below = hi}` in C++.

### 7. Patterns use the host's interpolation idiom

A line pattern is literal text with captures in it. Each language already
has a way to put values into text:

| Language | Pattern form |
|---|---|
| Rust | `pattern!("DIMENSION : {n}")`, capturing identifiers from scope like `format!` (Rust 1.58+) |
| C++ | `pattern("DIMENSION : {}", n)`, with placeholders checked against arguments at compile time, like `std::format_string` |
| Python | `t"DIMENSION : {n}"` (template strings, PEP 750, Python 3.14+); a tuple `("DIMENSION : ", n)` on older Pythons |
| TypeScript | A tagged template, `` p`DIMENSION : ${n}` `` |
| Java, Kotlin | Varargs parts, `pattern("DIMENSION : ", n)`; Kotlin's string templates are eager, and Java's were withdrawn |

An interpolated capture handle becomes a capture, `{n:2..46340}`. A
reference to an already bound value must be explicit, `ref(x)`, which
prints `{$x}`. The builder never guesses between the two. Unnamed and
discarded captures are values too: `validate(int)` prints `{:int}`, and
`discard()` prints `{_}`. These helpers avoid the names of Iris statements
such as `skip`. A trailing `...` is a marker value, `rest_of_line`,
never literal text.

### 8. Errors point at the host code

Every builder statement records where it was called: Rust
`#[track_caller]`, C++ `std::source_location` default arguments, Java and
Kotlin `StackWalker`, Python frames (skipping functions marked
`@track_caller`), and TypeScript `Error().stack` where available. A `build()` error shows the language error code and message, the
line of emitted text, and the host call site. A check failure while reading
has a reader spec location. The builder maps it back to the host call site,
so that `E-CHECK-FAILED` names the line of the user's program.

### 9. Composition is the point

A helper function that takes a scope and returns handles is a reusable
fragment, for example "read a TSPLIB matrix of size `n`". Iris has no
abstraction of its own, so this is the main thing builders add besides
types. Helpers must be safe to call twice, which principle 3's
redefinition rule ensures, and the API guides show a helper in every
language. Kotlin can pass the scope as a context parameter (stable since
Kotlin 2.4), if `iris-kotlin`'s Kotlin baseline allows it. Otherwise
helpers are extensions on the scope type. Java helpers are plain static
methods, since the open block is a scoped value.

### 10. Builders follow language versions

A builder targets one language version, and from Iris 1.0 on it prints
that version's line at the top
([PLAN.md §10.2](../PLAN.md#102-compatibility-between-01-and-10)).
Constructs added in 1.0 appear in the builder only for 1.0. Variant types,
if 1.0 adds them
([idea entry](1.0-ideas.md#add-variant-types-for-values-with-several-forms)),
map to each language's sum types: Rust enums, C++ `std::variant`, sealed
interfaces in Java and Kotlin, Python classes matched with `match`, and
TypeScript discriminated unions. Value types follow the data model. If 1.0
adds integers wider than 64 bits, handles gain a matching host type.

## The running example

All the sketches build [`formats/jobshop/jobshop.iris`](../formats/jobshop/jobshop.iris):

```iris
format jobshop

comment "#" "+++"
instances {
  sections skip other {
    "instance {name:word}" optional
    "{n_jobs:1..} {n_machines:1..}" last {
      check n_jobs <= (gecode.int_max - 2) / 2 / n_machines "there are too many jobs and machines"

      repeat n_jobs as jobs {
        each job
        repeat n_machines as steps {
          each step
          machine: 0..<n_machines
          duration: 0..
        }
      }
    }
  }

  check distinct(job.machine) "a job uses a machine twice; each job must use every machine once"
  check sum(j in jobs: sum(j.duration)) <= gecode.int_max "the total duration is too large"
}
```

The resulting data has `name` (string, depth 0, optional), `n_jobs` and
`n_machines` (int, depth 0), and `machine` and `duration` (int, depth 2).
The builder's text differs from this file in one way only: it names the
records and the comprehension variable itself, and it omits `each step`
and `as steps`, since nothing refers to them.

For brevity the sketches shorten the two long check messages to `MSG_*`
(`msg*` in Java).

### Rust

Builders suit specs assembled at run time. For a spec fixed at compile
time, the `iris!` macro (PLAN.md §8.1) is the more natural route in Rust:
it checks the spec when the program compiles. The macro is also planned to
generate the same typed handles from the text, as described in
[Type safety for text specs](#type-safety-for-text-specs).

`rust/gecode-iris/tests/build_jobshop.rs` compiles this program.

```rust
use gecode_iris::build::{distinct, gecode, lit, pattern, sum, sum_over, word, SpecBuilder};

let mut b = SpecBuilder::new("jobshop");
b.comment(["#", "+++"]);
let (name, n_jobs, n_machines, machine, duration) = b.instances(|i| {
    let name = i.optional_capture("name", word());
    let n_jobs = i.capture("n_jobs", lit(1)..);
    let n_machines = i.capture("n_machines", lit(1)..);
    let (job, machine, duration) = i.sections().skip_other().body(|s| {
        s.arm(pattern!("instance {name}")).optional();
        s.arm(pattern!("{n_jobs} {n_machines}")).last().body(|a| {
            a.check(n_jobs.le((gecode::INT_MAX - 2) / 2 / n_machines))
                .message("there are too many jobs and machines");
            a.repeat(n_jobs, |job| {
                let (machine, duration) = job.repeat(n_machines, |step| {
                    (step.read("machine", lit(0)..n_machines.ex()), // prints 0..<n_machines
                     step.read("duration", lit(0)..))
                });
                (job.record(), machine, duration)
            })
        })
    });
    i.check(distinct(job.get(machine))).message(MSG_DISTINCT);
    i.check(sum_over(job, |j| sum(j.get(duration))).le(gecode::INT_MAX)).message(MSG_TOTAL);
    (name, n_jobs, n_machines, machine, duration)
});
let spec = b.build()?; // parses and checks the emitted text

let mut params = Params::new();
params.set("instance", "la01");
let data = spec.read_path("jobshop1.txt", &params)?;
let jobs: i64 = data.get(n_jobs);
let first_job: &[i64] = data.get(machine).row(0);
let label: Option<&str> = data.get(name);
```

Rust has no overloading, so the comprehension is `sum_over`, beside the
`sum` that takes arguments. A statement needs its scope, so principle 2's
`handle.set(expr)` is `s.set(handle, expr)`. The dynamic forms:

```rust
let x: Field<Int> = s.forward("x"); // the annotation gives the type
s.if_(defined(x), |t| {
    t.let_("y", x + 1);             // use before the definition
});
s.read(x, int());                   // the first definition binds it
let again: DynField = s.read(x, float()); // redefined: use the returned handle
s.check(lit("1").dynamic().ne(1));
let zz = s.undefined("zz");
s.check(!defined(zz));
let end: Field<Str> = s.forward("_"); // `store _` keeps an entry named `_`
s.repeat_until("END").store(end);
```

`.ex()` turns a handle into a scalar expression, as a range bound requires.
On a handle it only asserts that the value is a scalar, and nothing checks the
assertion: on an array, `s.let_("v", a.ex())` returns a scalar-typed handle, and
`s.set(x, a.ex())` compiles. A wrong assertion passes `build()` and reading.
`data.try_get` then reports the mismatch and `data.get` panics, so it never
yields a wrong value. Use `.ex()` only where the value is a scalar. A host
constant has `.ex()` too, so that it can bound a range beside another
expression: `lit(0)..=gecode::INT_MAX.ex()`.

Every statement method takes `#[track_caller]`, so each statement records the
line that called it. A `BuildError` displays the language error code and
message, the line of emitted text, and the call site:

```text
E-SPEC-PLACEMENT: builder.iris:2: 'instances' must be the last statement of the spec
     2 | after: int
call site: src/main.rs:12:7
```

`BuildError::failure` keeps the original failure, with its code and its
location in the emitted text, and `BuildError::call_site` gives the call site.
The builder's own checks report a call site too. The spec that `build()` returns
keeps a sorted map from each statement's offset in the text to its call site, so
a failure while reading gets one as well: `Error::call_site` returns it, and the
rendered error ends with a `call site:` line. A failure that names no statement,
such as `E-TRAILING-INPUT`, has no call site, and neither does a spec parsed
from text. Catalog rejections are summaries in the data, so they have none.

The call site is that of the statement the failure belongs to, which can differ
from the emitted line. Some language errors sit on the token after their
statement: `b.read("bad", rest().dim(2))` followed by `b.read("next", int())`
shows the line `next: int`, as a text spec would, and names the call that read
`bad`. In the same way, a repeat's own failures keep the location of its last
body statement and name the repeat, and a section arm's failures keep the
location of `sections` and name the arm.

`#[track_caller]` reaches only so far:

- A statement in a block closure records its own line, which is the line you
  want. No closure is ever part of the chain.
- A statement in a helper function records the line inside the helper. Put
  `#[track_caller]` on the helper to record the helper's caller.
- A statement method used as a function value, such as `Scope::skip_line` passed
  to `for_each`, records its own definition in `gecode-iris-build`. Call it from a
  closure instead.
- `pattern!` writes no statement. It expands to a `Pattern` value, and the
  `line`, `find` or `arm` that takes the value records the call site.
- Expressions record nothing, so a failure in one has its statement's call site.
- Guard methods such as `message` and `store` belong to the statement they
  change, which keeps the call site of its opening call. A misused guard reports
  the guard's own line. A `}` has the call site of the method that wrote the
  block, which is `body` for a guard.

No statement method is a trait method, so nothing on the statement path loses
the attribute that way.

### C++

C++ can overload every operator Iris has, and `pattern` checks its
placeholders at compile time, as `std::format` does. Gecode 7 models are C++
programs, so this builder is also the way to keep a model and its reader in
one language. C++26 reflection (available in GCC 16 with `-freflection`)
could later read data into a user `struct`. The builder must not depend on
it while the library targets C++23.

```cpp
#include <iris/build.hpp>
using namespace iris::build;

SpecBuilder b("jobshop");
b.comment({"#", "+++"});
auto handles = b.instances([](auto& i) {
  auto name = i.optional_capture("name", word);
  auto n_jobs = i.capture("n_jobs", Range{.from = 1});
  auto n_machines = i.capture("n_machines", Range{.from = 1});
  auto jobs = i.sections({.skip_other = true}, [&](auto& s) {
    s.optional_arm(pattern("instance {}", name));
    return s.arm(pattern("{} {}", n_jobs, n_machines), {.last = true}, [&](auto& a) {
      a.check(n_jobs <= (host::int_max - 2) / 2 / n_machines,
              "there are too many jobs and machines");
      return a.repeat(n_jobs, [&](auto& job) {
        auto [machine, duration] = job.repeat(n_machines, [&](auto& step) {
          return std::tuple{step.read("machine", Range{.from = 0, .below = n_machines}),
                            step.read("duration", Range{.from = 0})};
        });
        return std::tuple{job.record(), machine, duration};
      });
    });
  });
  auto [job, machine, duration] = jobs;
  i.check(distinct(job.get(machine)), msg_distinct);
  i.check(sum(job, [&](const auto& j) { return sum(j.get(duration)); }) <= host::int_max,
          msg_total);
  return std::tuple{name, n_jobs, n_machines, machine, duration};
});
auto [name, n_jobs, n_machines, machine, duration] = handles;
iris::Spec spec = b.build(); // throws iris::build::BuildError; try_build returns std::expected

auto data = spec.read("jobshop1.txt", {{"instance", "la01"}});
std::int64_t jobs = data.get(n_jobs);
std::span<const std::int64_t> first_job = data.get(machine)[0];
std::optional<std::string_view> label = data.get(name);
```

`cpp/tests/build_test.cpp` builds this program, compares its text with the
text spec, and reads every jobshop case with it. The handles are bound
after each block returns, so that names inside a block do not shadow them
under `-Wshadow`. The free functions (`sum`, `distinct`, `pattern`, the type
constants `int_`, `float_`, `num`, `word`, `rest`, and so on) are function
objects, so argument-dependent lookup never replaces them with functions of
the same name, such as `std::min`. `host::int_max` prints `gecode.int_max`;
the public headers never name Gecode. A host constant is an integer operand;
`lit(host::int_max)` makes it an `Expr<std::int64_t>`, and
`host::int_max.dynamic()` an `Expr<Dynamic>` that can be indexed or iterated,
as in `size(host::int_max.dynamic())`. `else_if` and `else_` are calls on the
object that `if_` returns, and an optional arm is `optional_arm`. Copying an
expression copies its whole operation list, so a long chain built in a loop
should move it: `total = std::move(total) + x`.

A `BuildError` has a kind, the language error if there is one, the emitted
text and line, and the `std::source_location` of the host call. The spec
that `build()` returns keeps the call site of each emitted line, so a
failure while reading or cataloging gets one as well:
`iris::Error::call_site()` returns it, and `what()` ends with a
`call site: file:line:column` line before any earlier rejection. As in
Rust, a failure that names no statement, such as `E-TRAILING-INPUT`, has no
call site, nor does a catalog rejection, which is a summary in the data, nor
a spec parsed from text.

C++ errors locate the reader spec by line alone, and the builder prints one
statement per line, so call sites are mapped by line. A line gets the call
site of the first statement that starts on it, else of the call that wrote
the block a `}` closes. The parser, resolver and reader record, inside the
error, the line of the statement an error belongs to where the error lies
past it, and that line picks the call site: the declaration before the
token where its form is checked, the statement at the end of a block, the
`if` whose block is nested too deeply, a section arm for failures found at
its `sections` line, and a repeat for failures at its last body line. The
error's code, message and location do not change. Statements that share a
line, such as an `if` with an empty block and the `else` after it, share
the first one's call site.

A parameter handle also sets its parameter: after
`auto scale = b.param("scale", 3)`, `params.set(scale, 3)` writes the
integer in decimal, and a float handle's value in the shortest form that
reads back as the same float, with `.0` on a whole number. A value of
another type, or a handle to an array, does not compile.

The dynamic forms:

```cpp
auto x = s.forward<Field<std::int64_t>>("x");
s.read(x, int_);                   // the first definition binds it
auto again = s.read(x, float_);    // redefined: returns a DynField
auto mixed = (lit("1").dynamic() == 1);
auto zz = s.undefined("zz");
auto end = s.forward<Field<std::string>>("_"); // only a store binds it
```

### Python

`with` blocks match Python's own nesting, and names defined inside them
remain visible afterwards. Type stubs carry the handle types, so that
pyright and mypy know `data[machine]` is `list[list[int]]`. The builder is
pure Python; `build()` parses the emitted text with the Rust parser and
keeps each statement's call site. Patterns are t-strings on Python 3.14 and
later; on older versions the second arm is `(n_jobs, " ", n_machines)`.
`python/examples/build_jobshop.py` runs this program with tuple patterns.

```python
from gecode_iris import build as ib
from gecode_iris.build import gecode

b = ib.SpecBuilder("jobshop")
b.comment("#", "+++")
with b.instances() as i:
    name = i.optional_capture("name", ib.word)
    n_jobs = i.capture("n_jobs", ib.at_least(1))
    n_machines = i.capture("n_machines", ib.at_least(1))
    with i.sections(skip_other=True) as s:
        s.arm(t"instance {name}", optional=True)
        with s.arm(t"{n_jobs} {n_machines}", last=True) as a:
            a.check(
                n_jobs <= ib.div(ib.div(gecode.INT_MAX - 2, 2), n_machines),
                "there are too many jobs and machines",
            )
            with a.repeat(n_jobs) as jobs:
                jobs.as_name("jobs")  # error messages name the records
                with jobs.repeat(n_machines) as steps:
                    machine = steps.read("machine", ib.int_range(0, below=n_machines))
                    duration = steps.read("duration", ib.at_least(0))
    job = jobs.record
    i.check(ib.distinct(job[machine]), MSG_DISTINCT)
    i.check(ib.sum(job, lambda j: ib.sum(j[duration])) <= gecode.INT_MAX, MSG_TOTAL)
spec = b.build()  # raises ib.BuildError, naming the host line

data = spec.read("jobshop1.txt", params={"instance": "la01"})
data[n_jobs]   # int
data[machine]  # list[list[int]]; data.array(machine) gives numpy
data[name]     # str | None
```

The dynamic forms:

```python
x = b.forward("x", ib.IntField[int])
b.read(x, ib.int_)  # the first definition binds it
again = b.read(x, ib.float_)  # redefined: returns a DynField
mixed = ib.lit("1").dynamic() == 1
zz = b.undefined("zz")
end = b.forward("end", ib.StrField[str | None])  # only a store binds it
```

### Kotlin

`iris-kotlin` wraps the Java builder in a type-safe builder, in the style of
kotlinx.html and the Gradle Kotlin DSL. It uses lambdas with receivers
marked `@DslMarker`, so that an inner block cannot call an outer scope's
methods by accident. Delegated properties name entries. The handle classes
are Java's, so the class that carries the handles out spells them.

```kotlin
class Shop(
    val name: StringField<String?>, val nJobs: IntField<Long>, val nMachines: IntField<Long>,
    val machine: IntField<List<LongArray>>, val duration: IntField<List<LongArray>>,
)

val jobshop = irisSpec("jobshop") {
    comment("#", "+++")
    instances {
        val name by optionalCapture(word)
        val nJobs by capture(atLeast(1), name = "n_jobs")
        val nMachines by capture(atLeast(1), name = "n_machines")
        val (job, machine, duration) = sections(skipOther = true) {
            arm("instance ", name, optional = true)
            arm(nJobs, " ", nMachines, last = true) {
                check(nJobs le (gecode.INT_MAX - 2) / 2 / nMachines, "there are too many jobs and machines")
                repeat(nJobs) {
                    val (machine, duration) = repeat(nMachines) {
                        read("machine", 0..<nMachines) to read("duration", atLeast(0))
                    }
                    Triple(record, machine, duration)
                }
            }
        }
        check(distinct(job[machine]), MSG_DISTINCT)
        check(sum(job) { sum(it[duration]) } le gecode.INT_MAX, MSG_TOTAL)
        Shop(name, nJobs, nMachines, machine, duration)
    }
}

val shop = jobshop.handles
val data = jobshop.spec.read(path, params("instance" to "la01"))
val jobs: Long = data[shop.nJobs]
val firstJob: LongArray = data[shop.machine][0]
val label: String? = data[shop.name]
```

The dynamic forms:

```kotlin
val x = forward<Int>("x")
read(x, int())                      // the first definition binds it
val again = read(x, float())        // redefined: returns a dynamic handle
val mixed = lit("1").dynamic() eq 1
val zz = undefined("zz")
val end = forward<String>("_")      // only a store binds it
```

### Java

Java 27 is the baseline, so the builder can use every final feature up to
it. Three of them shape the design, and the others are deliberately left
out (see the end of this section).

**The open block is a scoped value.** Scoped values (JEP 506, final in
Java 25) let a caller share an immutable value with every callee for a
bounded extent. That is exactly the "current block" of a builder. Each
block statement binds a new scope while its body runs. The statements are
static methods that add to whatever scope is bound, so the body lambdas
take no parameters, and the code reads like the Iris text it prints. This
has three consequences:

- Statements always go to the innermost open block. Principle 3's scope
  discipline holds by construction, as `@DslMarker` makes it hold in
  Kotlin.
- A helper is an ordinary static method that calls `read`, `check`, and so
  on, and returns handles. It needs no scope parameter, which is what
  Kotlin gets from context parameters.
- A statement made outside any open block throws `IllegalStateException`,
  naming the statement. That happens when a body lambda is kept and run
  later, or run on another thread.
- A statement that its block does not take, such as an arm outside
  `sections` or a setting inside a block, throws `BuildException` of kind
  `SCOPE` at that call. Java cannot reject it at compile time without a
  receiver per block, as Kotlin has.

**Handles are declared, and one generic `get` reads them.** Java has no
tuples, and lambdas cannot assign local variables, so handles cannot
easily come back out of nested blocks. Java therefore declares them, in the
style of jOOQ's fields and the JPA static metamodel. A factory fixes the
value type and depth: `int0`, `int2`, `string0`, ..., and `intN("x", depth)`
for a handle of any depth that reads a `Value`. The statements define the
handles. `build()` checks that each handle the spec uses is defined, at the
value type, depth and optionality it declares, and that no two handles name
one entry. The type argument is the data view type, so a single
`<V extends @Nullable Object> V get(Field<V>)` returns `Long`, `long[]`,
or `List<long[]>` without overloading, and erasure does not matter.
Subtypes by value type (`IntField<V>`, `StringField<V>`, ...) carry the
expression methods. JSpecify annotations on type arguments put absence
in the type: `StringField<@Nullable String>` reads as `String?` in
Kotlin, and NullAway checks it in JSpecify mode. NullAway's support for
generics is still partial. Handle constants are named in lower camel case, like the entries
they stand for, not in constant case. Handles for a spec built at run time are
locals (`var weight = int2("weight");`), which lambdas can capture because
they are effectively final.

**Records receive data.** This part is not implemented yet.
`spec.reader(Shop.class, MethodHandles.lookup())`
maps entries to the components of a record. Components match entries by
name, and `@Entry("n_jobs")` renames one. The method checks every
component's type against the entry's value type, depth, and optionality
once, when the reader is created, and then reads through the canonical
constructor. Passing a `Lookup` lets the library construct records in the
user's module without the module having to open its packages. Dynamic
values (`num`, and parameters without a default) are sealed interfaces of
records, so an exhaustive `switch` with record patterns takes them apart.
A 1.0 variant type would read the same way.

`dev.gecode.iris.build` implements the builder, and google-java-format lays
out the code. The handle constants and the spec are static members of a
class, so the spec is built when the class is initialized:

```java
import static dev.gecode.iris.build.Iris.*;

import dev.gecode.iris.Data;
import dev.gecode.iris.Params;
import dev.gecode.iris.Spec;
import dev.gecode.iris.build.*;
import java.util.List;
import org.jspecify.annotations.Nullable;

final class Jobshop {
  static final StringField<@Nullable String> name = string0("name").optional();
  static final IntField<Long> nJobs = int0("n_jobs"), nMachines = int0("n_machines");
  static final IntField<List<long[]>> machine = int2("machine"), duration = int2("duration");
  static final RecordName job = recordName();

  static final Spec spec =
      spec(
          "jobshop",
          () -> {
            comment("#", "+++");
            instances(() -> {
              sections(
                  skipOther(),
                  () -> {
                    arm(pattern("instance ", capture(name, word())), optional());
                    arm(
                        pattern(capture(nJobs, atLeast(1)), " ", capture(nMachines, atLeast(1))),
                        last(),
                        () -> {
                          check(
                              nJobs.le(Gecode.intMax.minus(2).div(2).div(nMachines)),
                              "there are too many jobs and machines");
                          repeat(
                              nJobs,
                              job,
                              () ->
                                  repeat(
                                      nMachines,
                                      () -> {
                                        read(machine, range(0).below(nMachines));
                                        read(duration, atLeast(0));
                                      }));
                        });
                  });
              check(distinct(job.get(machine)), MSG_DISTINCT);
              check(sum(job, j -> sum(j.get(duration))).le(Gecode.intMax), MSG_TOTAL);
            });
          });
}

// Through handles
Data data = Jobshop.spec.read(input, "input", Params.of("instance", "la01"));
long jobs = data.get(Jobshop.nJobs);
long[] firstJob = data.get(Jobshop.machine).getFirst();
@Nullable String label = data.get(Jobshop.name);

// Into a record (not implemented yet)
record Shop(@Nullable String name, @Entry("n_jobs") long nJobs,
            @Entry("n_machines") long nMachines, List<long[]> machine, List<long[]> duration) {}
var reader = Jobshop.spec.reader(Shop.class, MethodHandles.lookup());
Shop shop = reader.read(path, Params.of("instance", "la01"));
```

Java has no operator overloading, so expressions are method chains:
`plus`, `minus`, `times`, `div`, `mod`, `pow`, `and`, `or` and `not` on
integers, the comparisons `lt`, `le`, `gt`, `ge`, `eq` and `ne` on numbers
and strings, and `neg`. `lit` starts a chain where the first operand is a
literal, as in `lit(2).pow(x)`. A handle used as a pattern part reads a
word, as `{x}` does, and `capture(x, type)` gives `{x:type}`.
`Builder.source()` returns the text, and `spec(...)` is
`builder(...).build()`. A `BuildException` names the failed check with
`kind()`, and `callSite()` is the `StackTraceElement` of the statement:
the first frame on the stack outside the builder, found with
`StackWalker`. A statement in a helper method therefore names the helper's
line. When the language rejects the text, `failure()` gives the
`IrisException`, `text()` the emitted text, and `emittedLine()` the line
that the error names.

The dynamic forms:

```java
var x = forward("x", Int.class);
read(x, intType()); // the first definition binds it
var again = read(x, floatType()); // redefined: returns a dynamic handle
var mixed = lit("1").dynamic().eq(lit(1).dynamic());
var zz = undefined("zz");
var end = forward("_", Str.class); // only a store binds it
```

`forward` takes a kind token, `Int`, `Real`, `Num`, `Str` or `Dyn`. It
declares the name in the open builder, so its handle belongs to that
builder and reads a required value. The float token is `Real` because a
`Float` token would clash with `java.lang.Float`, which every compilation
unit imports on demand: beside `import dev.gecode.iris.build.*` the name
is ambiguous, and under `import module` the `java.lang` class wins. A
handle that a factory declares belongs to no builder, and `.optional()`
makes it optional.

`import module` (JEP 511, final in Java 25) imports every package the
module exports. Static members still need the static import of `Iris.*`.
A compact source file also imports `java.base`, whose `java.lang.reflect.Field`,
`java.lang.reflect.Type` and `java.util.regex.Pattern` make those three
simple names ambiguous. A file that names one of them adds a single-type
import, such as `import dev.gecode.iris.Field;`.
The site's Java examples can be compact source files (JEP 512, final in
Java 25): a bare `void main()` run with `java Jobshop.java`.

What the builder does not use:

- **Preview features**, because a published library cannot require
  `--enable-preview`. Java 27 still previews Lazy Constants (third preview,
  JEP 531). Once they are final, `spec` becomes a lazy constant: it is
  built on first use, and a build failure is no longer wrapped in an
  `ExceptionInInitializerError`. Until then, a test loads every spec class.
  Primitive types in patterns (fifth preview) are not needed.
- **String templates**, which were withdrawn, so patterns are varargs
  parts.
- **Value classes**, which have not been delivered. Handles are small,
  immutable final classes without identity semantics, so they can become
  value classes later without an API change.

### TypeScript

TypeScript can infer the most, and users expect it to, as Zod, Kysely, and
Drizzle have taught them. The builder returns the handles it exports, and
`read` returns a plain object whose type follows from them. The builder is
pure TypeScript over whatever `Spec.parse` the package provides, whether
WASM or native. How `int` maps to `number` or `bigint` is the TypeScript
package's decision, and the builder follows it.

```ts
import { spec, p, word, atLeast, range, gecode, div, distinct, sum } from "gecode-iris/build";

const jobshop = spec("jobshop", (b) => {
  b.comment("#", "+++");
  return b.instances((i) => {
    const name = i.optionalCapture("name", word);
    const nJobs = i.capture("n_jobs", atLeast(1));
    const nMachines = i.capture("n_machines", atLeast(1));
    const { job, machine, duration } = i.sections({ skipOther: true }, (s) => {
      s.arm(p`instance ${name}`, { optional: true });
      return s.arm(p`${nJobs} ${nMachines}`, { last: true }, (a) => {
        a.check(nJobs.le(div(div(gecode.intMax.minus(2), 2), nMachines)),
          "there are too many jobs and machines");
        return a.repeat(nJobs, (r) => ({
          job: r.record,
          ...r.repeat(nMachines, (step) => ({
            machine: step.read("machine", range(0).below(nMachines)),
            duration: step.read("duration", atLeast(0)),
          })),
        }));
      });
    });
    i.check(distinct(job.get(machine)), MSG_DISTINCT);
    i.check(sum(job, (j) => sum(j.get(duration))).le(gecode.intMax), MSG_TOTAL);
    return { name, nJobs, nMachines, machine, duration };
  });
});

const shop = jobshop.read(text, { instance: "la01" });
shop.nJobs;   // number (or bigint, per the package's integer mapping)
shop.machine; // number[][]
shop.name;    // string | undefined
```

Keys of the returned object are host names, and the entries keep their Iris
names: `shop.nJobs` reads entry `n_jobs`.

The dynamic forms:

```ts
const x = b.forward<number>("x");
b.read(x, int()); // the first definition binds it
const again = b.read(x, float()); // redefined: returns a dynamic handle
const mixed = lit("1").dynamic().eq(1);
const zz = b.undefined("zz");
const end = b.forward<string>("_"); // only a store binds it
```

## Type safety for text specs

Many users will keep writing `.iris` text and still want typed access.
`gecode-iris-sema` already infers each name's value types, depths, and possible
absence. That is enough to generate the same handles a builder returns:

- **`iris gen --lang L SPEC`** prints a module of declared handles, in the
  style of the Java section, for Rust, Python, C++, Java, Kotlin, or
  TypeScript. A name whose facts are not a single value type and depth
  gets a dynamic handle. Rust is the only language so far: it prints one
  `pub const` per entry, named like the entry, to use by path
  (`jobshop::n_jobs`). A name redefined with another value type, a depth
  that varies, a depth above 8 in Rust, and a parameter without a default
  are all dynamic. A name is `Maybe` when it is not always in the output,
  as in the builder. `repeat 0 {if 0 {x:int}}` runs no definition of `x`, but
  every read publishes its empty column, so `x` is not `Maybe`.
- **The `iris!` macro** (PLAN.md §8.1) expands to a plain `Spec` today.
  It will instead expand to a value with a `spec` field and one field per
  handle, generated at compile time, so that Rust needs no generated file.
- **In Java, an annotation processor** does the same at compile time.
  `@IrisSpec("jobshop.iris")` on a class generates its handle constants
  and a matching record.
- **Deserialization** into user types is the third route: serde in Rust and
  kotlinx.serialization in Kotlin, both planned, and records in Java (see
  the Java section). Rust and Kotlin check shapes when they read. Java
  checks once, when the reader is created. None of them checks when the
  program is compiled.

Builders and generated views share the handle types, so a program can move
from a text spec to a builder, or the other way, without changing its data
access.

## Testing builders

Each implementation's builder is tested in four ways:

1. **Round trip over the corpus.** A generator in the Rust workspace,
   `iris gen builder --lang L SPEC`, prints builder code that reconstructs a
   reader spec. CI runs it on every reader spec in `conformance/cases/0.1`
   and `formats` that `iris check` accepts, 773 distinct files today. It
   runs the generated programs in each language and compares their output
   with the original, as syntax trees. Whitespace, comments, redundant
   parentheses, and generated record names do not count as differences.
   Rust, Python and C++ have generators so far. The generated code must
   pass the language's own tools unchanged before it runs: rustfmt and
   Clippy for Rust; `ruff format`, `ruff check` and ty for Python; and
   clang-format and clang-tidy 22.1.8, with the `.clang-format` and
   `.clang-tidy` of `cpp/`, for C++, which then builds with the warnings
   of `cpp/` as errors. The C++ round trip also runs as the CTest test
   `builder-round-trip`, with the compiler and flags of the CTest build.
   This proves that the builder covers the language and that its printer is
   right. The comparison is a new `gecode-iris-conformance` subcommand. The
   generator doubles as a migration tool for users who want to move a text
   spec into code.
2. **Idiomatic format builders.** Each format in `formats/` gets a
   hand-written builder in every language. These are the site's examples.
   Their tests read the format's cases and compare the data with the
   expected results.
3. **Type tests.** Accesses must have the expected types, and misuse must
   fail to compile: Rust `trybuild`, TypeScript `@ts-expect-error` with
   `expect-type`, Python `assert_type` under pyright and mypy, C++
   `static_assert` and compile-failure tests in CTest, Java and Kotlin
   compilation tests.
4. **Builder checks.** Unit tests cover scope discipline, provenance,
   redefinition, unprintable text, capture binding, and the mapping of
   errors to host call sites.

## Prior art and sources

- Kotlin type-safe builders and `@DslMarker`:
  [kotlinlang.org/docs/type-safe-builders.html](https://kotlinlang.org/docs/type-safe-builders.html).
  Context parameters are stable in Kotlin 2.4:
  [What's new in Kotlin 2.4](https://kotlinlang.org/docs/whatsnew24.html).
- Python template strings, PEP 750, new in Python 3.14:
  [`string.templatelib`](https://docs.python.org/3.14/library/string.templatelib.html).
- Java string templates were withdrawn and removed from JDK 23:
  [JEP 465](https://openjdk.org/jeps/465),
  [JDK-8329949](https://bugs.openjdk.org/browse/JDK-8329949).
- Java features final by Java 25:
  [JEP 506, scoped values](https://openjdk.org/jeps/506),
  [JEP 511, module imports](https://openjdk.org/jeps/511), and
  [JEP 512, compact source files](https://openjdk.org/jeps/512). Java 27
  finalized no language features; Lazy Constants remain in preview
  ([JDK 27 release notes](https://www.oracle.com/java/technologies/javase/27-relnote-issues.html)).
- C++26 reflection (P2996) is implemented in GCC 16 behind `-freflection`:
  [GCC 16.1](https://isocpp.org/blog/2026/04/gcc-16.1).
- TypeScript 7, the native compiler, is stable:
  [InfoQ](https://infoq.com/news/2026/08/typescript-7-released/). Builder
  types need nothing beyond what TypeScript 5 already supports.
- Typed column handles: jOOQ. Expression overloading with a raising
  `__bool__`: SQLAlchemy and pandas. Inference from schema builders: Zod,
  Kysely, and Drizzle.
