# iris-cli

The `iris` command and Rust conformance driver, part of milestone M3 in
[PLAN.md §8.1](../../PLAN.md#81-rust-rust) and the
[§4.2 layers](../../PLAN.md#42-the-rust-reference-implementation-is-layered).
The user command checks and formats reader specs, reads inputs through
[iris-core](../iris-core/README.md), and starts the language server. The driver
implements parse-and-resolve, reading, and catalog. Parse-only requests never
execute expressions or parameter defaults.

From `rust/`:

```sh
cargo run --locked -p iris-cli -- lsp
cargo run --locked -p iris-cli -- check path/to/spec.iris
cargo run --locked -p iris-cli -- read path/to/spec.iris path/to/input.txt --json --param instance=2
cargo run --locked -p iris-cli -- list path/to/spec.iris path/to/input.txt
cargo run --locked -p iris-cli -- fmt path/to/spec.iris
cargo run --locked -p iris-cli -- fmt --check --diff path/to/directory
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

## Reading input

`iris read SPEC FILE... --json [--param NAME=VALUE]` prints one canonical data
document per input file. It requires `--json`. Split assignments at the first
`=`; the remainder is the exact string value. Repeated keys replace their earlier
values without moving their supplied positions. Parameters apply to every file.
Runtime language errors exit 1 with an input location, bounded excerpt,
byte-position caret, and active reader-spec line. No partial read data is printed.
File/serialization failures exit 1; usage errors exit 2. Summary and dzn output
belong to later tasks.

`iris list SPEC FILE... [--param NAME=VALUE]` matches Gecode's `-list`: print each
file's instance count, then numbered names when all are usable, with first
rejection summaries in `(failed: MESSAGE)`. Without usable names, only failed
instances get individual lines. Catalog traverses every instance even when the
caller supplies a selector.

## Language server

`iris lsp` serves framed LSP over stdin/stdout through
[iris-lsp](../iris-lsp/README.md), with parse and resolution diagnostics.
See that crate's README for editor startup, synchronization, and encodings.
Errors go to stderr; the server exits 0 after shutdown/exit, or 1 on failure.

## Formatting

`iris fmt PATH...` formats files in place using the
[approved style](../../docs/formatting.md). Directory operands recurse into
all regular files whose names end in `.iris`, including hidden files and
files matched by `.gitignore`. Explicit file operands may have any suffix.
Symlinks are skipped, including explicit operands; directory symlinks are
never traversed. Directory entries are processed in path order. A changed file
is written to a temporary file beside it, which then replaces the original with
the original's permissions, so a failed write leaves the original intact. The
directory must therefore be writable.

- `--check` leaves files unchanged, reports each file that would change on
  stderr, and exits 1 if any would change.
- `--diff` leaves files unchanged and prints unified diffs on stdout. Each
  header names the file, then a tab and `original` or `formatted`, so
  `patch -p0` applies the diff even to names with spaces. A difference alone
  does not change its exit status; combine it with `--check` to print diffs
  and exit 1 when formatting is needed.
- `-` must be the only operand. It reads standard input and normally prints
  the formatted reader spec on stdout, even if already formatted. `--check`
  suppresses that output; `--diff` prints a diff instead. With both options,
  it prints only the diff and checks for changes.
- `--stdin-filepath PATH` uses that path for `.editorconfig` lookup and
  diagnostics; the path need not exist and is never written. Without it,
  standard input uses width 100 without looking up configuration and is named
  `<stdin>` in diagnostics and diffs. This option requires `-`.

For files and named standard input, only `max_line_length` from `.editorconfig`
affects formatting. Lookup uses `ec4rs`, including ancestor configurations,
globs, overrides and `root = true`; unlike `ec4rs` alone, an `.editorconfig`
that exists but cannot be opened is an error rather than skipped. An absent or `unset` property, or `off`,
means width 100. Other values must be positive integers; invalid widths and
configuration read failures are reported with exit status 1. No other property
controls formatting: indentation is always two spaces, for example.

Syntax errors and invalid UTF-8 are source errors: they are reported with a
location and source excerpt, the file is left unchanged, and the command exits
1. Invalid bytes are replaced only in the diagnostic excerpt. Formatting needs
valid syntax, without requiring successful semantic analysis.
Processing continues after per-file failures, so successful files are still
formatted. Any source, configuration, or I/O failure makes the overall exit
status 1, regardless of other successes. Otherwise it exits 0, except when
`--check` finds changes. Usage errors exit 2.

The two formatter dependencies are `ec4rs` 1.2 for the required EditorConfig
lookup and `similar` 2.7 for unified diffs, including context and missing-final-
newline markers. Both use their default features and add no transitive
dependencies. This avoids maintaining a configuration parser or diff algorithm.
CLI tests use temporary directories with a `root = true` boundary that
supplies no properties unless a test sets them; configuration above the
temporary directory cannot affect results.

## Conformance driver

`iris-driver parse SPEC [--params JSON] [--version 0.1]` implements the
[driver protocol](../../conformance/README.md#four-driver-commands). It emits
only the parse-success document (exit 0) or a canonical error (exit 1).
Misuse exits 2 and host failures exit 3, with empty stdout. Parse parameters
must be an empty JSON object; duplicate members, non-string values, malformed
Unicode, extra documents, unknown/repeated options, and unsupported versions
are rejected.

`iris-driver read SPEC INPUT [--params JSON] [--version 0.1]` returns canonical
data (exit 0) or a canonical language error (exit 1). It buffers output before
writing stdout. Host failures exit 3; unsupported reached features and misuse
exit 2, both with empty stdout. Parameters are decoded as ordered string pairs,
with exact values and duplicate rejection. Unknown names fail before input
UTF-8 validation. Parameter defaults execute even when overridden.

`iris-driver catalog SPEC INPUT [--params JSON] [--version 0.1] [--format json]`
returns canonical catalog data. Both operations defer eligible instance
rejections while keeping hard errors immediate. `dzn` and catalog's dzn format
remain unsupported (exit 2).

The integration test discovers and loads the entire `conformance/cases/0.1`
tree through the runner library, selects exactly `Operation::Parse`, and gives
those case directories to the existing runner. Non-parse cases are excluded
before invocation, never silently passed or exempted. Counts follow the current
tree, including generated Specification examples and pinned supplements.

Messages match the pinned reference for ASCII failures. A valid non-ASCII
character rejected by the lexer is rendered as its Unicode scalar; Gecode's
single-byte message can be invalid Unicode and cannot always be reproduced
byte for byte in canonical JSON. Codes and locations follow the Specification.

The `read_conformance` progress test discovers both case roots, validates the
exact pending list and its rust-010 owner, and excludes those
paths before driver invocation. Newly discovered cases run automatically.
The selection is 911 cases (155 parse, 730 read, 26 catalog), with 21 pending;
counts follow inventory changes. No known-failure driver ID is used.

```sh
cargo test --locked -p iris-cli --test read_conformance -- --nocapture
```

Read and catalog enforce [Specification §10's work budgets](../../spec/0.1/10-work-budgets.md):
2^20 idle units and 2^20 + 16*N expression units for the entire original input's
byte length N. Both allowances span all traversed instances and deferred
rejections. Equality is accepted; an excess charge immediately returns
`E-BUDGET`, including in skipped instances and catalog. Parse performs no
budgeted execution.

The ignored differential test runs 256 bounded scalar probes with fixed seed
73003, 384 array/read probes with seed 73004, 384 repeat probes with seed
73005, 384 line/sections probes with seed 73006, and 512 checks probes with seed
73007, plus 512 params/instances/catalog probes with seed 73008 (2,432 total).
Both drivers are decoded through the independent canonical validator. Scalar probes cover input spellings,
arithmetic boundaries, frozen types, settings, control flow, functions,
comparisons, and error order. Array/read probes cover ranks 1–4, zeros,
subtree copies, offsets and promotion, all nine layouts, byte alphabets,
sentinel/eol lists, aggregates, indexing, and comprehensions. Per-family counts
are printed. Repeat probes cover count admission and fresh frames, termination
and scanning, stores and aliases, ragged lists, missing rows and type/depth
consistency, nested columns, index statements, and progress diagnostics.
Line/sections probes cover capture boundaries and references, selected
commitment and ranges, line search, skips and token expectations, arm order,
termination and stores, and nested execution. CLI diagnostics also check capture
spans, repeated-arm precedence, missing-arm locations and interleaved section
and repeat contexts. Both CLI entry points exercise sections at the parser's
nesting boundary and serialize deeply dimensional arrays in arm bodies. Every
generated reader spec is validated before either driver runs. The test preserves
a temporary reproducer and prints commands on the first mismatch. Build the pinned adapter from the repository root, then
run from `rust/`:

```sh
adapter=$(../conformance/adapters/gecode/build.sh)
GECODE_ADAPTER="$adapter" cargo test --locked -p iris-cli --test scalar_differential -- --ignored --nocapture
```

Checks execute immediately outside instances. False checks report a saved
record start when a record witness exists, the current input cursor otherwise,
and a path of lifted indices followed by retained `all` indices. Hard errors
inside checks retain a null path. The CLI location table asserts all 33 pinned
probes beyond conformance's primary comparison, including source lines, raw
spans, contexts, empty bodies, Unicode byte positions, and explicit selections.
Deep checks exercise both user and driver routes at parser admission limits
and on 16,384-dimensional values without recursive JSON decoding.

The seed-73008 group has eight families of 64 probes: parameter conversions and
reexecution, ordinal/name selection, deferred checks, catalog checks, catalog
sections, local frames and records, and usable names. Bounds are 1–8 instances,
1–4 array leaves, sampled payloads -3–5, and 1–8 parameters. Another 34 pinned
location probes assert code, input coordinates, reader-spec line, context, and
record path for parameters, selection, skipped bodies, and deferred rejections.
