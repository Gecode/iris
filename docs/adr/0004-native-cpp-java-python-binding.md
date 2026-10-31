# 0004. Native C++ and Java, Python binding Rust

## Status

Accepted, 2026-10-31.

## Context

Gecode already has a tested C++ reader. JVM users need a library without
native dependencies, while Python can use Rust through packaged wheels
([PLAN.md §4.1](../../PLAN.md#41-implementation-strategy-decided),
[§8.3](../../PLAN.md#83-java-and-kotlin-java),
[§8.4](../../PLAN.md#84-c-cpp)).

## Decision

C++ and Java are native implementations. C++ ports Gecode's implementation,
replacing Gecode types and allocation with standard library facilities and
a pluggable allocator. Java is pure Java, with a Kotlin extensions module.

Python binds the Rust crate through PyO3 and maturin. It is not a separate
implementation of the reader language.

## Consequences

Rust, C++, and Java are independent implementations, alongside Gecode
6.5.0. Their agreement tests the precision of the Specification, and
differential testing can expose disagreements. Python shares Rust's
behavior and speed; JVM users avoid native dependencies. The standalone
C++ library has no Gecode dependency.
