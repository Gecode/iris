# rust

The Cargo workspace contains the standalone `iris-conformance` runner
(milestone M2), and four implementation crates for Iris 0.1 (milestone M3):
[iris-syntax](iris-syntax/README.md) owns source validation, the lossless
parser, and formatting; [iris-sema](iris-sema/README.md) owns parse-time
validity, resolution, and conservative value facts;
[iris-core](iris-core/README.md) reads scalar and array input, executes
repeats, index statements, line operations and sections, evaluates expressions,
and owns column data, canonical JSON, and dzn output;
[iris-cli](iris-cli/README.md) provides `iris check SPEC`, `iris read SPEC
FILE... [--summary|--json|--dzn] [--param NAME=VALUE]`, `iris list SPEC FILE...`,
`iris fmt PATH...`, `iris conformance DRIVER [ROOTS...]`, `iris --version`, and
the `iris-driver parse|read|catalog|dzn` conformance commands.
The runner uses the [conformance protocol](../conformance/README.md) and has no
dependency on implementation crates.

[iris-lsp](iris-lsp/README.md) provides `iris lsp`, the stdio language server
with parse and resolution diagnostics (milestone M4b).

[gecode-iris](gecode-iris/README.md) re-exports the public `iris-core` API. Its
package name is `gecode-iris`; Rust imports it as `gecode_iris`. All workspace
crates currently set `publish = false`; publication belongs to milestone M7.
Later work adds `iris-wasm` and, if built, `iris-lint`.
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
cargo test --locked -p gecode-iris --doc
cargo test --locked -p iris-cli --test complete
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

The parse integration test loads the whole version tree and selects parse
cases. The `read_conformance` test requires every discovered case in
`conformance/cases/0.1` and `formats` to pass through `iris-driver`, with no
exemptions. It prints counts by operation and area; new cases run automatically.
Read and catalog enforce the idle and expression budgets in
[Specification §10](../spec/0.1/10-work-budgets.md), with limits 2^20 and 2^20
+ 16*N for the full original input byte length N. Equality succeeds; an excess
charge immediately reports `E-BUDGET`. The allowances span every traversed
instance, including deferred rejections, and restart for each operation.
Rendering dzn consumes no language work budget.

Read input and run the full conformance test:

```sh
cargo run --locked -p iris-cli -- read path/to/spec.iris path/to/input.txt
cargo run --locked -p iris-cli -- read path/to/spec.iris path/to/input.txt --summary
cargo run --locked -p iris-cli -- read path/to/spec.iris path/to/input.txt --json
cargo run --locked -p iris-cli -- read path/to/spec.iris path/to/input.txt --dzn
cargo run --locked -p iris-cli -- list path/to/spec.iris path/to/input.txt --dzn
cargo test --locked -p iris-cli --test read_conformance -- --nocapture
```

The read command defaults to `--summary`: each file has a header with its path
and optional format name, followed by entry names, value types, scalar values,
and array shapes. Shapes report the outer size, the second-level row-length
range, and `x ...` for each deeper dimension, as Gecode does. Summary rendering
is iterative and linear in stored data and output size. Choose `--json` for
canonical JSON or `--dzn` for MiniZinc data; output options are mutually
exclusive. Multiple files concatenate their output, buffered until every file
has been read successfully. Repeated `--param NAME=VALUE` options preserve exact
strings and let later values replace earlier values for the same key.
`iris list SPEC FILE...` lists counts, usable names, and first rejection summaries in Gecode's
`-list` format. Parameters retain exact strings; defaults execute even when
overridden. `iris list --dzn` renders catalog data instead of the listing.
Driver `dzn` reads and renders one input; `catalog --format dzn` catalogs and
renders it. Both use the public `Data::write_dzn` API; `to_dzn` returns a
buffered byte vector like `to_json`.

`iris --version` prints the package version and supported language versions.
`read`, `list`, and `check` exit 0 on success, 1 for language, file, or resource
errors, and 2 for usage errors. `iris lsp` starts the stdio language server;
transport failures exit 1 and invalid arguments exit 2.

Format reader specs in place, preview changes, or format standard input:

```sh
cargo run --locked -p iris-cli -- fmt path/to/spec.iris path/to/directory
cargo run --locked -p iris-cli -- fmt --check --diff path/to/directory
cat path/to/spec.iris | cargo run --locked -p iris-cli -- fmt --stdin-filepath path/to/spec.iris -
cargo test --locked -p iris-cli --test fmt
```

Directories recurse into `.iris` files, skipping symlinks and including
`.gitignore` matches. `--check` leaves files unchanged and exits 1 when changes
are needed; `--diff` previews unified diffs without writing files and combines
with `--check`. Standard input normally produces formatted source on stdout.
The line width comes from `.editorconfig`'s `max_line_length`, defaulting to 100
when absent, `unset`, or `off`; all other properties are ignored. Source errors,
including invalid UTF-8, leave the file unchanged and make the command exit 1;
other files still run. See the [CLI README](iris-cli/README.md#formatting) for
stdin options and exit statuses. Do not reformat Specification examples or
conformance reader specs: some test layout or error positions deliberately.

Run cases through `iris` with an implementation's executable or wrapper command:

```sh
cargo run --locked -p iris-cli -- conformance '<command>' \
  --driver-id '<stable-id>' --filter '<path-substring>' --version 0.1 \
  --timeout 30 <cases-dir>...
cargo run --locked -p iris-cli -- conformance target/debug/iris-driver ../conformance/cases/0.1/smoke
```

`iris conformance` calls the `iris-conformance` library directly. With no roots,
it uses `conformance/cases/0.1` relative to the invocation directory (run from
the repository root or supply explicit roots). Version defaults to `0.1`, the
only supported language version; timeout defaults to 30 positive seconds per
case. The driver string uses shell quoting for its executable and prefix
arguments, without running a shell. Quote a path containing spaces inside that
string. Options match `iris-conformance run`; its `--driver` becomes a positional
operand. The same roots, options, and driver produce the same report and exits:
0 when all cases pass or have selected exemptions, 1 for failures or unexpected
passes, and 2 for configuration, case-loading, or zero-selection errors.

Run the full Rust driver suite with the standalone runner after the workspace build:

```sh
cargo run --locked -p iris-conformance -- run --driver "\"$PWD/target/debug/iris-driver\"" ../conformance/cases/0.1
cargo run --locked -p iris-conformance -- run --driver "\"$PWD/target/debug/iris-driver\"" ../formats
```

The [conformance workflow](../.github/workflows/conformance.yml) runs both roots
against Rust and the pinned Gecode adapter on every push and pull request, on
Ubuntu and macOS. The workspace build supplies both the runner and `iris-driver`;
Rust uses no driver ID or exemptions.

Driver IDs select precise known-failure annotations and are independent of the
executable's name. Omit the ID to apply no exemptions. See
`iris conformance --help` and the
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

Checks and structural record expressions are implemented. The full conformance
test runs all 932 cases (155 parse, 730 read, 28 catalog, 19 dzn). Expression
and lifted traversal use iterative borrowed views;
false-check diagnostics retain witness paths and saved record positions. The
differential test has 3,072 probes, including 640 dzn probes at seed 73010, and
a separate 50-value check of the documented Gecode float precision gap. Dzn
tests check linear work at 4× input size, I/O failures, and 65,536 dimensions
without recursion. See the [core README](iris-core/README.md) for the
complexity regressions and separate read/evaluator doubling measurements.
