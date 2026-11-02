# fuzz

Coverage-guided fuzzing of the Rust parser, formatter and reader with
[cargo-fuzz](https://rust-fuzz.github.io/book/cargo-fuzz.html) and libFuzzer,
part of milestone M3 in [PLAN.md §7.4](../../PLAN.md#74-fuzzing). Differential
fuzzing across implementations belongs to M4c.

The crate needs nightly Rust, so it declares its own empty workspace and keeps
its own `Cargo.lock`; the stable workspace commands in
[rust/README.md](../README.md) never build it. Install the tools once:

```sh
rustup toolchain install nightly --component rustfmt,clippy
cargo install cargo-fuzz --locked
```

## Targets

- `parse` parses arbitrary bytes as a reader spec. Valid UTF-8 must give a
  syntax tree whose text equals the source byte for byte; invalid UTF-8 must
  give the source error.
- `format` formats syntax-clean reader specs at widths 40, 80 and 100. It
  checks that formatting twice gives identical text and that the output parses
  to the same tree, including node boundaries, token text and comments, with
  whitespace ignored. Source failures and syntax errors are skipped.
- `read` splits its input at the first NUL byte: the bytes before it are the
  reader spec and the bytes after it are the data input. An input without NUL
  is a spec with empty data, so specs here never contain NUL; `parse` covers
  those. It parses the spec, reads and catalogs the input with no parameters,
  and renders every result as canonical JSON, dzn, or error JSON.

Reading uses the work budgets of
[Specification §10](../../spec/0.1/10-work-budgets.md), so every execution
ends after at most 2^20 idle iterations. Such budget-bound inputs are slow but
legal: an idle loop to the limit takes about a second in a release build and
several seconds under fuzzing instrumentation. The 60-second timeout separates
them from real hangs.

## Running

From `rust/`, write the seed corpora, then fuzz one target:

```sh
python3 -I fuzz/seed-corpus.py
cargo +nightly fuzz run --sanitizer none parse -- -max_total_time=300 -max_len=8192 -timeout=60
cargo +nightly fuzz run --sanitizer none format -- -max_total_time=300 -max_len=8192 -timeout=60
cargo +nightly fuzz run --sanitizer none read -- -max_total_time=300 -max_len=8192 -timeout=60
cargo +nightly fmt --manifest-path fuzz/Cargo.toml -- --check
cargo +nightly clippy --manifest-path fuzz/Cargo.toml --all-targets --locked -- -D warnings
```

`+nightly` overrides the stable channel in `rust/rust-toolchain.toml`. Every
Iris crate forbids unsafe code, so the runs skip AddressSanitizer. That
sanitizer would check only dependencies, and it slows allocation-heavy
reading several times. Debug assertions and overflow checks stay on. A failing
input is written to `fuzz/artifacts/TARGET/`; reproduce it with
`cargo +nightly fuzz run --sanitizer none TARGET fuzz/artifacts/TARGET/FILE`.

The [fuzz workflow](../../.github/workflows/fuzz.yml) checks this crate's
formatting and lints, then runs each target for two minutes on pushes and pull
requests that touch `rust/`, the conformance cases, or the formats. It uploads
failing inputs as artifacts.

## Corpora

`seed-corpus.py` generates the seeds from the reader specs and inputs in
`conformance/cases/0.1` and `formats/` instead of committing them. The cases
stay the single source, new cases join the corpora automatically, and the
repository does not carry hundreds of copies that libFuzzer would rename and
grow on every run. The parser and formatter use the same reader spec seeds.
The read corpus leaves out cases for Specification
chapter 10, which take each budget to its limit by design and would spend most
of a short run on a handful of slow seeds. `corpus/`, `artifacts/` and
`coverage/` are ignored.

Fix each finding in the crate that owns the behavior, and add its input there
as a regression test, so it runs with `cargo test --workspace` on every push.
