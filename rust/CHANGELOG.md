# Rust changelog

## Unreleased

### Library

- Add the unpublished `iris-wasm` browser boundary for reading, checking and
  formatting, with exact integer table values and bounded output previews.
- Add streaming `Data::write_json`; preserve `to_json` and its canonical bytes.
- Add validated column constructors and views to iris-core: `Value::from_ints`,
  `from_floats` and `from_strings`, `Value::leaves` and `levels`, `Entry::new`,
  and `Data::from_entries`, which return `ColumnError` for malformed columns or
  duplicate names.
- Add the unpublished `iris-js` crate, the WebAssembly boundary of the
  TypeScript package, with a parsed-spec handle and a binary data transfer.
- Share source diagnostic and formatter rendering through `iris-sema`, keeping
  `iris_cli::render_error` available as a re-export.
- Add `gecode_iris::iris!`, which checks an inline or file reader spec at
  compile time, reports its first error as a compile error with the code and
  the `iris check` text, and expands to a `Spec`. The new `iris-macros` crate
  implements it without depending on `iris-core`.
- Add `gecode_iris::build`, the spec builder of the new `iris-build` crate. It
  builds any Iris 0.1 reader spec from typed handles, prints deterministic text
  and parses it with `Spec::parse`; `pattern!` writes line patterns.
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
- Prepare crate metadata and versioned internal dependencies for publication;
  all crates remain unpublished with `publish = false`. Set the minimum
  supported Rust version to 1.88 and document API stability.
