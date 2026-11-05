# conformance

This directory holds the language-neutral conformance contract for Iris.
[PLAN.md §7](../PLAN.md#7-conformance-suite) describes the suite; milestone
M2 builds the runner, Gecode adapter and cases. This README is authoritative
for the case format and driver protocol. The standalone runner is available in
[rust/iris-conformance](../rust/iris-conformance/), with a
[Gecode adapter](adapters/gecode/README.md) and 15 [smoke cases](cases/0.1/smoke/).

[COVERAGE.md](COVERAGE.md) maps appendix D's individual decided rules to
existing cases and hand-written supplements in `cases/0.1/pinned/`.

## Case files and discovery

```text
conformance/cases/0.1/<area>/<case>/
  spec.iris                 required, valid UTF-8 source, OR
  spec.bin                  same role; arbitrary bytes, never both
  input.txt                 required for read/catalog/dzn, OR
  input.bin                 same role; arbitrary bytes, never both
  params.json               optional string-to-string JSON object
  expected.json             successful read/catalog JSON or parse success, OR
  expected.dzn              successful dzn bytes, OR
  error.json                canonical language error
  case.toml                 optional metadata and operation
  known/<driver-id>/...      optional alternate expectations referenced below
```

Exactly one primary expectation file exists. An empty input file and a missing input
file are different. Preserve every source/input/dzn byte, including NUL, CR LF, invalid
UTF-8, and the final newline. A case has exactly one of `spec.iris` and `spec.bin`.
The runner rejects invalid UTF-8 in `spec.iris`; use `spec.bin` for those negative
cases. It passes the selected spec path unchanged to the driver, which must not depend
on its extension. The runner does not parse Iris source or validate input UTF-8.
Either spec file may be a regular file or a file symlink, allowing formats to reuse
their reader spec. Do not follow directory symlinks during discovery.

`case.toml` omission means a read case with no descriptive metadata. Parse and catalog
cases must state their operation. A parse case may contain one input file, but the
runner must not pass or read that input for the parse request. Its params must be absent
or `{}`. Read, catalog, and dzn require exactly one input file even when the contents
are empty.

Recursively discover directories containing `spec.iris`, `spec.bin`, or `case.toml`,
and also accept a case directory itself as a root. Once a case directory is found, validate it and do
not descend into its support files. A metadata-only directory with no spec is an invalid
case. Deduplicate overlapping roots by canonical case-directory path, sort paths for
deterministic execution, and reject nonexistent roots or zero selected cases. Do not
infer an operation from directory names or an error code. This also supports
`formats/<format>/cases/<name>/` without special format logic.

For `--filter`, PATTERN is a case-sensitive literal substring of the displayed
case-directory path, normalized to `/`; it is not a regex or glob. Display paths
relative to the invocation directory when possible, otherwise absolute. Matching
therefore does not depend on shell expansion.

## case.toml keys

The loader rejects unknown metadata fields. Omitted metadata has no execution effect.
The complete key table and loader validation define the metadata contract.

| Key | Type and default | Rules |
|---|---|---|
| `operation` | `"parse"`, `"read"`, `"catalog"`, `"dzn"`; default `"read"` | Selects the language operation. |
| `output` | `"json"` or `"dzn"`; default JSON except for operation dzn | Only catalog may override its default to dzn. Parse/read require JSON; dzn requires dzn. |
| `title` | optional nonempty string | Human description. |
| `section` | optional nonempty string | Repository-relative Specification link including an anchor when available. |
| `source` | optional nonempty string | E.g. `gecode:test/instance.cpp:Read::Scalars`, or `spec:spec/0.1/03-data-model.md#data-definition-order`. |
| `tags` | array of strings; default `[]` | Descriptive only; no skipping or status effects. |
| `known_failure` | array of tables; default `[]` | At most one entry per driver ID; precise alternate outcome or narrow serialization exception, below. |

Read/catalog JSON success uses `expected.json` with the existing canonical data
document. Parse JSON success uses the protocol document below in `expected.json`. dzn
success, including catalog with `output = "dzn"`, uses `expected.dzn`. Every
operation/output combination can instead use `error.json`. Reject incompatible
combinations during loading. In particular, a read case with expected.dzn is invalid,
and a catalog dzn case must say `output = "dzn"` even when its expectation is an error.

Example metadata:

```toml
operation = "catalog"
output = "dzn"
title = "Catalog rejection summaries"
section = "spec/0.1/appendix-b-dzn.md#b7-catalog-data-and-executable-examples"
source = "spec:spec/0.1/appendix-b-dzn.md#dzn-catalog-summaries"
tags = ["instances", "dzn"]
```

The selected suite version and runner options supply execution policy; case metadata has
no version, timeout, command, skip flag or assertion language. The grammar check
classifies primary error codes as described below; it needs no additional metadata.

## Parameters

`params.json` is one UTF-8 JSON object mapping strings to strings. Reject duplicate
keys, non-string values, invalid Unicode, a BOM, and extra documents. Preserve object
insertion order because unknown parameters are checked in supplied order. Omission means
the empty map.

The runner passes the original validated JSON text as one `--params` argument. The
driver decodes each pair in source order and calls `Params::set(name, value)` once per
pair. It must not use the assignment-string parser, parse or reformat numbers, trim
strings, split commas, or apply shell expansion. For example
`{"n":"0007","label":"a,b","z":"-0.0"}` passes exactly those three strings. JSON escapes
are decoded once into the string values. Omitted params need no option; `{}` and
omission have the same meaning.

## Four driver commands

```text
DRIVER parse SPEC             [--params JSON] [--version V]
DRIVER read SPEC INPUT        [--params JSON] [--version V]
DRIVER catalog SPEC INPUT     [--params JSON] [--version V] [--format json|dzn]
DRIVER dzn SPEC INPUT         [--params JSON] [--version V]
```

`SPEC` and `INPUT` are file paths, with no required extension. `JSON` is inline JSON
text, not a pathname.
`--version` defaults to `0.1`; the runner always supplies its selected version
explicitly. The first runner supports only `0.1`, rejecting other selections clearly.
Options may follow the positional arguments as shown. Unknown/repeated options, missing
arguments, a nonempty parse parameter map, and unsupported versions are driver misuse.
Parse must parse and resolve only, without reading input, executing defaults or
statements, or inventing empty read data.

`catalog --format dzn` is the minimal extra protocol option required by notation §2.3
and appendix B.7. It catalogs first and renders those returned data; it does not
select/read one instance. Default catalog output is canonical JSON. The `dzn` command
continues to mean read-then-render. No fifth command is needed.

| Exit | stdout | Meaning |
|---|---|---|
| `0` | Exactly one operation-success JSON document, or the complete dzn byte sequence | Successful parse/read/catalog/dzn. JSON may have trailing JSON whitespace, with no other text. Empty dzn is valid. |
| `1` | Exactly one canonical error document | A language error, including a parse error during any operation. |
| `2` | Empty; diagnostic on stderr | Driver misuse. Never a language outcome or known failure. |
| `3` | Empty; diagnostic on stderr | Host/adapter failure, including inability to serialize non-Unicode reference data. Never a language outcome; only the exact `unrepresentable` exception below may be exempted. |

Other exit statuses, signal termination, timeout, stdout/stderr read failure, and
violations of these output rules are hard driver failures. stderr is diagnostic only and
is included in failure reports; its wording is compared only for the exact `unrepresentable` exception below. On errors there must be no
partial successful output. `E-UNMAPPED` is always a hard adapter failure even if
returned with exit 1 and structurally valid JSON. It must never occur in a primary or
alternate expectation.

## Parse success

The protocol-only parse-success object is:

```json
{"iris":"0.1","parsed":true}
```

It goes in `expected.json` with `operation = "parse"`. Require exactly these two
members, the selected language version, and the boolean `true`; ignore object order and
insignificant JSON whitespace. This is neither a data document nor a modification to the
canonical data/error schemas. Parse failure remains ordinary canonical error JSON in
`error.json`.

## Comparison

Consume [Specification chapter 3](../spec/0.1/03-data-model.md) and the existing
schemas; do not publish a second canonical encoding. Validate both fixture expectations
and driver documents at the boundary, retaining raw JSON number tokens until
type-directed validation has finished.

* Data: compare the version and ordered entries, with exact names, value types, depths,
  array shapes, strings and integers. Ignore only JSON object order and insignificant
  whitespace. Preserve int64 values without converting them through f64. Reject
  duplicate members, duplicate entry names, missing/extra members, wrong leaf
  types/depths, invalid Unicode, non-finite or overflowing floats, and invalid canonical
  numeric spellings. Empty/ragged arrays retain their declared types and depths. Use the
  [data schema](../spec/schema/data.schema.json) and
  [error schema](../spec/schema/error.schema.json) structural rules and chapter 3's
  additional semantic rules, not JSON Value equality alone.
* Floats: validate each raw number against chapter 3.5's canonical spelling and then
  compare that spelling exactly. No tolerance, ULP allowance, zero-sign normalization,
  or generic JSON number equality. For example canonical float 3 is `3`, canonical 1000
  is `1e3`, and negative zero is `-0.0`; `3.0`, `1000`, and `1E+03` are not
  substitutes in those positions. The runner uses Ryū's shortest decimal digits with the
  Specification's fixed/scientific choice and zero rules. dzn formatting has separate
  rules.
* dzn: compare the byte vectors exactly. Do not decode/parse/reformat them, normalize
  line endings, trim spaces, or supply a missing final LF. This includes catalog strings
  and zero-byte results. Appendix B defines rendering; the runner only compares its
  result.
* Errors: validate the canonical shape, then compare precisely
  `(code, primary_line, primary_column, record_path)`. Pick input_location if non-null,
  otherwise spec_location if non-null, otherwise `(null,null)`. Do not fill a null
  input column from the reader-spec location. Preserve null versus `[]` for the path.
  Ignore message, origin, context and secondary coordinates. Do not compare the complete
  error object or reinterpret witness paths from message/context text.

Validate code membership against the [existing registry](../spec/0.1/errors.md),
without inventing local codes. Do not reimplement error message mapping, error ordering,
or coordinate computation in the runner: these belong to drivers. The decoder follows
the canonical schemas' field requirements, including required nullable fields; nullable
fields must still be present.

## Known failures

Select a stable driver ID with optional runner argument `--driver-id ID`, independent
of executable name, installation path, and wrapper arguments. No new driver query
command is needed. Without this option, no known-failure entries apply; normal failures
remain failures. The reference ID is `gecode-6.5.0-599f977b7855`, binding exemptions to
the pinned reference. A later changed pin gets a new ID and a review of its remaining
discrepancies.

Each entry has these keys, with no others:

| Key | Meaning |
|---|---|
| `driver` | Nonempty stable driver ID; unique in this case. |
| `outcome` | `"data"`, `"parse"`, `"dzn"`, or `"error"` determines the alternate result kind and its required status 0 or 1. `"unrepresentable"` selects the exact Gecode serialization exception below. |
| `expected_file` | Case-relative path to the complete alternate result, conventionally under `known/<driver-id>/`; no absolute path or `..`. Required for alternate outcomes; forbidden for `"unrepresentable"`. |
| `reason` | Nonempty explanation of the exact observed discrepancy. |
| `difference` | Required when `driver` starts with `gecode-`: `spec/0.1/appendix-c-differences.md#c<N>-...` with a decimal subsection number and nonempty slug. Optional for other drivers. `"unrepresentable"` requires a `gecode-` driver and the exact C.1 link `spec/0.1/appendix-c-differences.md#c1-unicode-strings`. |

Example contract, not a real case to add in this task:

```toml
[[known_failure]]
driver = "gecode-6.5.0-599f977b7855"
outcome = "dzn"
expected_file = "known/gecode-6.5.0-599f977b7855/output.dzn"
reason = "The pinned printer omits float leaves in the range documented in C.4."
difference = "spec/0.1/appendix-c-differences.md#c4-dzn-floats-just-below-1e17"
```

The alternate file describes the complete expected observable outcome, not a wildcard,
error-code-only exception, or permission to ignore selected fields. Validate it with the
same rules as an ordinary outcome of its kind. Its success kind must be compatible with
the requested operation/output; error is permitted for every operation. It must differ
from the primary comparison outcome; reject a no-op exemption.

The `unrepresentable` exception has no alternate file. It matches only exit 3,
empty stdout, and stderr equal byte for byte to this complete line, including its final LF:

```text
iris-gecode-driver: cannot serialize non-UTF-8 reference string
```

For example:

```toml
[[known_failure]]
driver = "gecode-6.5.0-599f977b7855"
outcome = "unrepresentable"
reason = "Gecode reads a non-UTF-8 word after byte-wise token splitting; the adapter cannot serialize it."
difference = "spec/0.1/appendix-c-differences.md#c1-unicode-strings"
```

This annotation records the C.1 raw-byte string difference at the adapter's
serialization boundary. It does not accept any language outcome or other host failure.

Classification order:

1. A selected `unrepresentable` annotation with exactly the exit status, stdout,
   and stderr above: UNREPRESENTABLE, reported separately from XFAIL.
2. Any other driver/protocol failure, malformed/noncanonical JSON, crash, timeout,
   host/adapter error, or E-UNMAPPED: FAIL immediately, regardless of annotations.
   dzn remains an opaque byte result: the runner does not add a MiniZinc parser that
   would preempt a documented printer discrepancy such as C.4.
3. Actual outcome matches the primary: PASS if no matching annotation, otherwise XPASS,
   including for an `unrepresentable` annotation.
4. Actual outcome differs from the primary and matches the selected complete alternate:
   XFAIL. An `unrepresentable` annotation cannot match a representable alternate.
5. Anything else: FAIL, showing the primary diff and, if applicable, why the alternate
   did not match.

A known error-code difference cannot mask another line/column/path difference. A known
data difference cannot mask another value/entry/shape difference. Error wording remains
ignored by the normative projection, not by an exemption. Report XPASS separately and
make it nonzero so stale annotations must be removed. XFAIL and UNREPRESENTABLE do not
make the run fail.

An annotation does not create a language difference. The loader checks the shape of
Gecode difference links; authors must check that the linked accepted difference actually
permits the alternate outcome. New disagreements with pinned Gecode need an accepted
appendix C entry before exemption; stop and report absent authority. The only adapter
serialization exemption is the exact C.1 `unrepresentable`
exception above. Malformed JSON, every other host failure, and undefined crash behavior
remain hard failures. Appendix C.3 specifies no deterministic result for reference
undefined arithmetic, so it is not a wildcard exemption either.

## Specification examples and extraction

Gecode test porting follows [PORTING.md](PORTING.md). Extract the Specification's
executable examples from the repository root with:

```sh
(cd rust && cargo run --locked -p iris-conformance -- extract-examples \
  ../spec/0.1 ../conformance/cases/0.1/spec-examples)
```

Add `--check` to compare the complete generated tree without writing. It exits nonzero
for missing, changed, or stale files and stale directories. Run it after changing an
example or the extractor:

```sh
(cd rust && cargo run --locked -p iris-conformance -- extract-examples \
  ../spec/0.1 ../conformance/cases/0.1/spec-examples --check)
```

The extractor fully owns `cases/0.1/spec-examples/`: regeneration replaces its contents,
including deleting stale cases and support files. Keep generated cases in Git; edit
examples or extractor-owned annotations rather than generated files. The command needs
a Rust build, so `scripts/check-repo.sh` does not invoke it. The
[conformance workflow](../.github/workflows/conformance.yml) runs it on pushes and
pull requests on Ubuntu and macOS, alongside the grammar check and Rust checks.
It builds the Rust driver with the workspace and runs all 0.1 cases and formats
against both `iris-driver` and the pinned Gecode adapter.

Chapters are read in filename order. Each case is named `<chapter-stem>--<example-id>`,
for example `02-notation--notation-no-final-newline`. Notation §2.3 requires explicit,
globally unique IDs; missing or duplicate IDs are errors. Reordering examples does not
rename cases. Metadata records the chapter and ID and sets operation explicitly. A
catalog expected-dzn block additionally sets `output = "dzn"`.

The shared transport is direct: example `spec` becomes `spec.iris` when its decoded
bytes are valid UTF-8, otherwise `spec.bin`. Hex that decodes to valid UTF-8 still uses
`spec.iris`. Input becomes `input.bin`; params becomes `params.json`, preserving values
and source member order; expected data and expected parse become `expected.json`;
expected dzn becomes `expected.dzn`; expected error becomes `error.json`. Decode
literal/hex/no-final-newline exactly by notation §2.4, without dedenting, trimming,
adding LF, or interpreting source backslashes. Expected JSON uses the same canonical
validation as the runner. Malformed examples fail before the destination is changed.

Notation [§2.3](../spec/0.1/02-notation.md#23-example-headers-and-parts) admits
`--- expected parse` only for successful parse. It contains the protocol object above.
Empty data cannot stand in for parse success.

The extractor's `known_failure` function in
[examples.rs](../rust/iris-conformance/src/examples.rs) owns six deliberate annotations
and the five complete alternate expectations for `gecode-6.5.0-599f977b7855`:

- `input-invalid-utf8-split`: the exact `unrepresentable` exception above, under
  [C.1](../spec/0.1/appendix-c-differences.md#c1-unicode-strings).
- `input-invalid-utf8-comment` and `input-invalid-utf8-tail`: Gecode reads `x = 7`,
  under [C.1](../spec/0.1/appendix-c-differences.md#c1-unicode-strings).
- `reader-invalid-utf8-string` and `reader-invalid-utf8-comment`: Gecode parses
  successfully, under [C.2](../spec/0.1/appendix-c-differences.md#c2-reader-spec-source).
- `dzn-float-precision-gap`: Gecode omits three float leaves, under
  [C.4](../spec/0.1/appendix-c-differences.md#c4-dzn-floats-just-below-1e17).

These are fixed annotations reviewed against appendix C. The extractor never invokes
a driver or infers exemptions from its output. `input-invalid-utf8-split` reports
UNREPRESENTABLE only when its annotation
matches the exact serialization failure; every other failure remains FAIL.

## Checking the formal grammar

From `rust/`, check reader specs independently of any implementation or driver:

```sh
cargo run --locked -p iris-conformance -- check-grammar
cargo run --locked -p iris-conformance -- check-grammar ../conformance/cases ../formats
```

The command interprets [grammar.ebnf](../spec/0.1/grammar.ebnf), including its named
predicates, lexical rules, decoded patterns and parser limits. The grammar is embedded
at build time; `cargo run` rebuilds after it changes. With no paths, the command uses
`conformance/cases/0.1` and `formats` in the build-time checkout. Supply explicit,
invocation-relative case roots when using an installed or moved binary. Discovery,
loading and canonical deduplication use the same rules as the runner, including
`spec.bin` and file symlinks. Overlapping roots do not inflate counts.

Success expectations require grammar acceptance. A primary error expectation requires
rejection for these codes:

- `E-SPEC-UTF8`, `E-SPEC-CHARACTER`, `E-SPEC-STRING`, `E-SPEC-NUMBER`;
- `E-SPEC-SYNTAX`, `E-SPEC-PLACEMENT`, `E-SPEC-PATTERN`;
- `E-SPEC-UNKNOWN-TYPE`, `E-SPEC-NESTING`.

Other registered codes require acceptance. Parse-time semantic failures, such as an
unknown function or record-field error, therefore remain grammar-positive. The
operation, tags and driver-specific alternate expectations do not change this
classification. No additional `case.toml` field is required. When adding an error code
to the registry, check whether it belongs in the rejection set under
[appendix A.5](../spec/0.1/appendix-a-grammar.md#a5-syntax-and-other-parse-time-checks)
and [error ownership](../spec/0.1/errors.md#114-parse-time-codes).

The check recognizes source only. It does not reproduce expected error codes or
coordinates, perform static semantic checks, or execute the reader spec. It maintains
only the type visibility and alphabet-alias information required by grammar dispatch
and capture restrictions. Each disagreement prints its case path and expected class,
with recognition details for unexpected rejections; checking continues through the
remaining cases. Pattern diagnostics identify the outer string's source line and a
decoded byte offset. Successful cases have no individual output line.

The final line has this format (counts shown for the current corpus):

```text
Grammar summary: total=932 valid=873 invalid=59 accepted=873 rejected=59 fail=0
```

`valid` and `invalid` count expected classes; `accepted` and `rejected` count actual
recognition results. Each pair partitions `total`; `fail` counts disagreements. Exit
**0** means no disagreements, **1** means at least one disagreement, and **2** means a
CLI, grammar configuration, case-loading, IO or zero-selection error. Counts are per
case, including cases that share a reader spec. The checker uses no driver-specific
known-failure handling.

## Comparing reader specs

`iris-conformance compare LEFT RIGHT` compares two reader specs as syntax trees,
through the same recognizer as the grammar check. A language adding a builder
generator uses it to round-trip the corpus; see the [iris-conformance
README](../rust/iris-conformance/README.md#comparing-reader-specs).

## Running cases

From `rust/`:

```sh
cargo run --locked -p iris-conformance -- run --driver '<command>' \
  [--driver-id ID] [--filter PATTERN] [--skip PATTERN]... [--version 0.1] [--timeout SECONDS] <cases-dir>...
```

The version defaults to `0.1`, the only supported selection. The timeout is
positive and defaults to 30 seconds per case. Execution is serial. The driver
string is split once using shell quoting rules; its first word is the executable
and the remaining words are prefix arguments. No shell runs: there is no glob,
pipeline, environment assignment, variable expansion or command substitution.
Quote paths containing spaces inside the string, or supply an executable wrapper.
Request arguments are appended as separate argv elements. The runner retains its
invocation directory, closes driver stdin, and passes absolute spec/input paths.

Stdout and stderr are drained concurrently. The deadline covers child execution
and pipe completion. A timeout kills and reaps the direct child. Wrappers must
forward exit status, complete streams and clean up their descendants. The runner
does not manage arbitrary process trees.

Cases are all loaded before any driver runs. Configuration errors stop the run;
executed failures do not prevent later valid cases from running. Each case prints
`PASS`, `XFAIL`, `UNREPRESENTABLE`, `FAIL` or `XPASS` and its path. Failures include
field or byte differences and bounded diagnostic/output excerpts. The final line is exactly:

```text
Summary: total=N pass=P xfail=K fail=F xpass=U unrepresentable=R
```

Counts partition total. `--skip PATTERN`, repeatable, takes a literal substring like
`--filter`. Selected cases whose paths contain it are not run. Each one prints `SKIP`
and its path, is left out of the counts, and the summary gains ` skipped=S`. A pattern
that matches no selected case is a configuration error, so stale skips must be removed.
CI uses it only for the Gecode adapter on Linux, with the list in
[`adapters/gecode/libstdcxx-skips.txt`](adapters/gecode/libstdcxx-skips.txt): the
pinned reference reads and prints out-of-range floats wrongly when built against
libstdc++, and that platform bug is not an appendix C difference. Runner exits: **0** when fail and xpass are both zero;
**1** when either is nonzero; **2** for CLI, configuration, loading or zero-selection
errors. Help and version exit 0. A driver exit 2 is a case failure, hence runner
exit 1. XFAIL and UNREPRESENTABLE lines include the driver ID, reason and any difference
link. XPASS identifies a stale annotation and requires its removal.

Build the Rust workspace and run both roots from the repository root:

```sh
(cd rust && cargo build --workspace --locked)
(cd rust && cargo run --locked -p iris-conformance -- run --driver "\"$PWD/target/debug/iris-driver\"" ../conformance/cases/0.1)
(cd rust && cargo run --locked -p iris-conformance -- run --driver "\"$PWD/target/debug/iris-driver\"" ../formats)
```

The Rust driver needs no `--driver-id`: every case must match its primary
expectation, with no exemptions. CI uses `iris-conformance run` for both drivers
so they share the same command and report format.

See [Rust build instructions](../rust/README.md) for the canned driver demonstration.
Build the adapter and run the real suite from the repository root:

```sh
adapter=$(conformance/adapters/gecode/build.sh)
ctest --test-dir "$(dirname "$adapter")" --output-on-failure
(cd rust && cargo run --locked -p iris-conformance -- run --driver "\"$adapter\"" \
  --driver-id gecode-6.5.0-599f977b7855 ../conformance/cases/0.1)
```

The build prints the absolute executable path, so the driver works from the runner's
current directory. For a direct invocation, use
`"$adapter" read /absolute/spec.iris /absolute/input.bin`. The adapter preserves
the reference's UTF-8 behavior rather than implementing the Iris-only validation
differences in appendix C.1–C.2.
Non-Unicode reference string results fail at the serialization boundary with
exit 3 and empty stdout, for both JSON and dzn; they are not language errors. Only a
selected `unrepresentable` annotation with the exact C.1 link and exact diagnostic line
above exempts that serialization failure. Other exit 3 results remain hard failures.
