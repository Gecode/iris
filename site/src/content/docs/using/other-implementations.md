---
title: Other implementations
description: The status of the Python binding, the C++ and Java implementations, and Gecode.
editUrl: false
prev: false
next: false
---

Rust is the reference implementation and the only one available today. Use
the [CLI](/tools/cli/) to read input into JSON or dzn, or embed the
[Rust API](/using/rust/) in a host program.

## Planned implementations

- **Python:** a binding to the Rust crate.
- **C++:** a native implementation.
- **Java and Kotlin:** a native Java implementation with a Kotlin module.

None of them has a release date. The
[conformance suite](https://github.com/Gecode/iris/blob/main/conformance/README.md)
holds the language-neutral cases that every implementation must pass.

## Gecode

Gecode 6.5.0 reads Iris 0.1 through its own reader. Iris 0.1 is the Gecode
6.5.0 language, apart from the
[documented differences](/specification/0.1/appendix-c-differences/). Until
the Gecode `release-6.5.0` tag exists, the reference is commit
`5e7bbd5c26b0702c6fd652356b2289da0710ac3c`. Language versions are separate
from package versions.

Gecode 7 will use Iris 1.0. That integration is planned, without a release
date.
