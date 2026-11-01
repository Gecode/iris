# gecode-iris

The umbrella crate for the Iris reader API, part of milestone M3 in
[PLAN.md §8.1](../../PLAN.md#81-rust-rust), above the runtime layer in
[§4.2](../../PLAN.md#42-the-rust-reference-implementation-is-layered).
It re-exports all public items from [iris-core](../iris-core/README.md), its
only direct dependency. The package name is `gecode-iris`; Rust imports it as
`gecode_iris`. No library-name override is needed. Publication is disabled
until milestone M7, as for the other workspace crates.

The crate-level example parses a reader spec, reads input, and accesses an
integer scalar and a borrowed integer row. From `rust/`:

```sh
cargo test --locked -p gecode-iris --doc
```
