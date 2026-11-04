# iris-js

The WebAssembly boundary of the npm package `gecode-iris`, delivered in
milestone M4f ([PLAN.md §8.16](../../PLAN.md#816-typescript-typescript)).
The unpublished crate is built for `wasm32-unknown-unknown` by
`typescript/scripts/build.ts`; the [package guide](../../typescript/README.md)
covers the build and the TypeScript API.

The crate adds no semantics. Parsing, reading, errors, canonical JSON and dzn
all come from iris-core.

## Boundary contract

- `SpecHandle.parse(source, origin)` parses and checks a reader spec once.
- `read` and `catalog(input, origin, params)` return data in the binary
  transfer layout. `params` is a flat list of name/value pairs in caller order.
- `renderJson(data)` and `renderDzn(data)` render transfer bytes through
  iris-core, so JSON and dzn are produced only when the caller asks for them.
- A failure is thrown as a JSON envelope string:
  `{"kind":"language","document":D,"diagnostic":T}` carries the canonical
  error document `D` as text and the error's display text `T`;
  `{"kind":"host","category":C,"message":M}` has the category `size`,
  `unsupported` or `internal`. A trap is a `WebAssembly.RuntimeError`, never a
  string.
- The wasm32 export `instanceMemory()` is internal to the package and absent
  from its published declarations. It returns the instance's
  `WebAssembly.Memory`, which the package measures after each read.
- The off-by-default `fault-injection` feature adds the internal export
  `armTrap()`, which makes the next `read` or `catalog` trap with
  `unreachable`, as a panic does. Only the package's test build enables it,
  so the published module has no such export.

[`src/transfer.rs`](src/transfer.rs) documents the transfer layout. It aligns
every numeric array to 8 bytes, so JavaScript views leaves and offsets in place
as `BigInt64Array`, `Float64Array` and `Uint32Array`. Decoding uses checked
reads and iris-core's validated column constructors, so a malformed buffer is an
error, not a panic.

Compared with [iris-wasm](../iris-wasm/README.md), the playground's crate, it
keeps a parsed spec in a handle instead of taking strings per call, sets no
size limits or memory cap, offers no summary, table, check or format, and
returns binary data instead of rendered documents.

## Checks

From `rust/`:

```sh
cargo test --locked -p iris-js
cargo clippy -p iris-js --all-targets --locked -- -D warnings
cargo clippy -p iris-js --target wasm32-unknown-unknown --locked -- -D warnings
cargo +1.88.0 check -p iris-js --target wasm32-unknown-unknown --locked
```

Native tests call the same API without a JavaScript engine. Native clippy does
not compile the wasm-bindgen exports, so the wasm32 clippy run is needed too.
The binding dependency and the CLI must both be wasm-bindgen 0.2.129. The crate
builds with the workspace's `wasm` profile, which aborts on panic.
