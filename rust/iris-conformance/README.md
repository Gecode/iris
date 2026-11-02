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

Publication is disabled until milestone M7, as for the other workspace crates.
