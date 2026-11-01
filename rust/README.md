# rust

The Cargo workspace contains the standalone `iris-conformance` runner
(milestone M2), and three implementation crates for Iris 0.1 (milestone M3):
[iris-syntax](iris-syntax/README.md) owns source validation, the lossless parser,
and formatting; [iris-sema](iris-sema/README.md) owns parse-time validity,
resolution, and conservative value facts; [iris-cli](iris-cli/README.md) provides
`iris check SPEC` and the `iris-driver parse` conformance command. The runner
uses the [conformance protocol](../conformance/README.md) and has no dependency
on implementation crates.

Later work adds `iris-core`, `iris-lsp`, `iris-wasm` and, if built, `iris-lint`,
published under `gecode-iris`.
[PLAN.md §4.2](../PLAN.md#42-the-rust-reference-implementation-is-layered)
describes the layers; §8.1, §8.7, §8.12 and §8.13 describe the crates. Milestones
M3, M4b, M4e and M6b cover the core/CLI, language server, formatter and linter.

Use the installed stable Rust toolchain with rustfmt and Clippy. The workspace
selects channel `stable`, not a numeric compiler pin. Python 3 is required only
for the canned driver in process tests. From this directory:

```sh
cargo build --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Check reconstruction of every valid-UTF-8 reader spec in the repository:

```sh
cargo test --locked -p iris-syntax repository_losslessness -- --nocapture
```

The parser tests also cover the six pinned Gecode reader specs, embedded pattern
nodes, malformed-input recovery, and all published nesting boundaries. See the
[crate README](iris-syntax/README.md) for its syntax/semantic boundary and fixture
regeneration.

Check a reader spec and run every parse-only conformance case:

```sh
cargo run --locked -p iris-cli -- check path/to/spec.iris
cargo test --locked -p iris-cli --test parse_conformance -- --nocapture
```

The integration test loads the whole version tree and selects cases by their
operation. Read, catalog, and dzn await `iris-core`; the driver reports them as
unsupported misuse (exit 2) if invoked directly.

Run cases with an implementation's executable or wrapper command:

```sh
cargo run --locked -p iris-conformance -- run --driver '<command>' \
  --driver-id '<stable-id>' <cases-dir>...
```

Driver IDs select precise known-failure annotations and are independent of the
executable's name. Omit the ID to apply no exemptions. See `run --help` and the
[contract](../conformance/README.md#running-cases) for filtering, timeouts and exits.
No adapter or real language cases are supplied by this crate.

These demonstrations use only canned fixture outputs:

```sh
cargo run --locked -p iris-conformance -- run --driver 'python3 iris-conformance/tests/canned-driver.py' iris-conformance/tests/fixtures/passing
cargo run --locked -p iris-conformance -- run --driver 'python3 iris-conformance/tests/canned-driver.py' --driver-id canned iris-conformance/tests/fixtures/known
cargo run --locked -p iris-conformance -- run --driver 'python3 iris-conformance/tests/canned-driver.py' --driver-id canned iris-conformance/tests/fixtures/failing
```

The first two exit 0; the third intentionally exits 1 and demonstrates failure
reports and a stale known-failure annotation. All fixtures remain under the
crate's tests directory. Another implementation needs only to supply the four
protocol commands; it does not need to link this library.
