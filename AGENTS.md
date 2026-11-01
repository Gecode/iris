# Repository guidance

This file is the guidance for everyone who works in this repository, human or
coding agent. `CLAUDE.md` imports it.

## Where things are

- [`PLAN.md`](PLAN.md) is the master plan: versions, goals, architecture,
  repository layout (§5), deliverables, milestones, and decisions. Read it
  before starting work on a component.
- `CONTEXT.md` is the glossary. Use its terms in every document.
- `docs/adr/` holds the architecture decision records.
- `docs/1.0-ideas.md` collects candidate changes for Iris 1.0. Add an entry
  there when Iris 0.1 cannot express something, or expresses it awkwardly.
- `docs/lint-candidates.md` collects evidence for lints. Add an entry there
  when a valid reader spec contained a mistake that a tool could have
  flagged.
- Each component directory has a `README.md` naming its purpose, its
  `PLAN.md` section, and its milestone.

## Writing the name

- Write **Iris** in prose and `iris` in code, commands, package names, and
  the `.iris` file extension.
- Never write the name in capitals.
- The expansion "instance reading and interpretation system" may appear at
  most once in a document, in lowercase.

## Terminology

- A **reader spec**, or just *spec*, is a program in the Iris language, such
  as `dimacs-cnf.iris`. The term matches Gecode's API (`Spec::parse`,
  `-spec FILE`).
- The **Iris Language Specification**, or *the Specification*, is the
  normative document for one language version in `spec/`. Always capitalize
  it, and never shorten it to "spec".
- **Iris 0.1** is the Gecode 6.5.0 language; its only deliberate
  differences are listed in the Specification's appendix C. **Iris 1.0** develops it
  further and is the language of Gecode 7. Language versions and package
  versions are separate.

## Implementations

Rust is the reference implementation. Python binds the Rust crate. C++
(C++23) and Java (Java 27, with a Kotlin module) are native implementations.
The conformance suite in `conformance/` is the contract between them.

## Gecode

- Iris 0.1 is defined by Gecode 6.5.0. Until the `release-6.5.0` tag exists,
  the reference is `https://github.com/Gecode/gecode` at commit
  `5e7bbd5c26b0702c6fd652356b2289da0710ac3c` (branch `feature/6.5.0`).
- Never modify the Gecode checkout at `/Users/zayenz/gecode/gecode`: no
  branch switches, worktrees, builds, or edits. To read a file from it, use
  `git -C /Users/zayenz/gecode/gecode show <ref>:<path>`.

## Commits and history

- Never add AI attribution: no `Co-Authored-By` trailers for an AI and no
  "generated with" footers, in commits, pull requests, issues, or releases.
- Keep history linear. Update a branch by rebasing it onto its base, never by
  merging.

## Building and testing

Each component's area adds its build and test commands here when it creates
the component.

