# Python changelog

## Unreleased

- Add the `gecode-iris` package, imported as `gecode_iris`, which binds the Rust
  reference implementation with PyO3 and adds no reader behavior of its own.
- Ship abi3 wheels for Python 3.10 and later on Linux (manylinux and musllinux,
  x86-64 and AArch64), macOS (Arm and Intel) and Windows (x86-64), and a source
  distribution.
- Add type stubs, optional numpy array access, and the `iris` console script.
- Add `gecode_iris.build`, which builds any Iris 0.1 reader spec from Python
  code with typed handles: blocks are `with` statements, line patterns are
  t-strings on Python 3.14 and later and tuples on every version, and builder
  errors name the caller's line.
- Let `data[handle]`, `handle in data`, `Data.ints` and `Data.array` take a
  builder handle, typed by the handle, and add `Error.call_site` and
  `CallSite` for errors while reading a built spec.
