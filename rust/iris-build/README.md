# gecode-iris-build

The Rust spec builder, described in
[PLAN.md §8.15](../../PLAN.md#815-spec-builders) and designed in
[docs/builders.md](../../docs/builders.md). Use it through
[gecode-iris](../gecode-iris/README.md), as `gecode_iris::build`.

A builder writes a reader spec. Every call adds a statement, and `build()`
prints deterministic text and parses it with `Spec::parse`, so the result is an
ordinary `Spec` and the language's own checks decide what is valid.

```rust
use gecode_iris::Params;
use gecode_iris::build::{SpecBuilder, int};

let mut b = SpecBuilder::new("row");
let n = b.read("n", int());
let row = b.read("row", int().dim(n));
let spec = b.build()?;

let data = spec.read(b"2 10 20", "input.txt", &Params::new())?;
let length: i64 = data.get(n);
let values: &[i64] = data.get(row);
```

Every defining statement returns a typed handle, a `Field<V, D, O>` with a value
type, a depth and a presence. Depths are static up to `D8`; deeper entries, and
dimensions counted at run time, read as a `ValueRef`, one level at a time. The
four dynamic forms in [docs/builders.md](../../docs/builders.md) reach every spec
`iris check` accepts.

The crate has no language checks of its own. It checks scope discipline,
provenance, explicit redefinition, printable text, and capture and forward
binding, and leaves the rest to `Spec::parse`. The statements are one flat list
and expressions are flat postfix operations, so printing and dropping never
recurse. The text matches `iris fmt` with no line limit.

Every statement method takes `#[track_caller]` and records its call site as a
`&'static Location`. `build()` keeps a sorted map from statement offsets in the
text to call sites, in the `BuildError` and in the returned `Spec`. An error shows
its language code and message, the line of emitted text, and the call site, and
a failure while reading, such as `E-CHECK-FAILED`, names the call site too. A
helper function of yours needs `#[track_caller]` to pass on its caller;
[docs/builders.md](../../docs/builders.md#rust) lists where tracking stops.

Handles are `Copy` and outlive their builder, so the crate interns names as
`&'static str`. Every distinct name supplied stays allocated for the life of the
process, including the names of dropped builders. A host that generates names
without limit therefore grows without limit.

From `rust/`:

```sh
cargo test --locked -p gecode-iris-build
cargo test --locked -p gecode-iris --test build_jobshop --test build_corpus --test build_types
```

Bless the `build_types` snapshots with `TRYBUILD=overwrite`.
