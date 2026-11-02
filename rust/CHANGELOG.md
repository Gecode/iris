# Rust changelog

## Unreleased

### Library

- Add the unpublished `iris-wasm` browser boundary for reading, checking and
  formatting, with exact integer table values and bounded output previews.
- Add streaming `Data::write_json`; preserve `to_json` and its canonical bytes.
- Share source diagnostic and formatter rendering through `iris-sema`, keeping
  `iris_cli::render_error` available as a re-export.

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
