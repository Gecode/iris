// Starts initialize() from gecode-iris/manual with pending bytes, imports
// gecode-iris/inline, which installs its module synchronously, and parses a
// spec before the bytes arrive. The pending load must not replace the
// installed instance, so the spec keeps reading and frees cleanly. Prints the
// canonical JSON. Run with --expose-gc.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { initialize, Spec } from "gecode-iris/manual";

const [specPath, inputPath] = process.argv.slice(2);
if (specPath === undefined || inputPath === undefined) {
  throw new Error("usage: initialize-race.ts SPEC INPUT");
}
let deliverBytes: (bytes: Uint8Array<ArrayBuffer>) => void = () => {};
const bytes = new Promise<Uint8Array<ArrayBuffer>>((resolve) => {
  deliverBytes = resolve;
});
const pending = initialize(bytes);
await import("gecode-iris/inline");
const spec = Spec.parse(readFileSync(specPath), specPath);
// A spec that is only garbage-collected, freed by the finalizer.
Spec.parse(readFileSync(specPath), specPath);
deliverBytes(readFileSync(new URL("../../wasm/iris_js_bg.wasm", import.meta.url)));
await pending;
assert.equal(initialize(new Uint8Array()), pending);
globalThis.gc?.();
await new Promise((resolve) => setTimeout(resolve, 10));
process.stdout.write(spec.read(readFileSync(inputPath), inputPath).toJSON());
