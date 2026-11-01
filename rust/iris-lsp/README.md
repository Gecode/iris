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
Opening publishes diagnostics immediately. Changes apply in order and
publish after a 150 ms trailing debounce for each document. Closing clears
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
changes. Completion, formatting, semantic tokens, and live sample reading
belong to later LSP tasks.

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
capture names and bounds, rename refusals, and representative repository reader
specs. The CLI test starts the real binary and checks framing, diagnostics,
and orderly exit.
