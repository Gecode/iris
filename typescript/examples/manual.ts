import { readFile } from "node:fs/promises";
import { initialize, Spec } from "gecode-iris/manual";

const wasm = new URL("wasm/iris_js_bg.wasm", import.meta.resolve("gecode-iris/package.json"));
await initialize(readFile(wasm));

const spec = Spec.parse("n:int row[n]:int", "example.iris");
console.log(spec.read("3 10 20 30", "input.txt").ints("row"));
