# iris-syntax

`iris-syntax` validates reader-spec bytes, parses Iris 0.1 into a lossless
rowan concrete syntax tree, and formats reader specs. Broken syntax yields a tree and
diagnostics so reading tools, the language server, and the formatter can share
one parser. This is part of milestone M3; see
[PLAN.md §§4.2](../../PLAN.md#42-the-rust-reference-implementation-is-layered),
[8.1](../../PLAN.md#81-rust-rust), and
[8.5](../../PLAN.md#85-tree-sitter-grammar-tree-sitter-iris).
The language contract is the [Specification](../../spec/0.1/README.md).

```rust
use iris_syntax::{LanguageVersion, parse};

let source = b"line \"n={n:0..}\"";
let parsed = parse(source, "numbers.iris", LanguageVersion::V0_1)?;
assert_eq!(parsed.syntax().text().to_string().as_bytes(), source);
assert!(parsed.diagnostics().is_empty());
# Ok::<(), iris_syntax::ParseError>(())
```

The version is an explicit parameter; only `LanguageVersion::V0_1` exists.
`Parse` owns the green tree, diagnostics, version, and source origin.
`syntax()` returns a rowan root. Public syntax kinds distinguish statements,
types, expressions, and embedded pattern items. Pattern leaves retain the raw
source spelling, including quotes and escapes. Capture types use the same lexer,
type parser, type-name visibility, and nesting counters as ordinary source.

`parse` returns `ParseError::InvalidUtf8` before lexing when any source byte
sequence is malformed. The diagnostic has the first ill-formed subsequence's
1-based byte column, counting LF lines. For valid UTF-8, the tree text equals
the complete source, even when parsing reports errors. No bytes are normalized,
replaced, or stripped. Before UTF-8 validation, sources longer than `u32::MAX`
bytes return `ParseError::SourceTooLarge { len }`. This is a host resource limit
from rowan's representation, separate from language diagnostics.

## Syntax and semantic analysis

Zero syntax diagnostics is necessary for full parse success. Name resolution,
name conflicts, reading-type compatibility, and record scopes belong to
`iris-sema`. In particular, `x:unknown`, `a[1]:rest`, and
`type t=alphabet "ab" line "{x:t}"` retain syntax for that later analysis.
The parser only tracks completed named-type spellings to select the grammar's
type alternative; it does not resolve aliases. Fixed function names, call forms
and arities, host constants, forbidden definition names, placement, and both
operational nesting counters are checked here.

Diagnostics carry a registered code, a raw half-open byte range, a reported
reader-spec location, and a phase. Ordinary reported columns are absent even
when a precise raw range is known. A decoded LF inside a capture type can
advance its reported line while its raw span still points into the original
string. Whole-source lexical diagnostics precede parsing diagnostics. Capture
sub-lexer diagnostics belong to the parsing phase, after validation of the
whole pattern structure. Conversion to canonical errors must supply null input
location, context, and record path; this crate has no runtime error envelope.

A future semantic pass must interleave statement-local checks with syntax
errors in parser operation order, using the partial nodes and `Error` nodes at
failure points. For example, `a[1]:rest bogus` has a syntax error at `bogus`
here, but full parsing must report the earlier incompatible array type.
Global name-conflict phases run after statement parsing succeeds. Returning
`diagnostics()[0]` unconditionally cannot implement full parse-and-resolve
error ordering.

## Formatting

The formatter implements [the approved style](../../docs/formatting.md) for
milestone M4e ([PLAN.md §8.12](../../PLAN.md#812-formatter-iris-fmt-in-iris-syntax)).
It changes whitespace and line breaks outside protected text. String contents,
whole line patterns (including capture whitespace), comments, token spellings,
and statement order are preserved.

```rust
use iris_syntax::{FormatOptions, LanguageVersion, format};

let output = format(
    b"repeat 2 {x:int}",
    "rows.iris",
    LanguageVersion::V0_1,
    FormatOptions::default(),
)?;
assert_eq!(output, "repeat 2 {\n  x: int\n}\n");
# Ok::<(), iris_syntax::FormatError>(())
```

`FormatOptions` contains only `line_width`, defaulting to 100. Width counts
Unicode scalar values, including indentation; indivisible groups and protected
text may exceed it. Zero is invalid. The optional `editorconfig` Cargo feature
(off by default) provides `FormatOptions::for_path`, which reads
`max_line_length` from the path's `.editorconfig` hierarchy, using width 100
when unset or `off`, and reporting invalid values or unreadable configuration
files. The CLI and language server enable this feature. Default builds and
`--no-default-features` builds omit this API and the `ec4rs` dependency, so
syntax and formatting remain available without filesystem lookup for browser
and binding consumers. The formatting functions themselves do no filesystem
access. The only supported language version is `V0_1`.

`format` refuses every source failure or syntax diagnostic and returns no
partial output. Syntax-clean input is accepted even when later name resolution
or type checking would reject it. `FormatError` distinguishes source failures,
syntax diagnostics, invalid width, and invalid byte ranges.

`format_range` accepts half-open UTF-8 byte offsets and widens them to the
smallest complete statement sequence in a common root or block. A selection
inside an arm's block can format body statements; one spanning arms formats
the entire sections statement. Selecting a header, brace, or else-declaration
includes its containing statement. Trailing comments stay with their token.
Indentation comes from the surrounding syntax, rather than the source's spaces.

The extent includes the complete whitespace run on either side of the selected
statements and comments, ending at the nearest exterior token or comment, or
at the file edge. A run partly inside the requested range is included in full;
no whitespace run is split between formatted and untouched text. Within the
extent, whitespace follows the whole-document rules: collapse blank lines,
trim them at file and brace edges, use LF and spaces, and normalize indentation.
This can change whitespace before the following unselected token; that token
and all bytes outside the widened extent stay unchanged. For example, selecting
`x` in `first:int\n\n\nx :int\nlast:int` includes both surrounding runs and
produces `first:int\n\nx: int\nlast:int`.

The returned `RangeFormat` reports the widened range and zero or one
`FormatEdit`, entirely within that range. Applying it preserves every exterior
byte. Selected syntax errors cause refusal, including missing-token errors
reported at the following statement; errors in other statements do not. For
example, a selection inside `good:int` in `let bad=+ good:int` formats the
recovered declaration while preserving the malformed definition.

A cursor selects its containing statement, the following statement at an exact
boundary, or the previous statement at EOF. A whitespace-only document yields
no edits; a comment-only document widens to the whole document. Ranges outside
the source or splitting a UTF-8 scalar are rejected. LSP callers convert byte
offsets to their negotiated position encoding.

Formatter output changes only in minor package releases. Each style change
must appear in the changelog.

## Development

From `rust/`, run the workspace build, formatting, Clippy, and test commands in
[the workspace README](../README.md). Focused syntax checks:

```sh
cargo test --locked -p iris-syntax
cargo test --locked -p iris-syntax repository_losslessness -- --nocapture
cargo test --locked -p iris-syntax fixture_truncation_sweep -- --nocapture
```

Tests cover lexical boundaries, contextual keywords, range lookahead, statement
sequencing, precedence and association, patterns with raw spans, placement,
exact nesting boundaries, and recovery that preserves later declarations.
Every valid-UTF-8 test also checks reconstruction and the partition of source
bytes among leaf tokens. The six [Gecode fixtures](tests/fixtures/gecode/README.md)
are tested without a reference checkout. A deterministic sweep parses every
UTF-8-boundary prefix of those fixtures.

`repository_losslessness` enumerates tracked and nonignored working-tree
`.iris` files with NUL-delimited Git output. It reports totals, valid sources
checked, invalid UTF-8 sources excluded, and reconstruction failures. It checks
reconstruction regardless of syntax diagnostics; it does not claim semantic
conformance or invoke an implementation driver.

`repository_formatting` walks the same tracked and nonignored reader specs. For
each syntax-clean source it checks widths 40, 80 and 100 for idempotence, the
same CST without whitespace, and byte-identical protected text. It reports
syntax-clean, syntax-invalid and invalid UTF-8 counts separately. Run it with
`cargo test --locked -p iris-syntax repository_formatting -- --nocapture` to see
the counts. Formatter tests also reproduce all 22 guide pairs at their stated
widths, compare [rule snapshots](tests/fixtures/format/README.md), and exercise
range widening and error isolation. No test rewrites reader specs in place.
