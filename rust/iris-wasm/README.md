# iris-wasm

The in-memory browser boundary for the playground, delivered in milestone M5
([PLAN.md §8.10](../../PLAN.md#810-documentation-site-site)). The unpublished
crate exposes `read`, `check`, and `format` as string-to-string functions, both
natively and through wasm-bindgen. It does not depend on the CLI or filesystem
configuration. The site owns the worker, deadlines, memory ceiling and UI.

Requests carry language `0.1`, source text and origin labels. Parameter values
are strings. Responses distinguish success, language diagnostics, unsupported
features and host failures. Canonical JSON, dzn and summary remain complete Rust
text; table integers are decimal strings, including signed 64-bit bounds.
Table previews are bounded and explicitly mark omissions. Each source is limited
to 1 MiB, each origin to 256 bytes, parameters to 64 KiB, and each output to
1 MiB. Output overflow discards partial results. Check uses semantic error order;
formatting is syntax-only at width 100. There is no lint API yet.

From `rust/`, run `cargo test --locked -p iris-wasm`. The browser build is owned
by `site/scripts/build-wasm.mjs`; see the [site guide](../../site/README.md).
The binding dependency and CLI must both be wasm-bindgen 0.2.129. This crate
stays `publish = false`; CLI distribution archives are unaffected.
