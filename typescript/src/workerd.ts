// The entry point for workerd and edge runtimes, which compile a statically
// imported .wasm file into a WebAssembly.Module and forbid compiling bytes at
// run time. Instantiation is synchronous, so Spec.parse can run at module scope.
import module from "../wasm/iris_js_bg.wasm";
import { instantiate } from "./loading.js";

instantiate(module);

export * from "./index.js";
