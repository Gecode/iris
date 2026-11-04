import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { Spec } from "gecode-iris";

const SPEC = `
n: int  safe[2]: int  unsafe[2]: int  bounds[2]: int
f[2]: float  s[2]: word
repeat 2 { k: int  x[k]: int }
e[2][0]: int
`;
const INPUT = `
3
9007199254740991 -9007199254740991
9007199254740992 -9007199254740992
-9223372036854775808 9223372036854775807
0.5 1e300
hé ✓
2 1 2
0
`;
const data = Spec.parse(SPEC).read(INPUT);
const MIN_I64 = -(2n ** 63n);
const MAX_I64 = 2n ** 63n - 1n;

test("lists entries in definition order with value type and depth", () => {
  assert.deepEqual(
    data.entries.map(({ name, valueType, depth }) => `${name}:${valueType}:${depth}`),
    [
      "n:int:0",
      "safe:int:1",
      "unsafe:int:1",
      "bounds:int:1",
      "f:float:1",
      "s:string:1",
      "k:int:1",
      "x:int:2",
      "e:int:2",
    ],
  );
  assert.ok(Object.isFrozen(data.entries));
  assert.ok(Object.isFrozen(data.entries[0]));
  assert.deepEqual(data.entry("x"), { name: "x", valueType: "int", depth: 2 });
  assert.equal(data.entry("missing"), undefined);
  assert.equal(data.has("x"), true);
  assert.equal(data.has("missing"), false);
});

test("reads scalars and rows", () => {
  assert.equal(data.int("n"), 3);
  assert.equal(data.bigInt("n"), 3n);
  assert.equal(data.float("f", [1]), 1e300);
  assert.equal(data.string("s", [0]), "hé");
  assert.equal(data.int("x", [0, 1]), 2);
  assert.deepEqual(data.floats("f"), [0.5, 1e300]);
  assert.deepEqual(data.strings("s"), ["hé", "✓"]);
  assert.deepEqual(data.ints("x", [0]), [1, 2]);
  assert.deepEqual(data.ints("x", [1]), []);
});

test("reports sizes of arrays and ragged rows", () => {
  assert.equal(data.size("x"), 2);
  assert.equal(data.size("x", [0]), 2);
  assert.equal(data.size("x", [1]), 0);
  assert.equal(data.size("e", [1]), 0);
});

test("copies values, including ragged and empty arrays", () => {
  assert.deepEqual(data.value("x"), [[1, 2], []]);
  assert.deepEqual(data.value("e"), [[], []]);
  assert.deepEqual(data.value("x", [0]), [1, 2]);
  assert.equal(data.value("n"), 3);
  const row = data.ints("x", [0]);
  row[0] = 99;
  assert.deepEqual(data.ints("x", [0]), [1, 2]);
  const object = Spec.parse("n: int  a[n]: float  w: word").read("2 0.5 -1 hi").toObject();
  assert.deepEqual(object, { n: 2, a: [0.5, -1], w: "hi" });
  assert.deepEqual(Object.keys(object), ["n", "a", "w"]);
});

test("keeps a leading U+FEFF in strings", () => {
  const scalar = Spec.parse("w: word").read("\uFEFFhello");
  assert.equal(scalar.string("w"), "\uFEFFhello");
  assert.equal(scalar.value("w"), "\uFEFFhello");
  assert.deepEqual(scalar.toObject(), { w: "\uFEFFhello" });
  const row = Spec.parse("s[2]: word").read("\uFEFFa \uFEFFb");
  assert.deepEqual(row.strings("s"), ["\uFEFFa", "\uFEFFb"]);
  assert.deepEqual(row.value("s"), ["\uFEFFa", "\uFEFFb"]);
});

