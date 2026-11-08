# typescript

The npm package `gecode-iris` gives JavaScript and TypeScript the Rust
reference implementation, compiled to WebAssembly from the crate
[`rust/iris-js`](../rust/iris-js/README.md). The package adds no reader
behavior: parsing, reading, errors, canonical JSON and dzn all come from
gecode-iris-core, so a conformance difference is a binding bug. `PLAN.md` §8.16
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
[Runtimes](#runtimes).

## Install

npm publication is planned for milestone M7. Once the package is published,
install it with:

```sh
npm install gecode-iris
```

Until then, build it from a checkout as described in [Build](#build). In an npm
project beside an Iris checkout named `iris`:

```sh
(cd ../iris/typescript && npm ci && npm run build)
npm install ../iris/typescript
```

npm links the checkout's package directory, so rerun the build after updating
the checkout. The site's
[TypeScript guide](https://iris.gecode.dev/using/typescript/) shows these
commands; its build checks that they still appear here.

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
WebAssembly from bytes; workerd does not. The embedded module is 788,263
bytes (770 KiB) raw and 330,901 bytes gzipped, a third more than the 591,181
bytes (577 KiB) raw and 251,419 bytes gzipped of the `.wasm` file; `npm run
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
  are rendered by gecode-iris-core when called. `JSON.stringify(data)` calls `toJSON`
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
- `callSite`, for a spec made by the [spec builder](#spec-builder), the
  `{ file, line, column, function }` of the builder call that wrote the
  responsible statement, or `null`; the diagnostic then ends with a
  `call site:` line;
- `toJSON()`, the canonical error document.

Other failures use built-in errors. An unknown entry or an index out of bounds
throws `RangeError`, and a selection of the wrong value type or depth throws
`TypeError`. A string with a lone surrogate, or a parameter that is not a
string, throws `TypeError` instead of being changed to U+FFFD. Using a disposed
spec throws `TypeError`. A host failure throws `RangeError` when the data
exceeds the transfer format's 32-bit limits, and `Error` otherwise.

A trap in the WebAssembly module, which only a bug in the package can cause,
throws `IrisInternalError`. It is a subclass of `Error`, not of `IrisError`, and
its `cause` is the original `WebAssembly.RuntimeError`. The package then
replaces the WebAssembly instance with a fresh one from the same compiled
module, so later calls work, including calls on specs parsed before the trap.

If creating the fresh instance fails, the call still throws its own error, or
returns its result after a large read. The replacement stays pending and is
retried before the next call, which throws the failure if the retry fails too.

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

WebAssembly memory grows but never shrinks. When memory exceeds 64 MiB after
a `read` or `catalog` call, successful or not, the package replaces the
instance with a fresh one, which releases the old memory. The threshold is
`MEMORY_THRESHOLD` in [`src/backend.ts`](src/backend.ts). A replacement is
invisible to callers: each `Spec` keeps its text and origin, and parses them
again in the new instance before its next call.

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

## Spec builder

`gecode-iris/build` builds a reader spec in TypeScript instead of in `.iris`
text. It exports the whole package API too, and has the same export
conditions as `gecode-iris`; `gecode-iris/build/inline` and
`gecode-iris/build/manual` pair it with the other loaders. The builder is
pure TypeScript: it prints the reader spec and parses it with `Spec`.

```ts
import { atLeast, float, p, spec } from "gecode-iris/build";

const matrix = spec("matrix", (b) => {
  const rows = b.capture("rows", atLeast(1));
  const columns = b.capture("columns", atLeast(1));
  b.line(p`${rows} x ${columns}`);
  const cells = b.repeat(rows, (r) => r.read("cells", float.dim(columns)));
  b.check(rows.le(100), "too many rows");
  return { rows, columns, cells };
});

const data = matrix.read("2 x 2\n1 2\n3 4\n");
data.cells; // number[][]
matrix.source; // the reader spec as Iris text
```

- Statements are methods of a scope: `read`, `list`, `let`, `set`, `param`,
  `check`, `line`, `find`, `expect`, `skipLine`, `skipLines`, `index` and
  `defineType`. Blocks take callbacks: `repeat`, `repeatUntilEof`,
  `repeatUntil`, `sections` with `arms.arm(...)`, `if` with `elseIf`,
  `else` and `elseReadIf`, and `instances`. A callback returns the handles
  it wants to keep, and must not be `async`.
- A handle is `Field<V, D, P>`: value type `"int"`, `"float"`, `"num"`,
  `"string"` or `"dyn"`; depth 0 to 8, or `null` past 8; and `"required"`
  or `"maybe"`. The scope gives the depth, so a `read` in a `repeat`
  callback returns a handle one level deeper.
- `data.get(handle)` returns the entry's data with the handle's type, such
  as `number[][]`, or `undefined` for an absent `"maybe"` entry; add
  `{ bigint: true }` for `bigint` integers. A handle whose value type or depth
  differs from the entry's throws `TypeError`. `spec(format?, body)` returns
  `{ spec, handles, source, read, catalog }`, and its `read` maps the handles
  that `body` returned to their data. Any object with the shape of the
  `Handle` interface (`name`, `valueType`, `depth`, `presence`) works as a
  handle in both.
- Equality, `eq` and `ne`, compiles for two strings, two numbers, or an
  operand from `dynamic()`. `set(handle, value)` compiles for a value of the
  handle's value type and, where both are known, its depth.
- `forward(name, "int", { depth, optional })` reads the depth and absence
  from the literal options; options of a wider type, such as a variable
  typed `ForwardOptions`, give a handle with an open depth that may be
  absent. Options of an arm work the same way.
- Types are constants and methods: `int`, `float`, `num`, `word`, `rest`,
  `eol`, `atLeast(lo)`, `atMost(hi)`, `below(hi)`, `range(lo).below(hi)`,
  `range(lo, hi)`, `alphabet(...)`, and `.dim(n)`, `.dims(...)` and
  `.until(...)`. Expressions are methods of handles (`plus`, `lt`, `eq`,
  `at`, `get`, ...) and functions (`sum`, `count`, `all`, `any`, `distinct`,
  `div`, `rem`, `pow`, `min`, `max`, `size`, `defined`).
- Patterns are tagged templates, `` p`instance ${name}` ``, or
  `pattern(...parts)`. A capture handle becomes a capture; `ref(x)`,
  `validate(type)`, `discard()`, `blank` and `restOfLine` are the other
  parts.
- `forward(name)` and `forward(name, "int", { depth, optional })` name an
  entry before it is defined; `undefined(name)` refers to a name that is
  never defined; `forwardRecord`, `eachName`, `asName` and `recordAs`
  share record names between repeats.
- A builder check throws `BuildError` at the call that broke it, with a
  `kind` (`"scope"`, `"provenance"`, `"redefinition"`, `"binding"`,
  `"unprintable"`, or `"spec"` for text that Iris rejects) and the
  caller's `callSite`. An `IrisError` from parsing, reading or cataloging a
  built spec names the builder call of the responsible statement as
  `callSite`. Wrap a helper that writes statements for its caller in
  `trackCaller` to record the caller's line instead of the helper's.

`iris gen --lang typescript SPEC` prints typed handles for a reader spec
written as `.iris` text: a module of object literals typed as `Handle`, read
with `data.get(view.machine)` after `import * as view from "./view.ts"`. The
module imports only the `Handle` type, so it works with every entry point.
[`examples/view.sh`](examples/view.sh) shows the command and `view.out` its
output for the jobshop format.

Each format in [`formats/`](../formats/README.md) has a builder in
[`examples/`](examples/), `build-FORMAT.ts`, which exports a function that
returns the built spec and reads a case when Node runs the file.

The [design brief](../docs/builders.md#typescript) explains the choices.

## Tests

From this directory, after `npm run build`:

```sh
npm test
npm run test:types
```

`npm test` first builds the test build into the ignored `test-build/`
directory: `node scripts/build.ts --fault-injection` builds `rust/iris-js` with
its `fault-injection` feature, which adds the internal `armTrap` export, into
`test-build/wasm/` and copies `dist/` beside it. The package code is the same,
but it loads the module that can force a trap; the published module has no
such export. `npm test` then runs the `node:test` files in `test/` against the
built package.
[`test/runtimes.test.ts`](test/runtimes.test.ts) runs each fixture in
[`test/smoke/`](test/smoke/) in its own Node process: the Node entry from ESM
and from `require()`, the inline entry, and the manual entry with
`initialize()` from bytes, from a promised data URL after a failed load, and
while the inline entry installs its module before the pending bytes arrive.
Each reads the four-by-four case of
[`formats/sudoku`](../formats/sudoku/README.md) and must print the case's
expected JSON.
[`test/resilience.test.ts`](test/resilience.test.ts) reads past the memory
threshold with the published module and checks that specs parsed earlier keep
working, that memory returns to a fresh instance's size, and that the old
memory is garbage-collected; `npm test` runs Node with `--expose-gc` for it. It
also checks that only the test build's module exports `armTrap`.
[`test/traps.test.ts`](test/traps.test.ts) forces traps through the test build
and makes instantiation fail to check that a failed replacement never hides a
call's result or error. Both use the internal `instanceMemory` export of
`rust/iris-js`, which the published declarations omit.
[`test/examples.test.ts`](test/examples.test.ts) checks the examples of the
site's [TypeScript guide](https://iris.gecode.dev/using/typescript/), which
live in [`examples/`](examples/). It type-checks all of them with
[`examples/tsconfig.json`](examples/tsconfig.json) against `dist/`, and runs
each one from the repository root and compares its output with the recorded
`.out` file. Three examples are only type-checked: `browser.ts` needs a page,
`worker.ts` needs workerd, and `deadline-worker.ts` runs inside `deadline.ts`.
After changing an example, record its output from the repository root, for
example with `node typescript/examples/read.ts > typescript/examples/read.out`.
The site's build checks that the guide shows every file in `examples/`.
The four `test/build-*.test.ts` files test the spec builder: the text of
every statement and expression (`build-printing`), the builder checks of
scope, names, bindings, redefinition, captures and unprintable text
(`build-checks`), the call site that each statement and each parse, read and
catalog failure names (`build-call-sites`), and typed reads through handles,
including a jobshop builder and the jobshop example of the design brief
judged against [`formats/jobshop`](../formats/jobshop/README.md)
(`build-reading`).
[`test/format-builders.test.ts`](test/format-builders.test.ts) checks that
every format has a builder in `examples/`, and reads every case of each format
through it: it runs the Rust conformance runner, through Cargo, with
[`test/format-builder-driver.ts`](test/format-builder-driver.ts), which
answers each request with the spec of the format's builder instead of the
case's, so each case is judged, with its parameters and operation, by the
data or error it expects. It needs Cargo, and builds the runner on its first
run.
`npm run test:types` type-checks the sources, the tests,
[`test/types/api.ts`](test/types/api.ts) and
[`test/types/build.ts`](test/types/build.ts) against the published
declarations in `dist/`; each `@ts-expect-error` line in those files must
report an error. `build.ts` checks the types of handles and their data and
rejects misuse of the builder.

`npm run test:package` packs the package with `npm pack`, installs the
tarball into a temporary consumer, type-checks an import of `gecode-iris`,
`gecode-iris/inline`, `gecode-iris/manual` and the three `gecode-iris/build`
entry points with `strict` on and `skipLibCheck` off, imports each in Node,
and builds and reads a reader spec through `gecode-iris/build`, whose failed
check must name the consumer's line. It catches declarations and
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
must fail. Two more pages resolve `gecode-iris/build` to its `browser` and
`inline` exports, build a reader spec in
[`test/smoke/build-page.js`](test/smoke/build-page.js), and check that its
failed check names that file's line. Set `BROWSER_CHANNEL=chrome` to use an installed Chrome instead of
Playwright's Chromium. `npm run test:workerd` serves
[`test/smoke/workerd`](test/smoke/workerd/wrangler.jsonc) with the pinned
Wrangler's `wrangler dev`, which bundles the package with the `workerd`
condition and imports the format files as text modules. The worker reads the
case at module scope, and builds a reader spec through `gecode-iris/build`;
the test fetches the case's JSON and, from `/build`, the built spec's read
and call site. The configuration turns
Wrangler's usage metrics off.

[`test/conformance-driver.ts`](test/conformance-driver.ts) speaks the
[driver protocol](../conformance/README.md#four-driver-commands) through the
public API. From the repository root, it passes every case with no driver ID or
exemptions:

```sh
(cd rust && cargo run --locked -p gecode-iris-conformance -- run --driver "\"$(command -v node)\" \"$PWD/../typescript/test/conformance-driver.ts\"" ../conformance/cases/0.1)
(cd rust && cargo run --locked -p gecode-iris-conformance -- run --driver "\"$(command -v node)\" \"$PWD/../typescript/test/conformance-driver.ts\"" ../formats)
```

[`test/round-trip-runner.ts`](test/round-trip-runner.ts) runs the programs
that `iris gen builder --lang typescript` prints for the builder round trip
of [gecode-iris-gen](../rust/iris-gen/README.md#the-typescript-backend). The
package does not ship it. After `npm run build`, from the repository root:

```sh
(cd rust && cargo test --locked -p gecode-iris --test builder_round_trip_typescript -- --ignored --nocapture)
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
  through one synchronous step, and a replacement swaps it synchronously
  through the glue's reset function, so only one instance is ever live. Its
  declarations name no glue type, so the public declarations never import the
  glue.
- `src/backend.ts` calls the glue's exports once a module is loaded, turns
  failure envelopes into errors, replaces the instance after a trap or a large
  read, and parses specs again in a new instance. A handle from a replaced
  instance is never freed into the new one.
- `src/transfer.ts` decodes the binary layout documented in
  [`rust/iris-js/src/transfer.rs`](../rust/iris-js/src/transfer.rs).
- `src/spec.ts`, `src/data.ts` and `src/errors.ts` hold the classes.
  `src/handle.ts` declares handles and the data types they give, and
  `src/call-sites.ts` the call sites that errors carry.
- `src/build/` is the spec builder. `scope.ts` holds the scopes, their
  statements and `spec()`; `expr.ts`, `types.ts` and `pattern.ts` the
  expressions, types and patterns; `ops.ts` the expression trees;
  `statement.ts` and `print.ts` the
  statement stream and its printing; `check.ts` the binding checks;
  `site.ts` the call sites; and `error.ts` `BuildError`. `index.ts` is the
  builder's API, and `node.ts`, `browser.ts`, `workerd.ts`, `inline.ts` and
  `manual.ts` pair it with each entry point.
- `examples/` holds the TypeScript guide's examples and their recorded output.
