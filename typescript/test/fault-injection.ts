// The test build that npm test generates with scripts/build.ts
// --fault-injection: a copy of dist/ that loads the module built with
// iris-js's fault-injection feature. Its code is the package's, so its exports
// have the package's types; armTrap is the one export the published module
// lacks. Each test file runs in its own process, so a file that imports this
// uses only the test build's instance.
import assert from "node:assert/strict";

type Package = typeof import("gecode-iris");
type Glue = typeof import("../wasm/iris_js.js") & { armTrap(): void };
type Backend = typeof import("../dist/backend.js");

const testBuild = new URL("../test-build/", import.meta.url);

export const iris: Package = await load<Package>("dist/node.js", "Spec");
export const glue: Glue = await load<Glue>("wasm/iris_js.js", "armTrap");
export const MEMORY_THRESHOLD: number = (await load<Backend>("dist/backend.js", "MEMORY_THRESHOLD"))
  .MEMORY_THRESHOLD;

/** Imports a module of the test build, which must export `name`. */
async function load<T>(path: string, name: keyof T & string): Promise<T> {
  const module: unknown = await import(new URL(path, testBuild).href);
  if (!hasExport<T>(module, name)) {
    throw new Error(`test-build/${path} does not export ${name}; run npm test to build it`);
  }
  return module;
}

function hasExport<T>(module: unknown, name: keyof T & string): module is T {
  return typeof module === "object" && module !== null && name in module;
}

/** The memory of the test build's live instance. */
export function activeMemory(): WebAssembly.Memory {
  const memory: unknown = glue.instanceMemory();
  assert.ok(memory instanceof WebAssembly.Memory);
  return memory;
}