| Component | Commands |
|---|---|
| Repository | `scripts/check-repo.sh` checks required files, name spelling, and relative Markdown links (see `scripts/README.md`). |
| Gecode reference | `scripts/gecode-reference.sh` builds the pinned reference; `eval "$(scripts/gecode-reference.sh --print-env)"` exports its paths (see `scripts/README.md`). |
| `rust/` | In `rust/`: `cargo build --workspace --locked`; `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings`; `cargo test --workspace --locked`. Parser only: `cargo test --locked -p iris-syntax`; add `repository_losslessness -- --nocapture` to report byte-for-byte reconstruction of every `.iris` file. Formatter only: `cargo test --locked -p iris-syntax repository_formatting -- --nocapture` reports same-tree and idempotence counts at widths 40, 80 and 100; `guide_examples -- --nocapture` checks the pairs in [docs/formatting.md](docs/formatting.md). Formatter CLI: `cargo run --locked -p iris-cli -- fmt PATH...` formats in place; add `--check` to check without writing and `--diff` to preview changes. `cargo test --locked -p iris-cli --test fmt` checks CLI modes, traversal, source failures, and EditorConfig lookup. Semantic model only: `cargo test --locked -p iris-sema`. CLI: `cargo run --locked -p iris-cli -- check SPEC`. Parse-only conformance: `cargo test --locked -p iris-cli --test parse_conformance -- --nocapture` selects every parse case in the 0.1 tree and prints counts; `iris-driver` supports `parse` and `read` so far (`catalog` and `dzn` exit 2). Reader: `cargo test --locked -p iris-core`; `cargo run --locked -p iris-cli -- read SPEC FILE --json`. Progress: `cargo test --locked -p iris-cli --test read_conformance -- --nocapture` runs every case not listed in `rust/iris-cli/tests/pending-cases.txt`. Differential: `GECODE_ADAPTER="$adapter" cargo test --locked -p iris-cli --test scalar_differential -- --ignored --nocapture`, using the pinned adapter. Language server: `cargo test --locked -p iris-lsp`; `cargo test --locked -p iris-cli --test lsp_stdio`. Formatting requests only: `cargo test --locked -p iris-lsp --test formatting`. The EditorConfig lookup sits behind iris-syntax's off-by-default `editorconfig` feature; test it with `cargo test --locked -p iris-syntax --features editorconfig`. See [rust/README.md](rust/README.md), [rust/iris-sema/README.md](rust/iris-sema/README.md), [rust/iris-core/README.md](rust/iris-core/README.md), [rust/iris-cli/README.md](rust/iris-cli/README.md), and [rust/iris-lsp/README.md](rust/iris-lsp/README.md). |
| `python/` | To be added by the Python area. |
| `cpp/` | To be added by the C++ area. |
| `java/` | To be added by the Java area. |
| `tree-sitter-iris/` | To be added by the tree-sitter area. |
| `editors/` | TextMate: in `editors/textmate/`, `npm ci && npm test` runs scope assertions, corpus snapshots, and tokenization of every tracked grammar-positive `.iris` file, once per real file; `npm run check-repo-specs` runs the tokenization check alone (see [editors/textmate/README.md](editors/textmate/README.md)). JetBrains: in `editors/jetbrains/`, `./gradlew build verifyPlugin` builds the plugin, runs its tests against a stub language server (Python 3.10+), and checks IDE compatibility; `./gradlew runIde` starts a sandbox IDE (see [editors/jetbrains/README.md](editors/jetbrains/README.md)). |
| `site/` | To be added by the site area. |
| `conformance/` | Runner tests: `cd rust && cargo test --workspace --locked`. From the root: `adapter=$(conformance/adapters/gecode/build.sh)`; `ctest --test-dir "$(dirname "$adapter")" --output-on-failure`; `(cd rust && cargo run --locked -p iris-conformance -- run --driver "\"$adapter\"" --driver-id gecode-6.5.0-5e7bbd5c26b0 ../conformance/cases/0.1/smoke)`. Specification examples: `(cd rust && cargo run --locked -p iris-conformance -- extract-examples ../spec/0.1 ../conformance/cases/0.1/spec-examples --check)` checks the generated cases; omit `--check` to regenerate them (see [conformance/README.md](conformance/README.md)). Grammar: `(cd rust && cargo run --locked -p iris-conformance -- check-grammar)` independently checks the reader specs in `conformance/cases/0.1` and `formats` against the embedded formal grammar. Explicit roots: `(cd rust && cargo run --locked -p iris-conformance -- check-grammar ../conformance/cases ../formats)` (see [conformance/README.md](conformance/README.md#checking-the-formal-grammar)). CI: `.github/workflows/conformance.yml` runs the full 0.1 suite, formats, example freshness, grammar and Rust checks on Ubuntu and macOS. |
| `formats/` | From the root, after building the adapter as for `conformance/`: `(cd rust && cargo run --locked -p iris-conformance -- run --driver "\"$adapter\"" --driver-id gecode-6.5.0-5e7bbd5c26b0 ../formats)` (see [formats/README.md](formats/README.md)). |
