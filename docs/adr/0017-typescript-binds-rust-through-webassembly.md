# 0017. TypeScript binds Rust through WebAssembly

## Status

Accepted, 2026-11-03.

## Context

[ADR 0004](0004-native-cpp-java-python-binding.md) makes C++ and Java
native implementations and has Python bind the Rust crate. TypeScript
users need a package that runs in Node, browsers, Deno, Bun, and edge
runtimes such as Cloudflare Workers, and the TypeScript spec builder
needs `Spec.parse`, both error locations, and entry lookup from it
([PLAN.md §8.15](../../PLAN.md#815-spec-builders),
[§8.16](../../PLAN.md#816-typescript-typescript)). The playground already
runs the Rust reader as WebAssembly through the crate `iris-wasm`, whose
optimized module measures 645 KiB raw and 271 KiB gzipped
([§8.10](../../PLAN.md#810-documentation-site-site)).

## Decision

The npm package `gecode-iris` binds the Rust crate through WebAssembly.
It is not a separate implementation of the reader language.

A new crate, `rust/iris-js`, is the boundary, built with wasm-bindgen
like `iris-wasm` but separate from it, so the playground keeps its own
contract and size budget. A spec is parsed once into a handle. Read data
crosses the boundary in a binary layout built with `iris-core`'s validated
column constructors, and JavaScript copies it into typed arrays, so `Data`
holds no WebAssembly memory. JSON and dzn are rendered by `iris-core` on
request.

## Consequences

TypeScript shares Rust's behavior, so every fix and Iris 1.0 change
reaches it with no second reader to keep in step; a conformance difference
is a binding bug. One module serves Node, browsers, and edge runtimes.

The crate builds with `panic = "abort"`, so a panic surfaces in
JavaScript as a WebAssembly trap rather than an error, and the package
must recover from it. Calls are synchronous: the language's work budgets
bound every read, and a wall-clock deadline needs a Worker. Building the
package needs the Rust toolchain, the wasm32 target, and wasm-bindgen-cli.

## Alternative considered

A native Node addon would run only in Node and would need a binary per
platform. A pure-TypeScript reader would be a fourth independent
implementation to keep in step with the Specification. Reusing
`iris-wasm` would tie the package to the playground's string-only
contract, size limits, and memory cap.

## Condition to revisit

The API does not depend on the backend. If a pure-TypeScript backend
becomes worthwhile, for example for bundle size, it can replace
WebAssembly behind the same API.
