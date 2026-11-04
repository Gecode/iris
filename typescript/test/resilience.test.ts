// Instance replacement after a large read, with the published module. The
// test reaches the internal glue export instanceMemory, which the published
// declarations omit; the glue is the same module the package loaded. Traps are
// tested with the test build in traps.test.ts.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { setImmediate } from "node:timers/promises";
import { Spec } from "gecode-iris";
import { MEMORY_THRESHOLD } from "../dist/backend.js";
import { instanceMemory } from "../wasm/iris_js.js";

const COUNTED = "n: int  a[n]: int";

function activeMemory(): WebAssembly.Memory {
  const memory: unknown = instanceMemory();
  assert.ok(memory instanceof WebAssembly.Memory);
  return memory;
}

const freshBytes = activeMemory().buffer.byteLength;

/** An input of `n` sevens whose read grows memory by about 25 bytes per value. */
function counted(n: number): string {
  return `${n} ${"7 ".repeat(n)}`;
}

/** An input that leaves memory past the threshold after a read. */
const LARGE = counted(Math.ceil(MEMORY_THRESHOLD / 16));

test("only the test build's module can force a trap", () => {
  assert.ok(!exportNames("../wasm").includes("armTrap"));
  assert.ok(exportNames("../test-build/wasm").includes("armTrap"));
});

test("a read past the memory threshold leaves a fresh instance", () => {
  const spec = Spec.parse(COUNTED);
  assert.equal(spec.read(LARGE).size("a"), Math.ceil(MEMORY_THRESHOLD / 16));
  assert.equal(activeMemory().buffer.byteLength, freshBytes);
  assert.deepEqual(spec.read("3 1 2 3").ints("a"), [1, 2, 3]);
});

test("a small read keeps the instance", () => {
  const spec = Spec.parse(COUNTED);
  const before = activeMemory();
  spec.read(counted(1000));
  assert.equal(activeMemory(), before);
});

test("the old memory is collected after a replacement", async () => {
  const gc = globalThis.gc;
  assert.ok(gc !== undefined, "run under node --expose-gc");
  const spec = Spec.parse(COUNTED);
  const old = new WeakRef(activeMemory());
  spec.read(LARGE);
  await collect(old, gc, 10);
  assert.equal(old.deref(), undefined);
  assert.deepEqual(spec.read("1 9").ints("a"), [9]);
});

/** The export names of the module in `directory`, relative to this file. */
function exportNames(directory: string): string[] {
  const bytes = readFileSync(new URL(`${directory}/iris_js_bg.wasm`, import.meta.url));
  return WebAssembly.Module.exports(new WebAssembly.Module(bytes)).map((entry) => entry.name);
}

/** Collects garbage until `target` is cleared, in at most `rounds` rounds. */
async function collect(target: WeakRef<object>, gc: () => void, rounds: number): Promise<void> {
  if (rounds === 0 || target.deref() === undefined) {
    return;
  }
  // A WeakRef target stays alive until the current job ends.
  await setImmediate();
  gc();
  await collect(target, gc, rounds - 1);
}
