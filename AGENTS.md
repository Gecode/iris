# Repository guidance

This file is the guidance for everyone who works in this repository, human or
coding agent. `CLAUDE.md` imports it.

## Where things are

- [`PLAN.md`](PLAN.md) is the master plan: versions, goals, architecture,
  repository layout (§5), deliverables, milestones, and decisions. Read it
  before starting work on a component.
- `CONTEXT.md` is the glossary. Use its terms in every document.
- [`RELEASING.md`](RELEASING.md) is the entry point for component releases and
  site deployment.
- `docs/adr/` holds the architecture decision records.
- `docs/1.0-ideas.md` collects candidate changes for Iris 1.0. Add an entry
  there when Iris 0.1 cannot express something, or expresses it awkwardly.
- [`docs/builders.md`](docs/builders.md) is the design brief for spec
  builders in every implementation language. Follow its principles when
  designing or reviewing a builder API.
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
| `rust/` | In `rust/`: `cargo build --workspace --locked`; `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings`; `cargo test --workspace --locked`. WASM boundary: `cargo test --locked -p iris-wasm`; `cargo +1.88.0 check -p iris-wasm --target wasm32-unknown-unknown --locked`; browser build/parity commands are in the site row. Parser only: `cargo test --locked -p iris-syntax`; add `repository_losslessness -- --nocapture` to report byte-for-byte reconstruction of every `.iris` file. Formatter only: `cargo test --locked -p iris-syntax repository_formatting -- --nocapture` reports same-tree and idempotence counts at widths 40, 80 and 100; `guide_examples -- --nocapture` checks the pairs in [docs/formatting.md](docs/formatting.md). Formatter CLI: `cargo run --locked -p iris-cli -- fmt PATH...` formats in place; add `--check` to check without writing and `--diff` to preview changes. `cargo test --locked -p iris-cli --test fmt` checks CLI modes, traversal, source failures, and EditorConfig lookup. Semantic model only: `cargo test --locked -p iris-sema`. CLI: `cargo run --locked -p iris-cli -- check SPEC`. Parse-only conformance: `cargo test --locked -p iris-cli --test parse_conformance -- --nocapture` selects every parse case in the 0.1 tree and prints counts; `iris-driver` supports `parse`, `read`, `catalog`, and `dzn`; `catalog --format dzn` renders catalog data. CLI completion: `cargo test --locked -p iris-cli --test complete` checks summary, version, conformance options and exit statuses; `cargo test --locked -p gecode-iris --doc` runs the umbrella API and `iris!` examples; `iris!` macro: `cargo test --locked -p iris-macros` checks literal offsets, `cargo test --locked -p gecode-iris --test macro_errors` checks compile errors against trybuild snapshots (bless with `TRYBUILD=overwrite`), and `cargo test --locked -p gecode-iris --test macro_corpus -- --nocapture` compiles every valid conformance and format spec through the macro and compares its reads with `Spec::parse`; `cargo run --locked -p iris-cli -- conformance target/debug/iris-driver ../conformance/cases/0.1/smoke` runs the shared runner through the CLI (add `--driver-id ID` for exemptions). Reader: `cargo test --locked -p iris-core`; `cargo run --locked -p iris-cli -- read SPEC FILE...` prints a summary by default (or use `--summary`); `cargo run --locked -p iris-cli -- read SPEC FILE --json` or `cargo run --locked -p iris-cli -- read SPEC FILE --dzn`; add `--param NAME=VALUE` to set parameters or select an instance; `cargo run --locked -p iris-cli -- list SPEC FILE...` lists instances; add `--dzn` to render catalog data. Full conformance: `cargo test --locked -p iris-cli --test read_conformance -- --nocapture` requires every discovered case in `conformance/cases/0.1` and `formats` to pass without exemptions and prints operation/area counts. Differential: `GECODE_ADAPTER="$adapter" cargo test --locked -p iris-cli --test scalar_differential -- --ignored --nocapture`, using the pinned adapter. Budgets: `cargo test --locked -p iris-core budget -- --nocapture` checks limits, deferred hard errors, linear accounting and prompt exhaustion. Checks: `cargo test --locked -p iris-core checks_scale_linearly -- --nocapture` bounds check work; `cargo test --release --locked -p iris-core check_doubling_measurements -- --ignored --nocapture` prints the doubling tables. Language server: `cargo test --locked -p iris-lsp`; `cargo test --locked -p iris-cli --test lsp_stdio`. Formatting requests only: `cargo test --locked -p iris-lsp --test formatting`. Fuzzing (nightly, separate crate): `python3 -I fuzz/seed-corpus.py` writes seed corpora; `cargo +nightly fuzz run --sanitizer none parse -- -max_total_time=300 -max_len=8192 -timeout=60` fuzzes the parser, and `read` in place of `parse` fuzzes the reader; `cargo +nightly fmt --manifest-path fuzz/Cargo.toml -- --check` and `cargo +nightly clippy --manifest-path fuzz/Cargo.toml --all-targets --locked -- -D warnings` check and lint the crate (see [rust/fuzz/README.md](rust/fuzz/README.md)). Release binaries: from `rust/`, install `cargo install cargo-dist --locked --version 0.32.0`; `dist plan` previews all platforms, `dist plan --tag rust-v0.1.0` checks release-tag URLs, `dist generate --check --mode ci` checks the generated workflow, and `dist build --target aarch64-apple-darwin --tag rust-v0.1.0` builds locally (replace the target with the host triple). Run `/tmp/iris-zdev/actionlint .github/workflows/rust-v-release.yml` from the root to lint the workflow. The EditorConfig lookup sits behind iris-syntax's off-by-default `editorconfig` feature; test it with `cargo test --locked -p iris-syntax --features editorconfig`. See [rust/README.md](rust/README.md), [rust/iris-sema/README.md](rust/iris-sema/README.md), [rust/iris-core/README.md](rust/iris-core/README.md), [rust/iris-cli/README.md](rust/iris-cli/README.md), and [rust/iris-lsp/README.md](rust/iris-lsp/README.md). |
| `python/` | To be added by the Python area. |
| `cpp/` | To be added by the C++ area. |
| `java/` | To be added by the Java area. |
| `tree-sitter-iris/` | To be added by the tree-sitter area. |
| `editors/` | TextMate: in `editors/textmate/`, `npm ci && npm test` runs scope assertions, corpus snapshots, and tokenization of every tracked grammar-positive `.iris` file, once per real file; `npm run check-repo-specs` runs the tokenization check alone (see [editors/textmate/README.md](editors/textmate/README.md)). JetBrains: in `editors/jetbrains/`, `./gradlew build verifyPlugin` builds the plugin, checks IDE compatibility, and runs headless tests of highlighting, diagnostics and restart, executable lookup, settings, and the missing-server notification against a stub server (a Unix shell and Python 3.10+); for the real-server DIMACS highlighting and edit/fix diagnostic test, build in `rust/` with `cargo build --locked -p iris-cli --bin iris`, then run `IRIS_TEST_EXECUTABLE=/absolute/path/to/iris ./gradlew test verifyPlugin`. Without the variable Gradle prints a skip reason; CI requires it. The [JetBrains workflow](.github/workflows/jetbrains.yml) builds the server and plugin headlessly on Ubuntu and verifies compatibility with IDEA 2025.3, 2026.1, and 2026.2. `./gradlew runIde` starts a sandbox IDE (see [editors/jetbrains/README.md](editors/jetbrains/README.md)). |
| `site/` | In `site/`: `npm ci`; `npm run build`; `npm run check` compares example output/downloads with the local CLI, checks the hero mapping, and runs Astro diagnostics, internal link/anchor and asset checks, HTML validation, and text/control contrast checks against the existing build; `npm run check:wasm` executes the optimized WASM against every format case and enforces the 1 MiB raw / 384 KiB gzip budget; `npm run check:browser` checks playground interactions with Playwright (CI installs Chromium; locally set `PLAYWRIGHT_MODULE` and `BROWSER_CHANNEL=chrome`). `npm run check:examples` and `npm run check:hero` run those example checks separately; `npx astro check` runs diagnostics alone. Dev/build and example checks build the local `iris`/`iris-driver` with Cargo and respect `CARGO_TARGET_DIR`; `src/generated/` and `public/examples/` are ignored build outputs. Requires stable Rust with `wasm32-unknown-unknown`, wasm-bindgen-cli 0.2.129, Node 22.12+ and `rsvg-convert` (`librsvg` on macOS, `librsvg2-bin` on Ubuntu). `npm run dev` starts development; `npm run dev:docs` explicitly disables the WASM runtime for docs-only work; `npm run preview -- --host 127.0.0.1` previews the built site with search. Lint CI with `/tmp/iris-zdev/actionlint .github/workflows/site.yml`. See [site/README.md](site/README.md). |
| `conformance/` | Runner tests: `cd rust && cargo test --workspace --locked`. From the root: `adapter=$(conformance/adapters/gecode/build.sh)`; `ctest --test-dir "$(dirname "$adapter")" --output-on-failure`; `(cd rust && cargo run --locked -p iris-conformance -- run --driver "\"$adapter\"" --driver-id gecode-6.5.0-5e7bbd5c26b0 ../conformance/cases/0.1/smoke)`. Specification examples: `(cd rust && cargo run --locked -p iris-conformance -- extract-examples ../spec/0.1 ../conformance/cases/0.1/spec-examples --check)` checks the generated cases; omit `--check` to regenerate them (see [conformance/README.md](conformance/README.md)). Grammar: `(cd rust && cargo run --locked -p iris-conformance -- check-grammar)` independently checks the reader specs in `conformance/cases/0.1` and `formats` against the embedded formal grammar. Explicit roots: `(cd rust && cargo run --locked -p iris-conformance -- check-grammar ../conformance/cases ../formats)` (see [conformance/README.md](conformance/README.md#checking-the-formal-grammar)). Rust driver from the root after the workspace build: `(cd rust && cargo run --locked -p iris-conformance -- run --driver "\"$PWD/target/debug/iris-driver\"" ../conformance/cases/0.1)`; replace the root with `../formats` for format cases. No Rust driver ID or exemptions are needed. CI: `.github/workflows/conformance.yml` runs the full 0.1 suite and formats against both Rust and the pinned Gecode adapter, plus example freshness, grammar and Rust checks, on every push and pull request on Ubuntu and macOS. |
| `formats/` | From the root, after building the adapter as for `conformance/`: `(cd rust && cargo run --locked -p iris-conformance -- run --driver "\"$adapter\"" --driver-id gecode-6.5.0-5e7bbd5c26b0 ../formats)` (see [formats/README.md](formats/README.md)). |
