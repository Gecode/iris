# iris-conformance

The shared conformance runner for Iris implementations, part of milestone M2
in [PLAN.md §7.2](../../PLAN.md#72-runner-and-driver-protocol). It has no
dependency on the implementation crates. The CLI also uses its library to
provide `iris conformance`.

The [conformance contract](../../conformance/README.md) defines the case format,
driver protocol, runner commands, and grammar checks. From `rust/`:

```sh
cargo test --locked -p iris-conformance
cargo run --locked -p iris-conformance -- --help
```

The runner embeds two Specification files in `embedded/0.1/`, so that the
crate builds outside the repository, as in the Python source distribution:
`grammar.ebnf` is a copy of `spec/0.1/grammar.ebnf`, and `error-codes.txt`
lists the codes in the first column of `spec/0.1/errors.md`, one per line.
`tests/embedded.rs` fails when either differs from the Specification; after a
change to those files, update the copies to match.

Publication is disabled until milestone M7, as for the other workspace crates.
