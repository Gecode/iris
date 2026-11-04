// The gecode-iris/inline entry point. The module's bytes are embedded as
// base64, so loading needs no file access, fetch or bundler support, and the
// synchronous compilation lets Spec.parse run at module scope.
import moduleBase64 from "../wasm/iris_js_inline.js";
import { instantiate } from "./loading.js";

instantiate(new WebAssembly.Module(decodeBase64(moduleBase64)));

export * from "./index.js";

function decodeBase64(text: string): Uint8Array<ArrayBuffer> {
  const binary = atob(text);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}