test("copies values deeper than the call stack", () => {
  const depth = 16384;
  const deep = Spec.parse(`a${"[1]".repeat(depth)}: int`).read("7");
  for (const value of [deep.value("a"), deep.value("a", [], { bigint: true }), deep.toObject().a]) {
    let level = 0;
    let current = value;
    while (Array.isArray(current)) {
      assert.equal(current.length, 1);
      current = current[0];
      level++;
    }
    assert.equal(level, depth);
    assert.ok(current === 7 || current === 7n);
  }
  assert.equal(
    deep
      .value(
        "a",
        Array.from({ length: depth - 1 }, () => 0),
      )
      .toString(),
    "7",
  );
});

test("returns number only within the safe integer range", () => {
  assert.deepEqual(data.ints("safe"), [Number.MAX_SAFE_INTEGER, Number.MIN_SAFE_INTEGER]);
  assert.equal(data.int("safe", [0]), Number.MAX_SAFE_INTEGER);
  assert.throws(() => data.int("unsafe", [0]), RangeError);
  assert.throws(() => data.int("unsafe", [1]), RangeError);
  assert.throws(() => data.ints("unsafe"), RangeError);
  assert.throws(() => data.value("unsafe"), RangeError);
  assert.throws(() => data.toObject(), RangeError);
});

test("returns exact bigint values at the 64-bit bounds", () => {
  assert.equal(data.bigInt("bounds", [0]), MIN_I64);
  assert.equal(data.bigInt("bounds", [1]), MAX_I64);
  const row = data.bigInts("bounds");
  assert.ok(row instanceof BigInt64Array);
  assert.deepEqual([...row], [MIN_I64, MAX_I64]);
  row[0] = 0n;
  assert.equal(data.bigInt("bounds", [0]), MIN_I64);
  assert.deepEqual(data.value("bounds", [], { bigint: true }), [MIN_I64, MAX_I64]);
  assert.deepEqual(data.value("unsafe", undefined, { bigint: true }), [2n ** 53n, -(2n ** 53n)]);
  assert.deepEqual(data.value("f", [], { bigint: true }), [0.5, 1e300]);
  assert.equal(data.toObject({ bigint: true })["n"], 3n);
});

test("throws RangeError for unknown entries and indices out of bounds", () => {
  assert.throws(() => data.int("missing"), {
    name: "RangeError",
    message: "unknown entry 'missing'",
  });
  assert.throws(() => data.ints("x", [2]), {
    name: "RangeError",
    message: "entry 'x' at path [2]: path index out of bounds",
  });
  assert.throws(() => data.int("x", [1, 0]), RangeError);
  assert.throws(() => data.value("x", [-1]), RangeError);
});

test("throws TypeError for the wrong value type or depth", () => {
  assert.throws(() => data.int("x"), {
    name: "TypeError",
    message: "entry 'x': expected a scalar",
  });
  assert.throws(() => data.ints("n"), {
    name: "TypeError",
    message: "entry 'n': expected an array",
  });
  assert.throws(() => data.ints("x"), TypeError);
  assert.throws(() => data.float("n"), {
    name: "TypeError",
    message: "entry 'n': expected float leaves",
  });
  assert.throws(() => data.strings("f"), TypeError);
  assert.throws(() => data.size("n"), TypeError);
  assert.throws(() => data.value("x", [0.5]), TypeError);
  assert.throws(() => data.int("n", [0]), TypeError);
});

test("renders the canonical JSON and dzn bytes", () => {
  const format = new URL("../../formats/sudoku/cases/four-by-four/", import.meta.url);
  const sudoku = Spec.parse(readFileSync(new URL("spec.iris", format)), "spec.iris").read(
    readFileSync(new URL("input.txt", format)),
    "input.txt",
  );
  assert.equal(sudoku.toJSON(), readFileSync(new URL("expected.json", format), "utf8"));
  const ragged = new URL("../../conformance/cases/0.1/dzn/output-1/", import.meta.url);
  const dzn = Spec.parse(readFileSync(new URL("spec.iris", ragged))).read(
    readFileSync(new URL("input.txt", ragged)),
  );
  assert.equal(dzn.toDzn(), readFileSync(new URL("expected.dzn", ragged), "utf8"));
  assert.match(data.toJSON(), /"value":\[-9223372036854775808,9223372036854775807\]/);
});
