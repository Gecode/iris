# typescript

The npm package `gecode-iris` gives JavaScript and TypeScript the Rust
reference implementation, compiled to WebAssembly from the crate
[`rust/iris-js`](../rust/iris-js/README.md). The package adds no reader
behavior: parsing, reading, errors, canonical JSON and dzn all come from
iris-core, so a conformance difference is a binding bug. `PLAN.md` §8.16
describes the package; milestone M4f builds it.

```ts
import { readFileSync } from "node:fs";
import { Spec } from "gecode-iris";

const spec = Spec.parse(readFileSync("jobshop.iris"), "jobshop.iris");
const data = spec.read(readFileSync("jobshop1.txt"), "jobshop1.txt", {
  params: { instance: "la01" },
});
data.int("n_jobs"); // number
data.ints("machine", [3]); // number[]
process.stdout.write(data.toJSON());
```

The package is ESM only and needs Node 22.12 or later. It runs in Node,
browsers, workerd (Cloudflare Workers) and other edge runtimes; see
[Runtimes](#runtimes). The package is `private` until npm publication in
milestone M7.

## Build

Building needs stable Rust with the `wasm32-unknown-unknown` target,
wasm-bindgen-cli 0.2.129 and Node 22.18 or later, which runs the TypeScript
build script and tests directly. npm installs Binaryen's `wasm-opt`, the
TypeScript compiler, and Playwright and Wrangler for the smoke tests.

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --locked --version 0.2.129
npm ci
npm run build
```

`npm run build` runs [`scripts/build.ts`](scripts/build.ts). It checks the
wasm-bindgen version against the pin in `rust/iris-js/Cargo.toml`, builds the
crate with the workspace's `wasm` profile, generates the glue into `wasm/` with
`--target web --omit-default-module-path`, optimizes the module with `wasm-opt
-Oz`, validates it and prints its raw and gzip sizes, writes the base64 module
of the inline entry and prints its sizes, and compiles `src/` into `dist/`. `wasm/` and `dist/` are build output and are not committed. The build
respects `CARGO_TARGET_DIR`.

## Runtimes

Spec builders define specs at module scope, so every entry point loads the
WebAssembly module before an importing module evaluates: `Spec.parse` works
in the importing module's top level, with no `await` and no `initialize()`.
The export conditions of `gecode-iris` choose the entry point:

| Condition | Entry point | How it loads the module |
|---|---|---|
| `workerd`, `edge-light` | `dist/workerd.js` | A static import of `wasm/iris_js_bg.wasm`, which the runtime's bundler compiles; then synchronous instantiation |
| `node` | `dist/node.js` | Reads the file and compiles it synchronously, so `require("gecode-iris")` works as well as `import` |
| `browser`, and `default` for other runtimes | `dist/browser.js` | Top-level `await` fetches `new URL("../wasm/iris_js_bg.wasm", import.meta.url)` |

- **Node**, and Deno and Bun through their `node` condition: `import` and
  `require()` both load the module synchronously. `require()` of an ES module
  needs Node 22.12 or later.
- **Browsers:** an importing module waits for the browser entry's top-level
  `await`, so its own top level runs after the module is loaded. Bundlers
  differ in how they handle the `new URL(..., import.meta.url)` pattern:
  - `vite build` (Vite 7) and webpack 5 recognize it and emit the `.wasm` file
    as an asset. Vite's dev server pre-bundles dependencies and leaves the
    URL pointing at a missing file, so exclude the package with
    `optimizeDeps: { exclude: ["gecode-iris"] }` in `vite.config.js`.
  - esbuild (0.28) bundles the JavaScript but neither emits the `.wasm` file
    nor rewrites the URL, even with a `file` loader for `.wasm`
    ([esbuild#795](https://github.com/evanw/esbuild/issues/795)). With esbuild
    and other bundlers that do not support the pattern, import
    `gecode-iris/inline`, which needs no asset; or copy
    `wasm/iris_js_bg.wasm` into your output yourself and load it with
    `gecode-iris/manual` and `initialize()` from its URL.

  Without a bundler, serve the package directory and map `gecode-iris` to
  `dist/browser.js` in an import map.
- **workerd and edge runtimes:** Wrangler resolves the `workerd` condition and
  uploads the `.wasm` file as a compiled module, the only way workerd allows
  WebAssembly. `edge-light` uses the same entry point but has no test; a
  bundler that needs a suffix such as `?module` on `.wasm` imports cannot use
  it.

### `gecode-iris/inline`

The `inline` entry point embeds the module as base64 in
`wasm/iris_js_inline.js` and compiles it synchronously, so it needs no file
access, `fetch` or bundler support, and it works anywhere that compiles
WebAssembly from bytes; workerd does not. The embedded module is 783,519
bytes (765 KiB) raw and 328,573 bytes gzipped, a third more than the 587,623
bytes (574 KiB) raw and 249,808 bytes gzipped of the `.wasm` file; `npm run
build` prints both. Chrome compiles modules up to 8 MB synchronously on the
main thread, so the inline entry also works in a page.

### `gecode-iris/manual` and `initialize()`

`gecode-iris/manual` exports the same API but loads nothing. Await
`initialize(source)` before the first `Spec.parse`, which throws until then.
The source is a `WebAssembly.Module`, the module's bytes, a `Response`, or a
URL to fetch, or a promise of any of these; for example the module on a CDN:

```ts
import { initialize, Spec } from "gecode-iris/manual";

await initialize(new URL("https://cdn.example.com/iris_js_bg.wasm"));
const spec = Spec.parse(text);
```

Every entry point exports `initialize`. Once one load has started, later calls
return its promise and ignore their source, so calling it after an entry point
has loaded the module does nothing. Exactly one module instance is ever
installed: if an entry point that loads synchronously, such as
`gecode-iris/inline`, is imported while a load is pending, the pending load
resolves without replacing it. A failed load may be retried.

### Content Security Policy

WebAssembly compilation is governed by `script-src`. A page with a strict
Content-Security-Policy must allow it with `'wasm-unsafe-eval'`, for both the
browser and the inline entry points:

```text
Content-Security-Policy: script-src 'self' 'wasm-unsafe-eval'
```

Without it, loading the package fails with a `CompileError` that names the
policy.

## API

- `Spec.parse(text, origin = "<string>")` parses and checks a reader spec. A
  string is encoded as UTF-8; a `Uint8Array` is used as given, so invalid UTF-8
  throws `E-SPEC-UTF8`.
- `spec.read(input, origin = "<input>", { params })` reads one instance from a
  string or a `Uint8Array`, and `spec.catalog(...)` lists the instances with the
  same arguments. `params` is an object of strings or an iterable of
  `[name, value]` pairs, such as a `Map`; pairs are applied in order. Use
  `{ instance: "2" }` or `{ instance: "la01" }` to select an instance.
- `data.entries` lists the entries in definition order as frozen
  `{ name, valueType, depth }` objects; `valueType` is `"int"`, `"float"` or
  `"string"`. `data.entry(name)` returns one entry or `undefined`, and
  `data.has(name)` tests for it.
- A path is an array of indices counted from 0. `data.size(name, path)`
  returns the length of the array that `path` selects.
- `data.int`, `data.bigInt`, `data.float` and `data.string(name, path = [])`
  return a scalar. `data.ints`, `data.bigInts`, `data.floats` and
  `data.strings(name, path = [])` return a row: the selection must have
  depth 1.
- `data.value(name, path = [])` returns a scalar or nested arrays, which may
  be ragged, and `data.toObject()` returns every entry as an object in entry
  order. With the option `{ bigint: true }`, both return integers as `bigint`.
- `data.toJSON()` returns the canonical JSON document, ending in a newline, and
  `data.toDzn()` returns the dzn text of the Specification's appendix B. Both
  are rendered by iris-core when called. `JSON.stringify(data)` calls `toJSON`
  and so produces the document as one JSON string; write `data.toJSON()`
  directly instead.
- Every accessor returns a copy, so changing a returned array never changes the
  data.

### 64-bit integers

Iris integers are signed 64-bit. Accessors that return `number` (`int`,
`ints`, `value` and `toObject`) throw `RangeError` for a value beyond
`Number.MAX_SAFE_INTEGER` (2^53 − 1) in magnitude, rather than round it.
`bigInt`, `bigInts` (a new `BigInt64Array`) and the `{ bigint: true }` option
are always exact. Floats are always `number`.

### Errors

A language error throws `IrisError`, a subclass of `Error`. Its fields come
from the canonical error document:

- `code`, such as `E-CHECK-FAILED`, and `message`, without a location;
- `inputLocation` and `specLocation`, each a `Location`
  (`{ origin, line, column }`, any of them `null` when unknown) or `null`. A
  spec error has only a spec location; a read error has an input location and
  usually the spec location of the responsible statement;
- `context`, what was being read, or `null`;
- `recordPath`, the record indices of a failed check over records, or `null`;
- `diagnostic`, the text `iris read` prints: location, message, input excerpt
  with a caret, context and spec line;
- `toJSON()`, the canonical error document.

Other failures use built-in errors. An unknown entry or an index out of bounds
throws `RangeError`, and a selection of the wrong value type or depth throws
`TypeError`. A string with a lone surrogate, or a parameter that is not a
string, throws `TypeError` instead of being changed to U+FFFD. Using a disposed
spec throws `TypeError`. A host failure throws `RangeError` when the data
exceeds the transfer format's 32-bit limits, and `Error` otherwise.

### Memory

`Data` is copied out of WebAssembly memory into JavaScript typed arrays, so it
never needs to be freed. Only `Spec` holds WebAssembly memory. It is released
when the spec is garbage-collected; to release it earlier, dispose of the spec
with `using` or `spec[Symbol.dispose]()`. Calling `free()` is never required.

```ts
{
  using spec = Spec.parse(text);
  process.stdout.write(spec.read(input).toDzn());
} // the spec's memory is released here
```

### Work budgets and deadlines

Every read is bounded by the language's fixed work budgets ([Specification
chapter 10](../spec/0.1/10-work-budgets.md)). A read that exhausts one throws
`IrisError` with the code `E-BUDGET` and its input and spec locations. The
package has no budget settings.

Calls into WebAssembly are synchronous and cannot be interrupted on their own
thread. For a wall-clock deadline, read in a worker and terminate it when the
deadline passes, as the playground does. In Node, use `node:worker_threads`:

```ts
// read-worker.ts
import { parentPort, workerData } from "node:worker_threads";
import { Spec } from "gecode-iris";

const { spec, input } = workerData;
parentPort?.postMessage(Spec.parse(spec).read(input).toJSON());
```

```ts
import { Worker } from "node:worker_threads";

function readWithDeadline(spec: string, input: string, milliseconds: number): Promise<string> {
  const worker = new Worker(new URL("./read-worker.ts", import.meta.url), {
    workerData: { spec, input },
  });
  const timer = setTimeout(() => void worker.terminate(), milliseconds);
  return new Promise<string>((resolve, reject) => {
    worker.once("message", resolve);
    worker.once("error", reject);
    worker.once("exit", () => reject(new Error("read stopped at the deadline")));
  }).finally(() => clearTimeout(timer));
}
```

In a browser, use a Web Worker the same way.

## Tests

From this directory, after `npm run build`:

```sh
npm test
npm run test:types
```

`npm test` runs the `node:test` files in `test/` against the built package.
[`test/runtimes.test.ts`](test/runtimes.test.ts) runs each fixture in
[`test/smoke/`](test/smoke/) in its own Node process: the Node entry from ESM
and from `require()`, the inline entry, and the manual entry with
`initialize()` from bytes, from a promised data URL after a failed load, and
while the inline entry installs its module before the pending bytes arrive.
Each reads the four-by-four case of
[`formats/sudoku`](../formats/sudoku/README.md) and must print the case's
expected JSON.
`npm run test:types` type-checks the sources, the tests and
[`test/types/api.ts`](test/types/api.ts) against the published declarations in
`dist/`; each `@ts-expect-error` line in that file must report an error.

`npm run test:package` packs the package with `npm pack`, installs the
tarball into a temporary consumer, type-checks an import of `gecode-iris`,
`gecode-iris/inline` and `gecode-iris/manual` with `strict` on and
`skipLibCheck` off, and imports each in Node. It catches declarations and
files that the repository has but the tarball lacks.

Two smoke tests read the same case outside Node:

```sh
npx playwright install chromium   # once
npm run test:browser
npm run test:workerd
```

`npm run test:browser` serves the package and opens pages in Chromium through
the pinned Playwright, with an import map that resolves `gecode-iris` to the
`browser` export and then to the `inline` export. Each page sends a strict
Content-Security-Policy with `'wasm-unsafe-eval'`; a third page omits it and
must fail. Set `BROWSER_CHANNEL=chrome` to use an installed Chrome instead of
Playwright's Chromium. `npm run test:workerd` serves
[`test/smoke/workerd`](test/smoke/workerd/wrangler.jsonc) with the pinned
Wrangler's `wrangler dev`, which bundles the package with the `workerd`
condition and imports the format files as text modules. The worker reads the
case at module scope; the test fetches its JSON. The configuration turns
Wrangler's usage metrics off.

[`test/conformance-driver.ts`](test/conformance-driver.ts) speaks the
[driver protocol](../conformance/README.md#four-driver-commands) through the
public API. From the repository root, it passes every case with no driver ID or
exemptions:

```sh
(cd rust && cargo run --locked -p iris-conformance -- run --driver "\"$(command -v node)\" \"$PWD/../typescript/test/conformance-driver.ts\"" ../conformance/cases/0.1)
(cd rust && cargo run --locked -p iris-conformance -- run --driver "\"$(command -v node)\" \"$PWD/../typescript/test/conformance-driver.ts\"" ../formats)
```

## Code style

The formatter's output is the style. The package does not build with Vite, so
it pins oxfmt and type-aware oxlint as devDependencies rather than using Vite+.
[`.oxfmtrc.json`](.oxfmtrc.json) sets the repository's line width of 100, and
[`.oxlintrc.json`](.oxlintrc.json) enables the correctness, suspicious and perf
rules as errors. `node:test` handles the promises that `test()` returns, so
`no-floating-promises` is off in test files. Lint and type tests need the
generated glue, so run them after the build:

```sh
npm run format        # rewrite files
npm run format:check  # check without writing
npm run lint          # oxlint --type-aware --deny-warnings
```

The Rust crate follows the workspace's rustfmt and Clippy settings.

## Layout

- `src/index.ts` is the runtime-neutral API and the `gecode-iris/manual`
  entry. `src/node.ts`, `src/browser.ts`, `src/workerd.ts` and `src/inline.ts`
  are the entry points of [Runtimes](#runtimes); each loads the module and
  re-exports the API.
- `src/loading.ts` loads the module. Every loading path installs its instance
  through one synchronous step, so only one instance is ever installed, and
  its declarations name no glue type, so the public declarations never import
  the glue.
- `src/backend.ts` calls the glue's exports once a module is loaded and turns
  failure envelopes into errors.
- `src/transfer.ts` decodes the binary layout documented in
  [`rust/iris-js/src/transfer.rs`](../rust/iris-js/src/transfer.rs).
- `src/spec.ts`, `src/data.ts` and `src/errors.ts` hold the classes.
