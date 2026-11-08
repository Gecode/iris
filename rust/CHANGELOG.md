# Rust changelog

## Unreleased

### Breaking changes

- `gecode_iris::iris!` now expands to a value with a `spec` field, the `Spec`,
  and one field per typed handle: the handles that `iris gen --lang rust`
  prints for the spec, with the same names, types and depths. An entry named
  `spec` is the field `spec_2`. To migrate, replace `iris!(...)` with
  `iris!(...).spec` where a `Spec` is needed, or read through the handles:
  `data.get(value.n)` in place of `data.int("n")?`.

### Library

- Add Java and Kotlin printers to `gecode_iris_gen::view_code`;
  `Language::has_view` is true for Java and Kotlin, and false only for
  TypeScript.
- Add a C++ printer to `gecode_iris_gen::view_code`; `Language::has_view` is
  true for C++. `view_code` now takes the `Parse` beside the `Analysis`, since
  the C++ header's namespace is named after the spec's origin.

- Add `gecode_iris_gen::rust_view`, `RustView` and `RustHandle`: the
  declarations that `view_code` prints for Rust, as data. gecode-iris-macros
  now depends on gecode-iris-gen, which is published before it.

- Add `SemanticModel::output_entries` and `OutputEntry` to gecode-iris-sema: each
  output entry's depth, value type, possible absence and definitions.
- Add `gecode_iris_gen::view` and `view_code`, which turn a reader spec into a
  list of typed handles and print it, with a Rust printer.
- Add `Host::ex` to gecode-iris-build, so that a host constant can bound a range
  beside another expression: `lit(0)..=gecode::INT_MAX.ex()`.
- Add `Error::owner` to gecode-iris-core: where the statement an error belongs
  to starts, so that a host can find its call site as `CallSites::locate` does.
- Add a Python printer to `gecode_iris_gen::view_code`; `Language::has_view`
  is true for Python.
- Add a hand-written spec builder for each format in the format library, as
  gecode-iris examples. A test reads every case of each format through its
  builder and judges the result as the conformance runner does.
- Fix gecode-iris-build printing a capture whose type uses a record or a
  comprehension: the builder panicked, or omitted the repeat's `each` or `as`
  name. Such a type now names them like any other expression.
- Add a Python backend to gecode-iris-gen, `Language::Python`, whose programs
  use `gecode_iris.build` and pass `ruff format`, `ruff check` and ty
  unchanged, and a test that round-trips every accepted corpus spec through it.
- Add `Language::has_view` and `ViewError`. `view_code` returns a `ViewError`:
  the spec's first error, or `ViewError::NoPrinter` for a language without a
  view printer, such as Python.
- Add a C++ backend to gecode-iris-gen, `Language::Cpp`, whose programs use
  `<iris/build.hpp>`, compile without warnings under the flags of `cpp/`, and
  pass clang-format and clang-tidy 22.1.8 unchanged, and a test that
  round-trips every accepted corpus spec through it.
- Add a Java backend to gecode-iris-gen, `Language::Java`, whose programs use
  `dev.gecode.iris.build`, compile without warnings under the flags of `java/`,
  Error Prone and NullAway included, and pass google-java-format unchanged, and
  a test that round-trips every accepted corpus spec through it.
- Add a Kotlin backend to gecode-iris-gen, `Language::Kotlin`, whose programs
  use `dev.gecode.iris.kotlin.build`, compile without warnings under the flags
  of `java/` and pass ktfmt in kotlinlang style unchanged, and a test that
  round-trips every accepted corpus spec through it.
- Add a TypeScript backend to gecode-iris-gen, `Language::TypeScript`, whose
  programs use `gecode-iris/build` and pass oxfmt, oxlint with
  `--type-aware --deny-warnings` and strict tsc unchanged, and a test that
  round-trips every accepted corpus spec through it.

### CLI

- Add `iris gen --lang rust SPEC`, which prints a module of declared handles of
  a text spec, of the types the spec builder returns. A name without one value
  type and depth gets a `DynField`. No handle is named `spec`, the field of
  `iris!` that holds the `Spec`, so an entry `spec` is `spec_2`.
- Add `iris gen builder --lang python SPEC`, which prints a Python builder
  program. `iris gen --lang` accepts only languages with a view printer.
- Add `iris gen --lang python SPEC`, which prints a Python module of handles
  declared with `gecode_iris.build.declared`, of the types the Python spec
  builder returns. Identifiers follow the builder program's rules without a
  leading underscore, and a depth above 8 gets an `ib.DynField`.
- Add `iris gen builder --lang cpp SPEC`, which prints a C++ builder program.
- Add `iris gen --lang cpp SPEC`, which prints a C++ header of
  `inline constexpr` handles of the types the C++ spec builder returns, in a
  namespace named after the spec's file, laid out as clang-format 22.1.8
  writes them. Every depth gets a typed handle.
- Add `iris gen builder --lang java SPEC`, which prints a Java builder program.
- Add `iris gen builder --lang kotlin SPEC`, which prints a Kotlin builder
  program.
