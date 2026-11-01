# iris-cli

The `iris` command and Rust conformance driver, part of milestone M3 in
[PLAN.md §8.1](../../PLAN.md#81-rust-rust) and the
[§4.2 layers](../../PLAN.md#42-the-rust-reference-implementation-is-layered).
Only parse-and-resolve is implemented here. Neither command reads instance
input or executes reader-spec expressions, including parameter defaults.

From `rust/`:

```sh
cargo run --locked -p iris-cli -- check path/to/spec.iris
cargo run --locked -p iris-cli --bin iris-driver -- parse path/to/spec.iris --version 0.1
cargo test --locked -p iris-cli
cargo test --locked -p iris-cli --test parse_conformance -- --nocapture
```

`iris check SPEC` prints `SPEC: ok`, optionally with the format name, and exits
0. A language error exits 1 and prints a Gecode-compatible location/message
header followed by a source excerpt and caret. The added excerpt is a CLI
presentation choice: Gecode's parse diagnostics have only the header. Reported
virtual capture lines remain in the header; the excerpt uses the raw source.
The renderer escapes control characters in diagnostic text and excerpts.
Usage errors exit 2; file acquisition and resource failures exit 1.
`iris --version` reports package 0.1.0 and language 0.1.

`iris-driver parse SPEC [--params JSON] [--version 0.1]` implements the
[driver protocol](../../conformance/README.md#four-driver-commands). It emits
only the parse-success document (exit 0) or a canonical error (exit 1).
Misuse exits 2 and host failures exit 3, with empty stdout. Parse parameters
must be an empty JSON object; duplicate members, non-string values, malformed
Unicode, extra documents, unknown/repeated options, and unsupported versions
are rejected. The other protocol operations currently exit 2 as unsupported.

The integration test discovers and loads the entire `conformance/cases/0.1`
tree through the runner library, selects exactly `Operation::Parse`, and gives
those case directories to the existing runner. Non-parse cases are excluded
before invocation, never silently passed or exempted. Counts follow the current
tree, including generated Specification examples and pinned supplements.

Messages match the pinned reference for ASCII failures. A valid non-ASCII
character rejected by the lexer is rendered as its Unicode scalar; Gecode's
single-byte message can be invalid Unicode and cannot always be reproduced
byte for byte in canonical JSON. Codes and locations follow the Specification.
