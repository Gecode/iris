# gecode-iris-cli

The `iris` command, part of milestone M3 in
[PLAN.md §8.1](../../PLAN.md#81-rust-rust) and the [§4.2
layers](../../PLAN.md#42-the-rust-reference-implementation-is-layered). The
user command checks and formats reader specs, reads inputs through
[gecode-iris-core](../iris-core/README.md), and starts the language server. The
[conformance driver](../iris-driver/README.md), a separate crate, implements
parse-and-resolve, reading, catalog, and dzn output. Parse-only requests never
execute expressions or parameter defaults.

From `rust/`:

```sh
cargo run --locked -p gecode-iris-cli -- --version
cargo run --locked -p gecode-iris-cli -- lsp
cargo run --locked -p gecode-iris-cli -- check path/to/spec.iris
cargo run --locked -p gecode-iris-cli -- read path/to/spec.iris path/to/input.txt
cargo run --locked -p gecode-iris-cli -- read path/to/spec.iris path/to/input.txt --summary
cargo run --locked -p gecode-iris-cli -- read path/to/spec.iris path/to/input.txt --json --param instance=2
cargo run --locked -p gecode-iris-cli -- read path/to/spec.iris path/to/input.txt --dzn
cargo run --locked -p gecode-iris-cli -- list path/to/spec.iris path/to/input.txt
cargo run --locked -p gecode-iris-cli -- list path/to/spec.iris path/to/input.txt --dzn
cargo run --locked -p gecode-iris-cli -- fmt path/to/spec.iris
cargo run --locked -p gecode-iris-cli -- fmt --check --diff path/to/directory
cargo run --locked -p gecode-iris-cli -- gen builder --lang rust path/to/spec.iris
cargo run --locked -p gecode-iris-cli -- gen --lang rust path/to/spec.iris
cargo run --locked -p iris-driver -- parse path/to/spec.iris --version 0.1
cargo run --locked -p gecode-iris-cli -- conformance target/debug/iris-driver ../conformance/cases/0.1/smoke
cargo test --locked -p gecode-iris-cli
cargo test --locked -p gecode-iris-cli --test complete
cargo test --locked -p gecode-iris-cli --test gen
cargo test --locked -p iris-driver --test parse_conformance -- --nocapture
```

`iris check SPEC` prints `SPEC: ok`, optionally with the format name, and exits
0. A language error exits 1 and prints a Gecode-compatible location/message
header followed by a source excerpt and caret. The added excerpt is a CLI
presentation choice: Gecode's parse diagnostics have only the header. Reported
virtual capture lines remain in the header; the excerpt uses the raw source.
The renderer escapes control characters in diagnostic text and excerpts.
Usage errors exit 2; file acquisition and resource failures exit 1.
`iris --version` reports the Cargo package version and supported language versions
(currently `iris 0.1.0 (languages: 0.1)`).

The command runs in the library: `gecode_iris_cli::run(args)` takes the process
arguments, program name first, and returns the exit status as a `u8`. It
writes to the process's standard output and error, flushes standard output
before it returns, and never exits the process. Help and usage messages name
the program from the first argument, as clap does. The `iris` binary only
passes `std::env::args_os()` to it, and the Python package's `iris` console
script calls it through the extension module (see the [Python
README](../../python/README.md#the-iris-command)).

## Reading input

`iris read SPEC FILE... [--summary|--json|--dzn] [--param NAME=VALUE]...`
defaults to `--summary`. Each file has a header with its path and optional
format name. Entries follow in definition order: name, value type, scalar
value or array shape. As in `gecode-instance -summary`, arrays show the outer
size, then the minimum and maximum second-level row lengths (0 for an empty
outer array), then `x ...` for every deeper dimension. Scalar strings use
escaped quotes and control characters. Rendering is iterative, takes linear
work in stored data and output bytes, and copies no subtrees.

Output options are mutually exclusive. `--json` uses
canonical JSON; `--dzn` uses the exact bytes in [Specification appendix
B](../../spec/0.1/appendix-b-dzn.md), including its float spelling, type
comments, and final LF rules. Multiple files concatenate JSON or dzn documents
without headers or extra separators; summaries retain each file's header.
Output is buffered until all files have been read successfully. Split assignments at the first `=`; the remainder is the exact
string value. Repeated keys replace their earlier values without moving their
supplied positions. Parameters apply to every file. Runtime language errors
exit 1 with an input location, bounded excerpt, byte-position caret, and active
reader-spec line. No partial read data is printed. File/serialization failures
exit 1; usage errors exit 2.

`iris list SPEC FILE... [--param NAME=VALUE]` matches Gecode's `-list`: print
each file's instance count, then numbered names when all are usable, with first
rejection summaries in `(failed: MESSAGE)`. Without usable names, only failed
instances get individual lines. Catalog traverses every instance even when the
caller supplies a selector. Add `--dzn` to render that catalog data with the
same writer as `iris-driver catalog --format dzn`, without listing headers. The
plain listing keeps Gecode's command-line behavior; catalog dzn is an
additional CLI representation (Gecode's `-list` and `-dzn` select separate
modes).

## Conformance

`iris conformance DRIVER [ROOTS...] [--driver-id ID] [--filter TEXT]
[--version 0.1] [--timeout SECONDS]` reuses the `gecode-iris-conformance` library.
It prints the same case lines and final report as `iris-conformance run`.
The driver is a command string split using shell quoting, with no shell
execution or expansion; quote paths with spaces inside the string. Roots are
relative to the invocation directory. With none, the default is
`conformance/cases/0.1`, intended for invocation from the repository root;
from `rust/` or an installed binary, supply explicit roots.

The language version defaults to `0.1` (the only supported version), and the
positive timeout defaults to 30 seconds per case. `--filter` selects a path
substring. `--driver-id` selects that driver's precise known-failure annotations;
without it, no exemptions apply. Exemptions cannot hide arbitrary differences;
stale annotations produce XPASS and fail the run. These options mirror
`iris-conformance run`, with its `--driver` supplied positionally.
Exit 0 means no FAIL or XPASS, exit 1 means at least one, and exit 2 means a
usage, configuration, loading, or zero-selection error. See the
[runner contract](../../conformance/README.md#running-cases) for exemptions and
driver protocol details.

## Language server

`iris lsp` serves framed LSP over stdin/stdout through
[gecode-iris-lsp](../iris-lsp/README.md), with parse and resolution diagnostics.
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

## Generating builders

`iris gen builder --lang LANGUAGE SPEC` prints a program that builds the reader
spec with the spec builder of that language, through
[gecode-iris-gen](../iris-gen/README.md). The languages are `rust`, `python`,
`cpp` and `java`. The program is a complete file: its `builder` function returns the
`SpecBuilder`, and running it prints the spec's source.

A spec that `iris check` rejects exits 1 with the same message on standard error
and nothing on standard output. A file that cannot be read also exits 1, and an
unknown language or a missing `--lang` is a usage error with exit 2.
`tests/fixtures/jobshop-builder.rs`, `jobshop-builder.py`,
`jobshop-builder.cpp` and `jobshop-builder.java` record the output for the jobshop format; after an
intended change to the generated code, write the new output there.

The round trip checks what the programs do, from `rust/`:

```sh
cargo test --locked -p gecode-iris --test builder_round_trip -- --nocapture
```

It generates a builder for every `.iris` file in `conformance/cases/0.1` and
`formats`, then formats, lints, compiles and runs them, and compares each printed
spec with its original. Its last line gives the counts. `compared` is the files
that `iris check` accepts and `skipped` is the files that it rejects, so the two
add up to the whole corpus; nothing else is skipped. `extra` is the specs beyond
the corpus, which go through the same steps: the layout forms at several widths
and the operands of `clippy::eq_op`. The limit specs of Specification §5.12 run
too, outside all of these counts. The Python and C++ round trips report the
same counts; see [gecode-iris-gen](../iris-gen/README.md) for their commands.

## Generating views

`iris gen --lang LANGUAGE SPEC` prints a module of declared handles for a text
spec, through [gecode-iris-gen](../iris-gen/README.md). The handles have the
types a spec builder returns, so `data.get` gives the same host types. Rust is
the only language so far:

```rust
// Typed handles of a reader spec, generated by `iris gen --lang rust`.

use gecode_iris::build::{D0, D2, Field, Int, Maybe, Str};

#[allow(dead_code, non_upper_case_globals)]
pub const name: Field<Str, D0, Maybe> = Field::declared("name");
#[allow(dead_code, non_upper_case_globals)]
pub const n_jobs: Field<Int> = Field::declared("n_jobs");
#[allow(dead_code, non_upper_case_globals)]
pub const machine: Field<Int, D2> = Field::declared("machine");
```

Save the output to a file and bring it into a crate in one of three ways:

```rust
mod jobshop; // the output is src/jobshop.rs

mod jobshop {
    include!("jobshop.rs"); // relative to this file; suits a build script's output
}

include!("jobshop.rs"); // the handles and the `use` join this module
```

The first two give a module of its own, so prefer them. With the third, the
including module must not define or import the same names: an entry `main` clashes
with `fn main`, and a second `use` of `Field` is an error.

Use the handles by path, as in `data.get(jobshop::n_jobs)`. The constants are
lowercase, like their entries, so any import of one has a cost: after
`use jobshop::*`, `use jobshop::n_jobs` or the third form above, a `let n_jobs = 3;`
in that scope declares no variable. It becomes a pattern that matches the
constant, and normally fails to compile with a type mismatch. A `match` arm of
that name changes the same way. Each handle holds the entry's real name as a
string; its identifier follows the rules of the builder program, so `fn` becomes
`fn_2`.

An entry that may be absent gets `Maybe`, and an entry without one value type and
depth gets a `DynField`, which reads `Option<ValueRef>`. Exit statuses are those
of `iris gen builder`. `tests/fixtures/jobshop-view.rs` records the output for the
jobshop format; after an intended change to the generated code, write the new
output there. `gecode-iris`'s `build_jobshop` test includes that file and checks
that its handles have the builder's types.

From `rust/`, these compile views of the format library and read through them:

```sh
cargo test --locked -p gecode-iris --test view_formats --test view_soundness -- --nocapture
```

`view_formats` generates one program with a module per format, plus specs that
need dynamic handles or unusual identifiers. It checks the program with `rustfmt`
and `clippy`, then compares every handle with untyped access on each format case.
A second program brings one view into a crate in all three ways, with warnings as
errors.
`view_soundness` checks, without compiling, that what a handle promises holds for
the conformance cases, the format cases and generated specs.

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

`iris-driver dzn SPEC INPUT [--params JSON] [--version 0.1]` performs the same
read and emits dzn on success. `iris-driver catalog SPEC INPUT [--params JSON]
[--version 0.1] [--format json|dzn]` renders catalog data, defaulting to JSON.
Both read and catalog defer eligible instance rejections while keeping hard
errors immediate. Language failures remain canonical error JSON (exit 1) for
both output formats. All driver documents are buffered before stdout; writer
and stdout I/O failures use the existing host-failure status 3.

The integration test discovers and loads the entire `conformance/cases/0.1`
tree through the runner library, selects exactly `Operation::Parse`, and gives
those case directories to the existing runner. Non-parse cases are excluded
before invocation, never silently passed or exempted. Counts follow the current
tree, including generated Specification examples and pinned supplements.

Messages match the pinned reference for ASCII failures. A valid non-ASCII
character rejected by the lexer is rendered as its Unicode scalar; Gecode's
single-byte message can be invalid Unicode and cannot always be reproduced
byte for byte in canonical JSON. Codes and locations follow the Specification.

The `read_conformance` test requires every discovered case in
`conformance/cases/0.1` and `formats` to pass through `iris-driver`. It prints
counts by operation and area; new cases run automatically. The current total
is 932 cases (155 parse, 730 read, 28 catalog, 19 dzn). No driver ID is needed:
Rust must match the primary expectations without exemptions. The
[conformance workflow](../../.github/workflows/conformance.yml) also runs both
roots through `iris-conformance run` against the built driver on every push
and pull request, on Ubuntu and macOS, alongside the Gecode adapter runs.

```sh
cargo test --locked -p iris-driver --test read_conformance -- --nocapture
```

Read and catalog enforce [Specification §10's work budgets](../../spec/0.1/10-work-budgets.md):
2^20 idle units and 2^20 + 16*N expression units for the entire original input's
byte length N. Both allowances span all traversed instances and deferred
rejections. Equality is accepted; an excess charge immediately returns
`E-BUDGET`, including in skipped instances and catalog. Parse performs no
budgeted execution.

The ignored differential test runs 256 bounded scalar probes with fixed seed
73003, 384 array/read probes with seed 73004, 384 repeat probes with seed
73005, 384 line/sections probes with seed 73006, and 512 checks probes with
seed 73007, plus 512 params/instances/catalog probes with seed 73008 and 640
dzn probes with seed 73010 (3,072 total). JSON results use the independent
canonical validator; dzn bytes compare exactly. Scalar probes cover input
spellings, arithmetic boundaries, frozen types, settings, control flow,
functions, comparisons, and error order. Array/read probes cover ranks 1–4,
zeros, subtree copies, offsets and promotion, all nine layouts, byte alphabets,
sentinel/eol lists, aggregates, indexing, and comprehensions. Per-family counts
are printed. Repeat probes cover count admission and fresh frames, termination
and scanning, stores and aliases, ragged lists, missing rows and type/depth
consistency, nested columns, index statements, and progress diagnostics.
Line/sections probes cover capture boundaries and references, selected
commitment and ranges, line search, skips and token expectations, arm order,
termination and stores, and nested execution. CLI diagnostics also check
capture spans, repeated-arm precedence, missing-arm locations and interleaved
section and repeat contexts. Both CLI entry points exercise sections at the
parser's nesting boundary and serialize deeply dimensional arrays in arm
bodies. Every generated reader spec is validated before either driver runs. The
test preserves a temporary reproducer and prints commands on the first
mismatch. Build the pinned adapter from the repository root, then run from
`rust/`:

```sh
adapter=$(../conformance/adapters/gecode/build.sh)
GECODE_ADAPTER="$adapter" cargo test --locked -p iris-driver --test scalar_differential -- --ignored --nocapture
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

The seed-73010 group covers finite binary64 bit patterns and spelling
boundaries, including signed zero, extremes and subnormals; all ASCII string
bytes and multibyte UTF-8; the frozen name quoting set; empty, rectangular,
ragged and cross-parent shapes at depths 1–10; records; catalog summaries; and
read failures. A separate differential test checks all 50 signed values in
appendix C.4: Iris emits their exact fixed spelling, and the pinned reference
omits leaf text. The CLI test compares all 21 dzn cases against the driver,
including catalog, parameters, repeated input files, empty documents, and error
output boundaries.
