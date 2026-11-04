// The browser entry point, and the default for runtimes without a specific
// condition. Top-level await fetches and instantiates the module before any
// importing module evaluates, so Spec.parse can run at module scope. Vite and
// webpack recognize the URL pattern and emit the module as an asset.
import { initialize } from "./loading.js";

await initialize(new URL("../wasm/iris_js_bg.wasm", import.meta.url));

export * from "./index.js";
