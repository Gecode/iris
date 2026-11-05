# gecode-iris-gen

Generates spec builder programs from reader specs, as described in
[docs/builders.md](../../docs/builders.md). It belongs to the builder work of
[PLAN.md §8.15](../../PLAN.md#815-spec-builders). It depends only on `gecode-iris-syntax`
and `gecode-iris-sema`, and `iris gen builder` is its command.

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
`builder_code`. Rust is the only backend so far.

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

From `rust/`:

```sh
cargo test --locked -p gecode-iris-gen
cargo test --locked -p gecode-iris --test builder_round_trip -- --nocapture
```
