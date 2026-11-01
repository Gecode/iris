# rust

This directory will hold the Cargo workspace of the Rust reference
implementation: the crates `iris-syntax`, `iris-sema`, `iris-core`,
`iris-cli` (the `iris` binary), `iris-lsp`, and `iris-wasm`, published under
the umbrella crate `gecode-iris`, and `iris-lint` if the linter is built.
`PLAN.md` §4.2 describes the layers, and §8.1, §8.7, §8.12, and §8.13
describe the crates. Milestone M3 builds the core and the CLI, M4b the
language server, M4e the formatter, and M6b decides on the linter.
