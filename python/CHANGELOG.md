# Python changelog

## Unreleased

- Add the `gecode-iris` package, imported as `gecode_iris`, which binds the Rust
  reference implementation with PyO3 and adds no reader behavior of its own.
- Ship abi3 wheels for Python 3.10 and later on Linux (manylinux and musllinux,
  x86-64 and AArch64), macOS (Arm and Intel) and Windows (x86-64), and a source
  distribution.
- Add type stubs, optional numpy array access, and the `iris` console script.
