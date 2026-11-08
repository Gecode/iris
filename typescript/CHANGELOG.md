# TypeScript changelog

## Unreleased

- Add the `gecode-iris` package, which runs the Rust reference implementation
  through WebAssembly and adds no reader behavior of its own.
- Support Node 22.12 and later, browsers, and workerd and other edge runtimes. The
  package is ESM only and ships its declarations.
- Add the spec builder, `gecode-iris/build`, which builds reader specs in
  TypeScript with typed handles, callback blocks and `p` tagged-template
  patterns, and names the caller's line in its errors and in the errors of
  reading a built spec. `data.get(handle)` reads an entry with the handle's
  type, and `IrisError` gains `callSite`.
