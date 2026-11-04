// Loads gecode-iris with require() and reads SPEC and INPUT at module scope,
// printing the canonical JSON.
const { readFileSync } = require("node:fs");
const { Spec } = require("gecode-iris");

const [specPath, inputPath] = process.argv.slice(2);
if (specPath === undefined || inputPath === undefined) {
  throw new Error("usage: require.cjs SPEC INPUT");
}
const spec = Spec.parse(readFileSync(specPath), specPath);
process.stdout.write(spec.read(readFileSync(inputPath), inputPath).toJSON());
