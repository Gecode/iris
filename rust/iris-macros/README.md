# iris-macros

The procedural macro behind `gecode_iris::iris!`, described in
[PLAN.md §8.1](../../PLAN.md#81-rust-rust). It is work after Iris 0.1 with no
milestone of its own. This is an implementation crate: use the macro through
[gecode-iris](../gecode-iris/README.md), whose `iris!` passes its own crate
path to this one.

```rust
let spec = gecode_iris::iris!(r#"
    n:int  # the row length
    row[n]:int
"#);
let dimacs = gecode_iris::iris!(file = "specs/dimacs-cnf.iris");
```

The macro checks the spec with [iris-syntax](../iris-syntax/README.md) and
[iris-sema](../iris-sema/README.md), in the same order as `iris check`, and
reports the first error as a compile error: the error code, then exactly the
text `iris check` prints. A valid spec expands to `Spec::parse` on the same
bytes and origin. The crate never depends on `iris-core`
([§4.2](../../PLAN.md#42-the-rust-reference-implementation-is-layered)).

An inline spec has the origin `iris!`. A file path is relative to the calling
package's `CARGO_MANIFEST_DIR` and is its own origin. The expansion embeds the
file with `include_bytes!`, so Cargo rebuilds the caller when the file changes.

On stable Rust the compiler marks the whole literal or file path. On nightly
the macro marks the error's position inside a raw literal, or a cooked literal
without escapes.

From `rust/`:

```sh
cargo test --locked -p iris-macros
cargo test --locked -p gecode-iris --test macro_errors
cargo test --locked -p gecode-iris --test macro_corpus -- --nocapture
```

The first runs the literal offset unit tests. `macro_errors` compiles the
invalid invocations in `gecode-iris/tests/ui` with trybuild and compares the
compiler's messages with the committed `.stderr` snapshots; rerun it with
`TRYBUILD=overwrite` to bless changed messages. `macro_corpus` compiles every
valid reader spec in `conformance/cases/0.1` and `formats` through both forms,
then checks that each reads its cases exactly as `Spec::parse` does.
