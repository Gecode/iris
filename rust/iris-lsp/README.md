# iris-lsp

The Iris 0.1 language server, part of milestone M4b in
[PLAN.md §8.7](../../PLAN.md#87-language-server-rustiris-lsp).
The library serves an LSP connection; the existing `iris` binary starts it
with `iris lsp`.

## Starting the server

Build and start from `rust/`:

```sh
cargo build --locked -p iris-cli --bin iris
./target/debug/iris lsp
```

Configure an editor's language-server executable as `iris` and its arguments
as `["lsp"]`, or use the absolute path to the built binary. An installed
executable uses the same command:

```sh
iris lsp
```

The process reads framed LSP messages from stdin and writes them to stdout.
Errors go to stderr. Unframed text typed into a terminal is not an LSP client.
The editor may start it in the project directory; document identity comes from
URIs, including `untitled:` URIs, without relying on the working directory.
The editor owns executable discovery and installation.

## Diagnostics and synchronization

The server advertises incremental synchronization with open/close support.
Open buffers are authoritative: it does not read reader specs from disk.
Opening analyzes immediately. Without runnable samples it publishes diagnostics
immediately; with samples it schedules reading immediately and waits for that
version’s merged result. Changes apply in order and start analysis and reading
after a 150 ms trailing debounce for each document. Closing clears
diagnostics and removes pending work. Each publication for an open document
includes its version; stale and duplicate changes are ignored.

Positions use UTF-16 by default. The server chooses UTF-8 when the client
explicitly offers it in `general.positionEncodings`. Both edits and diagnostics
use the negotiated encoding and physical source lines, including CRLF and
lone CR. Invalid edits leave the previous buffer and version intact and log
an error; a full replacement or close/open can resynchronize the buffer.

The server reuses `iris-syntax` and `iris-sema`, preserving the checker's
[error order](../../spec/0.1/errors.md#112-error-order). It publishes at most
one parse or resolution error, with its registered code, message, raw source
range, and a `codeDescription` link. Unavailable ordinary values remain a
read-time concern. LSP document text is Unicode, so invalid file bytes cannot
produce `E-SPEC-UTF8` through document notifications.

Diagnostic links currently target the error registry's section anchors at
`https://github.com/Gecode/iris/blob/main/spec/0.1/errors.md#114-parse-time-codes`.
This is an interim source-document target, without per-code anchors. Public
reachability and editor hyperlink rendering are not established by server
tests. The documentation base is one constant in `src/diagnostic.rs` so the
site can supply its final routes later.

Analysis runs on the server thread using complete reparses; the document cache
retains each tree and semantic model together and invalidates both on accepted
changes. Sample reading runs on one separate worker thread.

## Live sample reading

A directive comment asks the server to read a sample with the whole reader spec:

```iris
# iris: sample samples/small.txt
# iris: sample "samples/with spaces.txt" instance=2 param scale=3
# iris: sample samples/named.txt instance="named item" param label="a b"
```

The syntax is `# iris: sample PATH [instance=SEL] [param NAME=VALUE]...`.
Double quotes delimit whitespace-containing components of paths, selectors,
and parameter values. Only those delimiting quotes are removed. Backslashes
are literal; there are no escapes, shell expansion, expressions, single quotes,
or trailing shell comments. An unmatched double quote is an error.
`instance=SEL` uses the core `instance` parameter: a positive ordinal or an
exact name. Parameter values remain text, and an empty parameter value is
allowed. Repeated keys, including `param instance=...`, use the last value.

Directives can appear anywhere a comment is legal, including inline and inside
blocks. Each applies to the whole spec even inside an unexecuted branch.
Text inside a string is not a directive. Several directives read independently,
in source order, with their own parameters; repeated paths are not deduplicated.
Labels include the relative path, selection when supplied, and directive line.
Unknown command or suffix words produce a warning; a sample with an unknown
suffix still runs. Malformed recognized clauses produce an error and skip that
sample. Directive diagnostics do not hide the checker's parse/resolution error.

PATH must be nonempty and relative; `..` is permitted. It is resolved against
the decoded local `file:` URI's parent directory, without canonicalization or
cwd fallback. File URI decoding follows the formatting rules below, including
spaces and supported platform paths. Non-file documents such as `untitled:`
receive a directive diagnostic explaining the required file URI. Missing,
unreadable, and non-regular sample files produce diagnostics on the directive.

Runtime language errors highlight their exact reader-spec statement span when
available, using negotiated positions. Errors without such a location, including
unknown parameters, trailing input, and input-wide invalid UTF-8, highlight the
directive. Messages identify the sample and include its input line/column,
bounded excerpt, context and record path when available. Language codes retain
their documentation links; tooling errors use `sample-directive` or `sample-file`.
Each sample diagnostic is limited to 4 KiB, with at most 80 caret columns.

One worker reads serially across documents and samples. Every accepted edit
immediately cancels obsolete work and removes cached summaries; replacement
work starts after debounce. Stale, duplicate or invalid changes leave current
work intact. Requests answer from the current cache without waiting for I/O.
A buffer version publishes one merged diagnostic list after all its runnable
samples finish. Superseded versions publish nothing; closing clears diagnostics
and prevents late results from restoring the document. Shutdown cancels reading
and responds immediately. Cancellation is cooperative: blocked OS acquisition,
parser analysis, allocation and standard-library conversion can delay it and
worker teardown. No thread is forcibly terminated.

A successful directive's hover shows its sample identity, optional format name,
and entry names, types, scalar values and shapes in definition order. Its inlay
hint appears at the end of the physical directive line, before CR/LF, with the
bounded preview as tooltip. Ordinary declaration hints remain available.
Hover follows the client's Markdown/plaintext preference; arbitrary sample text
stays literal in Markdown. Pending directives may show a reading message, and
failures use their diagnostics instead of a successful summary hint. Clients
advertising `workspace.inlayHint.refreshSupport` receive refresh requests after
hint installation and invalidation; other clients see the cache on their next
hint request.

Previews inspect at most 64 entries, 1,024 second-level rows in total, and 16
dimensions. Names, labels and scalar strings are capped at 160 Unicode scalars
before quoting, and the preview at 16 KiB UTF-8. Omission markers identify entry,
dimension and output limits. A shape that exceeds the row budget is explicitly
truncated rather than reporting a prefix's min/max as the full shape. The read
itself is complete.

Sample files are reread on open and accepted reader-spec changes. There are no
sample-file watches, save triggers or background polling: a disk edit is picked
up by the next spec edit or close/reopen.

## Hover, completion, and inlay hints

Hover documents built-in keywords, reading types, and functions with links to
the Specification sections summarized in `src/builtins.rs`. Names show the
semantic model's value type and depth at that definition or reference. Record
names and named types are described separately from data entries. Contextual
keywords used as names retain their name information.

Completion offers statement keywords, visible named types in type positions,
and available name candidates, functions, and host constants in expressions.
Names from completed repeats are offered as columns; comprehension bindings
are offered only in their bodies. Suggestions describe possible names, without
claiming that a conditional definition will execute. Layout expressions offer
the nine matrix layout strings, inserting quotes when needed. Completion edits
replace the current identifier or layout string, and filter by its typed prefix.

Hover and completion documentation use the client's first supported markup
format, defaulting to plaintext. Function snippets are used only when the client
advertises `snippetSupport`; other clients receive plain names.

Inlay hints annotate value definitions in the requested range, including reads,
captures, `let`, and `param`. Both hover and hints preserve uncertainty: unknown
facts say `unknown`, and multiple possibilities are separated by `or` and marked
`ambiguous`. A definition inside a repeat shows its local depth; references use
the semantic model's selected view, including collected column depths.

These requests use the latest open buffer and negotiated position encoding.
They reuse the bulk position table and request helpers used by structure
requests. Completion computes scope visibility once and scans definitions once;
hints scan definitions once and render only those in the requested range.
Syntax traversal and ancestor walks are iterative. Feature work takes linear
time in source and model size, in addition to cached analysis.

## Navigation and rename

The server advertises definition, references, and rename with prepare-rename.
These requests use the latest open buffer immediately, including changes still
waiting for diagnostic publication. Names, named types, record names, and names
inside line patterns use the semantic model's raw source ranges and the
negotiated position encoding.

Definition returns every possible definition of the selected reference.
References returns the uses of the selected definitions, deduplicated in source
order, and honors `includeDeclaration`. Rename follows shared references
transitively: every possible definition of every affected reference joins the
rename group. Separate definitions with the same spelling remain separate when
no reference connects them.

Rename refuses collisions with values outside the group, including columns
published by repeats. It reparses and analyzes the proposed text to enforce
contextual keyword, type, record, and comprehension rules, and verifies that
references retain their targets. Contextual keywords are accepted where the
Specification permits them. Rename requires a reader spec without parse or
resolution errors, because an invalid source can have a partial semantic model.
Navigation can still use the available model for such a source.

Requests for unknown or closed documents and invalid positions return parameter
errors. Definition and references return empty arrays on non-name positions or
unresolved names; prepare-rename returns null there, and rename refuses them.
All edits concern the selected open document; the server does not search files
on disk or modify the buffer itself.

## Outline, folding, and semantic tokens

Document symbols outline repeats, sections, arms, and instances. Labels use
their source headers; ranges include their bodies. Clients offering
`hierarchicalDocumentSymbolSupport` receive nested symbols; other clients
receive a flat list of locations. Arms are `Key` symbols for clients that
list `Key` in `symbolKind.valueSet`, and `String` symbols otherwise.

Folding ranges cover brace blocks, sections, and consecutive comment lines.
Blank lines separate comment runs. With `lineFoldingOnly`, block folds leave
the closing brace visible; otherwise ranges include character positions.

Full semantic tokens use this ordered legend (indices start at zero):

```text
variable, parameter, type, function, keyword, string, number, comment,
operator, property
```

The modifier legend is `declaration`, `readonly`, `defaultLibrary`, with bits
1, 2, and 4 respectively. Captures are variable declarations; current-value
references inside line patterns are variables. Named types and parameters
retain their semantic roles, and contextual keywords used as names receive
name tokens. Built-in types and functions carry `defaultLibrary`; host
constants also carry `readonly`. No custom token types are needed.

All three features use the latest open buffer and negotiated encoding.
Semantic tokens use relative deltas in source order, split at physical line
endings even when the client supports multiline tokens. Escapes keep their
raw source spans. Token range and delta requests are not advertised.

Bulk positions are indexed once per request. CST walks inspect each node or
leaf a constant number of times, and semantic names use a range index rather
than repeated model searches. Each request's feature work takes linear time
and memory in the document and model size, in addition to cached analysis.

## Formatting

The server advertises document and range formatting using the existing
`iris-syntax` formatter and [fixed style](../../docs/formatting.md). It ignores
client `tabSize`, `insertSpaces`, and other formatting options. For `file:`
URIs, `FormatOptions::for_path` reads `max_line_length` from the document path's
`.editorconfig` hierarchy on each request, with the same behavior as `iris fmt`.
Unset or `off` means width 100. Invalid values and unreadable configurations
return request failures. Non-file URIs, including `untitled:`, use width 100.
File URI authorities must be empty or `localhost`; other authorities return
parameter errors. Paths are percent-decoded, including `%2F` as `/`, matching
the former Unix URL conversion. NUL bytes and relative paths are refused. Unix
paths retain decoded bytes, including non-UTF-8 names. Windows accepts UTF-8
`/C:/...` drive paths and converts separators; unsupported forms are refused.
The source always comes from the latest open buffer.

Document formatting returns one whole-document replacement when text changes.
Range formatting forwards the formatter's zero or one replacement, within its
reported extent: the smallest complete statement sequence covering the request,
including attached comments and surrounding whitespace. Bytes outside that
extent remain unchanged. No protocol position lies between the CR and LF of a
CRLF pair, so an extent boundary there widens the edit by that one byte and
the replacement restores it. Already formatted input returns an empty edit
array.
A syntax error returns no document edits; range formatting still accepts a
clean statement when errors occur elsewhere, and returns no edits for a
selection it cannot format because of syntax errors. Resolution errors do not
prevent formatting.

Edits use the negotiated position encoding and the shared position table,
indexed once per request that returns edits. They preserve the formatter's
exact output, including LF normalization outside protected text when the source
uses CRLF or lone CR. Unknown or closed documents, reversed ranges, nonexistent
lines, and positions splitting Unicode scalars return parameter errors.
Characters beyond a line's length clamp to its end, as for other requests.
The server does not modify the buffer or write the reader spec to disk.

## Validation

From `rust/`:

```sh
cargo test --locked -p iris-lsp
cargo test --locked -p iris-cli --test lsp_stdio
```

The in-process client in `tests/support/mod.rs` uses the production server
entry point. Tests cover synchronization, diagnostic links and ranges,
Unicode encodings, atomic edits, document isolation, debounce decisions,
lifecycle errors, navigation, ambiguous definitions, transitive rename groups,
capture names and bounds, rename refusals, nested outlines, block and comment
folding, semantic token roles and deltas, hover documentation and shapes,
completion contexts and visibility, optional snippets, uncertain inlay hints,
document and range formatting, syntax-error isolation, EditorConfig width,
physical line endings and formatting edit application, and representative
repository reader specs. The CLI test starts the real binary and checks framing,
diagnostics, and orderly exit.
