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
`builder_code`. There are backends for Rust and Python. `Language::has_view`
says whether a language also has a view printer; Python has none yet.

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
but only keywords are reserved, since a view has no generated names beside its
own. An identifier stops at 40 columns, which gives a declaration three layouts.
A new language adds a printer, not a change to `View`.

From `rust/`:

```sh
cargo test --locked -p gecode-iris-gen
cargo test --locked -p gecode-iris --test view_formats --test view_soundness -- --nocapture
cargo test --locked -p gecode-iris --test builder_round_trip -- --nocapture
```

Both round trips print `compared C, skipped S, extra E, differences D`. They
fail on any difference, when compared and skipped specs do not add up to the
corpus, or when fewer than 770 specs are compared. `tests/round_trip/mod.rs` in
gecode-iris holds what they share: the corpus, the extra specs of the layout
and lint rules, and the limit specs.
