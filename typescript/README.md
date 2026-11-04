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

The package is ESM only and needs Node 22.12 or later. This task provides the
Node entry; browser and Workers entries follow. The package is `private` until
npm publication in milestone M7.

## Build

Building needs stable Rust with the `wasm32-unknown-unknown` target,
wasm-bindgen-cli 0.2.129 and Node 22.18 or later, which runs the TypeScript
build script and tests directly. npm installs Binaryen's `wasm-opt` and the
TypeScript compiler.

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
-Oz`, validates it and prints its raw and gzip sizes, and compiles `src/` into
`dist/`. `wasm/` and `dist/` are build output and are not committed. The build
respects `CARGO_TARGET_DIR`.

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
`npm run test:types` type-checks the sources, the tests and
[`test/types/api.ts`](test/types/api.ts) against the published declarations in
`dist/`; each `@ts-expect-error` line in that file must report an error.

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

- `src/index.ts` is the runtime-neutral API; `src/node.ts` is the Node entry,
  which loads the module synchronously, so `require()` works and `Spec.parse`
  runs at module scope.
- `src/backend.ts` is the only module that calls the WebAssembly glue. It turns
  failure envelopes into errors.
- `src/transfer.ts` decodes the binary layout documented in
  [`rust/iris-js/src/transfer.rs`](../rust/iris-js/src/transfer.rs).
- `src/spec.ts`, `src/data.ts` and `src/errors.ts` hold the classes.
