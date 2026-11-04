// Instance replacement after a trap, and after a replacement that fails. The
// tests force traps through the test build (see fault-injection.ts).
import assert from "node:assert/strict";
import { test, type TestContext } from "node:test";
import type * as Iris from "gecode-iris";
import { activeMemory, glue, iris, MEMORY_THRESHOLD } from "./fault-injection.ts";

const { IrisError, IrisInternalError, Spec } = iris;
const COUNTED = "n: int  a[n]: int";
const PAGE_BYTES = 64 * 1024;
const freshBytes = activeMemory().buffer.byteLength;

test("a trap throws IrisInternalError, and a spec parsed before it still reads", () => {
  const spec = Spec.parse(new TextEncoder().encode(COUNTED), "counted.iris");
  const before = activeMemory();
  glue.armTrap();
  assert.throws(
    () => spec.read("2 1 2"),
    (error: unknown) =>
      error instanceof IrisInternalError &&
      !(error instanceof IrisError) &&
      error.name === "IrisInternalError" &&
      error.cause instanceof WebAssembly.RuntimeError,
  );
  assert.notEqual(activeMemory(), before);
  assert.deepEqual(spec.read("2 1 2").ints("a"), [1, 2]);
  assert.throws(
    () => spec.read("x", "bad.txt"),
    (error: unknown) => error instanceof IrisError && error.specLocation?.origin === "counted.iris",
  );
  spec[Symbol.dispose]();
});

test("a spec parsed from a Buffer parses its own copy again after a trap", () => {
  const text = Buffer.from(COUNTED);
  const spec = Spec.parse(text);
  text.fill("!");
  glue.armTrap();
  assert.throws(() => spec.read("1 1"), IrisInternalError);
  assert.deepEqual(spec.read("1 3").ints("a"), [3]);
});

test("a stale spec disposes without freeing into the new instance", (context) => {
  const free = context.mock.method(glue.SpecHandle.prototype, "free");
  const stale = Spec.parse(COUNTED);
  glue.armTrap();
  assert.throws(() => stale.read("1 1"), IrisInternalError);
  stale[Symbol.dispose]();
  assert.equal(free.mock.callCount(), 0);
  assert.throws(() => stale.read("1 1"), TypeError);
  const current = Spec.parse(COUNTED);
  assert.deepEqual(current.read("1 4").ints("a"), [4]);
  current[Symbol.dispose]();
  assert.equal(free.mock.callCount(), 1);
});

test("a failed replacement after a read keeps the read's result", (context) => {
  const spec = Spec.parse(COUNTED);
  growPastThreshold();
  const instantiation = failInstantiation(context);
  assert.deepEqual(spec.read("1 5").ints("a"), [5]);
  instantiation.mock.restore();
  assertRecovered(spec);
});

test("a failed replacement after a read keeps the read's IrisError", (context) => {
  const spec = Spec.parse(COUNTED);
  growPastThreshold();
  const instantiation = failInstantiation(context);
  assert.throws(() => spec.read("x"), IrisError);
  instantiation.mock.restore();
  assertRecovered(spec);
});

test("a failed replacement after a trap keeps the IrisInternalError", (context) => {
  const spec = Spec.parse(COUNTED);
  growPastThreshold();
  const instantiation = failInstantiation(context);
  glue.armTrap();
  assert.throws(
    () => spec.read("1 1"),
    (error: unknown) =>
      error instanceof IrisInternalError && error.cause instanceof WebAssembly.RuntimeError,
  );
  instantiation.mock.restore();
  assertRecovered(spec);
});

test("a pending replacement is retried before the next call, which reports its failure", (context) => {
  const spec = Spec.parse(COUNTED);
  glue.armTrap();
  const instantiation = failInstantiation(context);
  assert.throws(() => spec.read("1 1"), IrisInternalError);
  assert.throws(() => spec.read("1 1"), /no instance for the test/);
  assert.throws(() => Spec.parse(COUNTED), /no instance for the test/);
  instantiation.mock.restore();
  assertRecovered(spec);
});

/** Checks that the next call replaces the instance and that `spec` reads in it. */
function assertRecovered(spec: Iris.Spec): void {
  assert.deepEqual(spec.read("1 6").ints("a"), [6]);
  assert.equal(activeMemory().buffer.byteLength, freshBytes);
}

/** Grows the live instance's memory past the threshold without a read. */
function growPastThreshold(): void {
  const memory = activeMemory();
  memory.grow(Math.floor((MEMORY_THRESHOLD - memory.buffer.byteLength) / PAGE_BYTES) + 1);
  assert.ok(memory.buffer.byteLength > MEMORY_THRESHOLD);
}

/** Makes every instantiation throw until the test ends or the mock is restored. */
function failInstantiation(context: TestContext) {
  return context.mock.method(WebAssembly, "Instance", () => {
    throw new Error("no instance for the test");
  });
}
