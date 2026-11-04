// Loads the WebAssembly module. Every loading path installs its instance
// through one synchronous step, so exactly one instance is ever installed.
// The exported declarations name no glue type, so the public declarations
// never import the glue.
import { initSync } from "../wasm/iris_js.js";

/**
 * The WebAssembly module of `gecode-iris`, as a compiled module, its bytes, a
 * fetch response, or the URL to fetch it from.
 */
export type ModuleSource =
  | WebAssembly.Module
  | ArrayBuffer
  | ArrayBufferView<ArrayBuffer>
  | Response
  | URL
  | string;

let loaded = false;
let loading: Promise<void> | undefined;

/** Whether an instance is installed. */
export function isLoaded(): boolean {
  return loaded;
}

/** Instantiates the compiled module synchronously, unless a module is loaded. */
export function instantiate(module: WebAssembly.Module): void {
  if (!loaded) {
    initSync({ module });
    loaded = true;
  }
}

/**
 * Loads the WebAssembly module from `source`. Every entry point except
 * `gecode-iris/manual` loads the module when it is imported. Once a load has
 * started or a module is loaded, later calls return the same promise and
 * ignore their source; a failed load may be retried.
 */
export function initialize(source: ModuleSource | Promise<ModuleSource>): Promise<void> {
  if (loaded) {
    return loading ?? Promise.resolve();
  }
  loading ??= compile(source)
    .then(instantiate)
    .catch((error: unknown) => {
      // An entry point may have installed a module while this load was pending.
      if (loaded) {
        return;
      }
      loading = undefined;
      throw error;
    });
  return loading;
}

async function compile(pending: ModuleSource | Promise<ModuleSource>): Promise<WebAssembly.Module> {
  const source = await pending;
  if (source instanceof WebAssembly.Module) {
    return source;
  }
  if (typeof source === "string" || source instanceof URL) {
    return compileResponse(await fetch(source));
  }
  if (source instanceof Response) {
    return compileResponse(source);
  }
  return WebAssembly.compile(source);
}

async function compileResponse(response: Response): Promise<WebAssembly.Module> {
  if (!response.ok) {
    throw new Error(
      `failed to fetch the WebAssembly module: ${response.status} ${response.statusText} fetching '${response.url}'`,
    );
  }
  // Streaming compilation requires the application/wasm MIME type.
  if (response.headers.get("Content-Type") === "application/wasm") {
    return WebAssembly.compileStreaming(response);
  }
  return WebAssembly.compile(await response.arrayBuffer());
}
