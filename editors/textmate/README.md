# Iris TextMate grammar

[`iris.tmLanguage.json`](iris.tmLanguage.json) highlights Iris 0.1 reader
specs as `source.iris`. It is the hand-written grammar described in
[`PLAN.md` §8.6](../../PLAN.md#86-textmate-grammar-editorstextmate), delivered
in milestone M4b. There is no generation or build step. Editor extensions
and site highlighters can load the JSON file directly.

The grammar scopes comments, strings and their four escapes, numbers,
reading types, named-type definitions, keywords, names, calls, operators,
and delimiters. Keyword-shaped declaration names take priority when their
colon is visible on the same physical line, including after nested array
dimensions. Statements can share a line, and expressions can continue on
the next line. Expression operands and grouped expressions keep contextual
references such as `check sections` and `check line` out of statement scopes.
Quoted format names include their glued `-` and `.` suffixes.

Pattern strings in `line`, `find`, and sections arms have additional
`meta.capture.iris` and `meta.reference.iris` scopes. Captures such as
`{x:int}`, `{x}`, `{:int}`, and `{_}`, references such as `{$x}`, and the
terminal `...` have distinct scopes. Sections terminators, ordinary
strings, and check messages retain ordinary string scopes. Nested arm
bodies return to statement highlighting. Strings in capture types handle
both the outer source escapes and the decoded string's escapes.

This grammar provides highlighting, not syntax validation or name
resolution. For example, it does not enforce parser nesting limits or
resolve named reading types. TextMate examines one physical line at a
time: a keyword-shaped declaration name whose dimensions or colon occur
on a later line can retain a keyword scope. A reference named `int`,
`until`, `eol`, or `eof` can likewise receive a type or keyword scope
within its statement. Semantic tokens from the language server correct
these classifications. The Rust parser and the language server own
complete syntax and semantic diagnostics.

## Tests

With Node LTS and npm, run in this directory:

```sh
npm ci
npm test
```

`npm test` runs:

- Node tests for the repository sweep's lexical-negative rule and for
  scope containment: each problematic statement in
  [`tests/corpus/containment.iris`](tests/corpus/containment.iris) is followed
  by a pattern whose name and type must retain their capture scopes.
- Scope assertions in [`tests/assertions/scopes.iris`](tests/assertions/scopes.iris),
  including positive name scopes and excluded keyword scopes for
  keyword-shaped declarations, captures versus references, and ordinary
  strings versus patterns.
- `vscode-tmgrammar-test` snapshots for the twelve reader specs in
  `tests/corpus/`: six focused handwritten files and the six unchanged
  Gecode fixtures. Their [upstream notice](tests/corpus/gecode/README.md)
  accompanies the copies.
- `npm run check-repo-specs`, which uses `git ls-files -z` to tokenize every
  tracked `.iris` file, plus the local test corpus before staging. It
  strictly decodes UTF-8, carries the TextMate state between lines, and
  fails with a path, line, and column for any `invalid` scope. A conformance
  case is skipped only when its adjacent `error.json` expects
  `E-SPEC-STRING`, `E-SPEC-NUMBER`, `E-SPEC-CHARACTER`, or `E-SPEC-UTF8`:
  the source lexical errors in [§11.4](../../spec/0.1/errors.md#114-parse-time-codes)
  and source UTF-8 validation error in [§11.6](../../spec/0.1/errors.md#116-iris-only-errors)
  of the Specification. These fixtures deliberately violate source validity.
  All other tracked reader specs, including syntax, pattern, and semantic
  negatives, must have no `invalid` scopes. The check reports checked and
  skipped file counts.
  The invalid scopes currently identify illegal source characters and
  string escapes; absence of those scopes does not establish syntax validity.

The tokenization check uses the same pinned TextMate and Oniguruma versions
as the assertion and snapshot runner.

To inspect an intended highlighting change, run `npm run update-snapshots`
and review the `.iris.snap` diffs. Commit the snapshots with the grammar
change. The [CI workflow](../../.github/workflows/textmate.yml) runs
`npm ci && npm test` when this component, its workflow, any `.iris` file,
or a conformance case's `error.json` changes.
