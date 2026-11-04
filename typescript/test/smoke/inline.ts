// Imports gecode-iris/inline and reads SPEC and INPUT at module scope,
// printing the canonical JSON.
import { readFileSync } from "node:fs";
import { Spec } from "gecode-iris/inline";

const [specPath, inputPath] = process.argv.slice(2);
if (specPath === undefined || inputPath === undefined) {
  throw new Error("usage: inline.ts SPEC INPUT");
}
const spec = Spec.parse(readFileSync(specPath), specPath);
process.stdout.write(spec.read(readFileSync(inputPath), inputPath).toJSON());
