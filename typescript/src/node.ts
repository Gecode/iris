// The Node entry point. It compiles and instantiates the module synchronously,
// with no top-level await, so require() works and Spec.parse can run as soon
// as an importing module evaluates.
import { readFileSync } from "node:fs";
import { instantiate } from "./loading.js";

instantiate(
  new WebAssembly.Module(readFileSync(new URL("../wasm/iris_js_bg.wasm", import.meta.url))),
);

export * from "./index.js";
