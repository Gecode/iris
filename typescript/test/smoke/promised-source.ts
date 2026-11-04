// Initializes gecode-iris/manual from promised sources: a rejected promise and
// a module that compiles but fails to instantiate, each of which must allow a
// retry, then a promise of the module as a data URL. Reads SPEC and INPUT,
// printing the canonical JSON.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { initialize, Spec } from "gecode-iris/manual";

const [specPath, inputPath] = process.argv.slice(2);
if (specPath === undefined || inputPath === undefined) {
  throw new Error("usage: promised-source.ts SPEC INPUT");
}
await assert.rejects(initialize(Promise.reject(new Error("no module"))), /no module/);
// A valid module whose one import, x.y, the glue does not provide.
const unlinkable = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x04, 0x01, 0x60, 0x00, 0x00, 0x02, 0x07,
  0x01, 0x01, 0x78, 0x01, 0x79, 0x00, 0x00,
]);
await assert.rejects(initialize(unlinkable));
const module = readFileSync(new URL("../../wasm/iris_js_bg.wasm", import.meta.url));
await initialize(Promise.resolve(`data:application/wasm;base64,${module.toString("base64")}`));
const spec = Spec.parse(readFileSync(specPath), specPath);
process.stdout.write(spec.read(readFileSync(inputPath), inputPath).toJSON());
