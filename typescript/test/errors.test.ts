import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { IrisError, Spec } from "gecode-iris";

function irisError(operation: () => unknown): IrisError {
  try {
    operation();
  } catch (error) {
    assert.ok(error instanceof IrisError, `expected IrisError, got ${String(error)}`);
    return error;
  }
  throw new Error("expected an IrisError");
}

test("a spec error has a spec location and no input location", () => {
  const error = irisError(() => Spec.parse("n: int\nx: nope", "bad.iris"));
  assert.ok(error instanceof Error);
  assert.equal(error.name, "IrisError");
  assert.equal(error.code, "E-SPEC-UNKNOWN-TYPE");
  assert.equal(error.message, "unknown type 'nope'");
  assert.deepEqual(error.specLocation, { origin: "bad.iris", line: 2, column: null });
  assert.equal(error.inputLocation, null);
  assert.equal(error.context, null);
  assert.equal(error.recordPath, null);
  assert.equal(error.diagnostic, "bad.iris:2: unknown type 'nope'");
  assert.deepEqual(JSON.parse(error.toJSON()), {
    code: "E-SPEC-UNKNOWN-TYPE",
    message: "unknown type 'nope'",
    input_location: null,
    spec_location: { origin: "bad.iris", line: 2, column: null },
    context: null,
    record_path: null,
  });
});

test("a failed check over records has both locations, context and record path", () => {
  const directory = new URL(
    "../../conformance/cases/0.1/params-instances/skipped-instance-records-2/",
    import.meta.url,
  );
  const spec = Spec.parse(readFileSync(new URL("spec.iris", directory)), "spec.iris");
  const error = irisError(() =>
    spec.read(readFileSync(new URL("input.txt", directory)), "input.txt"),
  );
  assert.equal(error.code, "E-CHECK-FAILED");
  assert.deepEqual(error.inputLocation, { origin: "input.txt", line: 1, column: 5 });
  assert.deepEqual(error.specLocation, { origin: "spec.iris", line: 1, column: null });
  assert.equal(error.context, "instance [1]");
  assert.deepEqual(error.recordPath, [1]);
  assert.ok(Object.isFrozen(error.recordPath));
  assert.equal(error.toJSON(), readFileSync(new URL("error.json", directory), "utf8").trimEnd());
  assert.equal(
    error.diagnostic,
    [
      "input.txt:1:5: check failed for record 2 of 'x': positive",
      "     1 | 2 1 0 a",
      "       |     ^",
      "while reading: instance [1]",
      "spec: spec.iris:1",
    ].join("\n"),
  );
});

test("exhausting a work budget is an IrisError with locations", () => {
  assert.deepEqual(Spec.parse("repeat 1048576 {}").read("").entries, []);
  const error = irisError(() => Spec.parse("repeat 1048577 {}").read(""));
  assert.equal(error.code, "E-BUDGET");
  assert.deepEqual(error.inputLocation, { origin: "<input>", line: 1, column: 1 });
  assert.deepEqual(error.specLocation, { origin: "<string>", line: 1, column: null });
});
