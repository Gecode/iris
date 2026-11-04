// Imports gecode-iris/manual, which loads nothing until initialize() is
// awaited, then reads SPEC and INPUT, printing the canonical JSON.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { initialize, Spec } from "gecode-iris/manual";

const [specPath, inputPath] = process.argv.slice(2);
if (specPath === undefined || inputPath === undefined) {
  throw new Error("usage: manual.ts SPEC INPUT");
}
assert.throws(() => Spec.parse(readFileSync(specPath)), /await initialize\(\) first/);
await initialize(readFileSync(new URL("../../wasm/iris_js_bg.wasm", import.meta.url)));
const spec = Spec.parse(readFileSync(specPath), specPath);
process.stdout.write(spec.read(readFileSync(inputPath), inputPath).toJSON());
