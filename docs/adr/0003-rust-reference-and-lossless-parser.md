# 0003. Rust reference implementation and lossless parser

## Status

Accepted, 2026-10-31.

## Context

The reader needs precise source spans. The language server and formatter
also need syntax trees for incomplete or broken reader specs. Rust provides
safe, fast code that compiles to native targets and WASM
([PLAN.md §4.1–§4.2](../../PLAN.md#4-architecture)).

## Decision

Rust is the reference implementation. Its parser is lossless and
error-tolerant from the start, using a rowan concrete syntax tree.
`gecode-iris-syntax` owns lexing, parsing, and formatting; `gecode-iris-sema` owns
resolution and diagnostics; `gecode-iris-core` owns reading, evaluation, the data
model, work budgets, and output.

The CLI, language server, playground, and Python binding use these layers.
The language version is a parsing parameter from the start, to support
both Iris 0.1 and Iris 1.0 later.

## Consequences

One parser serves reading and editing tools, including broken input, so
precise spans and retained syntax are part of its initial design.

## Alternative considered

Separate parsers for the reader and editing tools would need to be kept in
sync. The shared parser avoids that duplication ([PLAN.md §4.2](../../PLAN.md#42-the-rust-reference-implementation-is-layered)).
