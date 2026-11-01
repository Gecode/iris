# iris-syntax

`iris-syntax` validates reader-spec bytes, lexes them, and parses Iris 0.1 into
a lossless rowan concrete syntax tree. Broken syntax yields a tree and
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