- Add `iris gen --lang java SPEC`, which prints a Java class of
  `public static final` handles of the types the Java spec builder returns,
  named after the spec's file in upper camel case, without a package, laid out
  as google-java-format writes it. Above depth 3 a handle reads a `Value`.
  The types of `java.lang` are qualified, so that classes of the same name in
  the user's package cannot take their place.
- Add `iris gen --lang kotlin SPEC`, which prints a Kotlin object of the same
  handles, laid out as ktfmt writes it in kotlinlang style. It imports the
  standard library types it names, for the same reason.
- Add `iris gen builder --lang typescript SPEC`, which prints a TypeScript
  builder program. `iris gen --lang typescript` says that TypeScript has no
  view generator yet.

## 0.1.0 - 2026-11-06

### Library

- Add the `gecode-iris-gen` crate, which generates spec builder programs
  from reader specs, with a Rust backend. `iris gen builder --lang rust SPEC`
  prints one.
- Add `iris-conformance compare`, which compares two reader specs as syntax
  trees, and a test that round-trips every accepted reader spec of the
  conformance cases and format library through the Rust builder.
- Add the unpublished `iris-wasm` browser boundary for reading, checking and
  formatting, with exact integer table values and bounded output previews.
- Add streaming `Data::write_json`; preserve `to_json` and its canonical bytes.
- Add validated column constructors and views to gecode-iris-core: `Value::from_ints`,
  `from_floats` and `from_strings`, `Value::leaves` and `levels`, `Entry::new`,
  and `Data::from_entries`, which return `ColumnError` for malformed columns or
  duplicate names.
- Add the unpublished `iris-js` crate, the WebAssembly boundary of the
  TypeScript package, with a parsed-spec handle and a binary data transfer.
- Share source diagnostic and formatter rendering through `gecode-iris-sema`, keeping
  `gecode_iris_cli::render_error` available as a re-export.
- Add `gecode_iris::iris!`, which checks an inline or file reader spec at
  compile time, reports its first error as a compile error with the code and
  the `iris check` text, and expands to a `Spec`. The new `gecode-iris-macros` crate
  implements it without depending on `gecode-iris-core`.
- Add `gecode_iris::build`, the spec builder of the new `gecode-iris-build` crate. It
  builds any Iris 0.1 reader spec from typed handles, prints deterministic text
  and parses it with `Spec::parse`; `pattern!` writes line patterns.
- Point builder errors at the caller's code. Every statement of
  `gecode_iris::build` records its call site with `#[track_caller]`, and
  `BuildError` shows the language code and message, the line of emitted text and
  the call site. `Error::call_site` gives the call site of a failure while
  reading a built spec, through the new `CallSites` map; a spec parsed from text
  has none, and its errors are unchanged.
- Add typed access: `Data::get` and `Data::try_get` read through a `Handle`, and
  `ValueRef` gains `as_floats`, `as_strs`, `len`, `is_empty`, `get`, `row` and
  `rows`. `Spec::source` returns the text of a spec, and `Params::set` accepts a
  handle and a formatted value.

- Implement Iris 0.1 parsing, name resolution, type and shape analysis, reading,
  and expression evaluation, including arrays, repeats, line patterns,
  sections, checks over records, parameters, instances, and catalogs.
- Provide a lossless, error-tolerant syntax tree and source diagnostics with
  stable error codes, input locations, and record paths for failed checks.
- Expose the reader through `gecode-iris`, with columnar data, zero-copy row
  slices, canonical JSON, and MiniZinc dzn output.
- Enforce language work budgets and support host limits and cancellation;
  distinguish language diagnostics, host failures, and invalid data access.

### CLI and language server

- Add `iris check`, `read`, `list`, and `conformance`, with summary, JSON, and
  dzn output, parameters, instance selection, and documented exit statuses.
- Add `iris fmt` with EditorConfig line widths, recursive directory formatting,
  standard input, check mode, and unified diffs.
- Add `iris lsp` with diagnostics, definition and reference navigation, rename,
  hover, completion, inlay hints, symbols, folding, semantic tokens, document
  and range formatting, and live reading of sample files.

### Conformance, performance, and distribution

- Provide an independent conformance runner and the Rust `iris-driver`; pass
  the full Iris 0.1 suite and format cases, alongside pinned Gecode checks in CI.
- Add parser and reader fuzzing, formatter equivalence checks, and benchmarks.
  Meet the pinned Gecode reference's speed on the documented 100 MB DIMACS
  comparison.
- Prepare cargo-dist CLI archives and installers for Linux, macOS, and Windows
  under `rust-v` tags. Homebrew publication remains disabled.
- Prepare the ten `gecode-iris` crates for crates.io, with metadata, versioned
  internal dependencies and the license text. `iris-driver`, `iris-wasm`,
  `iris-js` and the fuzz crate stay unpublished. Set the minimum supported Rust
  version to 1.88 and document API stability.
