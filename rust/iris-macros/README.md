# gecode-iris-macros

The procedural macro behind `gecode_iris::iris!`, described in
[PLAN.md §8.1](../../PLAN.md#81-rust-rust). It is work after Iris 0.1 with no
milestone of its own. This is an implementation crate: use the macro through
[gecode-iris](../gecode-iris/README.md), whose `iris!` passes its own crate
path to this one.

```rust
let row = gecode_iris::iris!(r#"
    n:int  # the row length
    row[n]:int
"#);
let data = row.spec.read(b"3 10 20 30", "input.txt", &gecode_iris::Params::new())?;
let n: i64 = data.get(row.n);
let dimacs = gecode_iris::iris!(file = "specs/dimacs-cnf.iris");
```

The macro checks the spec with [gecode-iris-syntax](../iris-syntax/README.md) and
[gecode-iris-sema](../iris-sema/README.md), in the same order as `iris check`, and
reports the first error as a compile error: the error code, then exactly the
text `iris check` prints. The crate never depends on `gecode-iris-core`
([§4.2](../../PLAN.md#42-the-rust-reference-implementation-is-layered)).

A valid spec expands to a block that declares a struct, `TypedSpec`, and
returns a value of it. Its `spec` field is `Spec::parse` on the same bytes and
origin. Each other field is a typed handle from
[gecode-iris-gen](../iris-gen/README.md)'s `rust_view`, the declarations that
`iris gen --lang rust` prints: the same identifier, the same `Field` type, and
`Field::declared` on the entry's name. The block imports the handle types from
`gecode_iris::build`, as the view module does. `rust_view` gives no handle the
identifier `spec`, so an entry named `spec` is the field `spec_2`, as in
`iris gen`. Other names follow the view generator's rules: snake case, and a
numeric suffix for a keyword or for an identifier that another entry has.
Code outside the block cannot name `TypedSpec`; the `gecode-iris` documentation
says how to keep the parts in a named type.

An inline spec has the origin `iris!`. A file path is relative to the calling
package's `CARGO_MANIFEST_DIR` and is its own origin. The expansion embeds the
file with `include_bytes!`, so Cargo rebuilds the caller when the file changes.

The crate also implements `pattern!`, which
[gecode-iris-build](../iris-build/README.md) exports. It splits a line pattern literal
into text and `{name}` interpolations, and expands to `Pattern` calls.

On stable Rust the compiler marks the whole literal or file path. On nightly
the macro marks the error's position inside a raw literal, or a cooked literal
without escapes.

From `rust/`:

```sh
cargo test --locked -p gecode-iris-macros
cargo test --locked -p gecode-iris --test macro_errors --test macro_handles
cargo test --locked -p gecode-iris --test macro_corpus -- --nocapture
```

The first runs the literal offset unit tests, and compares the handle fields
of the expansion with the declarations that `iris gen --lang rust` prints for
every format in `formats`, for specs with dynamic, optional, deep and
unusually named entries, and with the recorded jobshop view of
`iris-cli/tests/fixtures`. `macro_errors` compiles the invalid invocations in
`gecode-iris/tests/ui` with trybuild and compares the compiler's messages with
the committed `.stderr` snapshots. `macro_handles` compiles and runs typed
reads through the handle fields in `gecode-iris/tests/macro-handles/pass`, and
checks that a wrong value type, an optional handle read as required, and an
unknown field fail to compile, against the snapshots in
`gecode-iris/tests/macro-handles/fail`. Rerun either with `TRYBUILD=overwrite`
to bless changed messages. `macro_corpus` compiles every valid reader spec in
`conformance/cases/0.1` and `formats` through both forms, with all their
handle fields, then checks that each `spec` reads its cases exactly as
`Spec::parse` does.
