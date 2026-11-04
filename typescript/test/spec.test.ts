import assert from "node:assert/strict";
import { test } from "node:test";
import { IrisError, Spec } from "gecode-iris";

const COUNTED = "n: int  a[n]: int";
const NAMED = "param n = 1\ninstances { name: word  a[n]: int }";
const NAMED_INPUT = "first 5\nsecond 7\n";

test("parses a spec from text and from UTF-8 bytes", () => {
  const fromText = Spec.parse(COUNTED);
  const fromBytes = Spec.parse(new TextEncoder().encode(COUNTED), "counted.iris");
  assert.deepEqual(fromText.read("2 1 2").ints("a"), [1, 2]);
  assert.deepEqual(fromBytes.read("2 1 2").ints("a"), [1, 2]);
});

test("defaults the origins to <string> and <input>", () => {
  assert.throws(
    () => Spec.parse("x: nope"),
    (error: unknown) => error instanceof IrisError && error.specLocation?.origin === "<string>",
  );
  assert.throws(
    () => Spec.parse(COUNTED).read("x"),
    (error: unknown) => error instanceof IrisError && error.inputLocation?.origin === "<input>",
  );
});

test("rejects spec bytes that are not UTF-8 with a language error", () => {
  assert.throws(
    () => Spec.parse(new Uint8Array([0x6e, 0xff])),
    (error: unknown) => error instanceof IrisError && error.code === "E-SPEC-UTF8",
  );
});

test("reads input text and bytes, many times from one spec", () => {
  const spec = Spec.parse(COUNTED);
  for (let n = 0; n < 50; n++) {
    const input = `${n} ${"7 ".repeat(n)}`;
    const data = n % 2 === 0 ? spec.read(input) : spec.read(new TextEncoder().encode(input));
    assert.equal(data.size("a"), n);
  }
});

test("applies params given as an object or as pairs", () => {
  const spec = Spec.parse(NAMED);
  assert.deepEqual(
    spec.read(NAMED_INPUT, "in.txt", { params: { instance: "second" } }).ints("a"),
    [7],
  );
  const pairs = new Map([["instance", "first"]]);
  assert.deepEqual(spec.read(NAMED_INPUT, "in.txt", { params: pairs }).ints("a"), [5]);
});

test("passes pairs in caller order", () => {
  const spec = Spec.parse(NAMED);
  const unknown = (params: Iterable<readonly [string, string]>): string => {
    try {
      spec.read(NAMED_INPUT, "in.txt", { params });
    } catch (error) {
      assert.ok(error instanceof IrisError);
      return error.message;
    }
    throw new Error("read succeeded");
  };
  assert.match(
    unknown([
      ["zeta", "1"],
      ["alpha", "1"],
    ]),
    /zeta/,
  );
  assert.match(
    unknown([
      ["alpha", "1"],
      ["zeta", "1"],
    ]),
    /alpha/,
  );
});

test("lists instances with catalog", () => {
  const catalog = Spec.parse(NAMED).catalog(NAMED_INPUT);
  assert.equal(catalog.int("instances"), 2);
  assert.deepEqual(catalog.strings("name"), ["first", "second"]);
});

test("throws TypeError after disposal, and disposal is idempotent", () => {
  const spec = Spec.parse(COUNTED);
  spec[Symbol.dispose]();
  spec[Symbol.dispose]();
  assert.throws(() => spec.read("1 1"), TypeError);
  assert.throws(() => spec.catalog("1 1"), TypeError);
});

test("rejects ill-formed strings and non-string params with TypeError", () => {
  const spec = Spec.parse(COUNTED);
  assert.throws(() => Spec.parse("n: int \ud800"), TypeError);
  assert.throws(() => Spec.parse(COUNTED, "\udc00"), TypeError);
  assert.throws(() => spec.read("1 \ud800"), TypeError);
  assert.throws(() => spec.read("1 1", "\ud800"), TypeError);
  assert.throws(() => spec.read("1 1", "in.txt", { params: { n: "\ud800" } }), TypeError);
  // JavaScript callers can pass any value.
  for (const params of [{ n: 1 }, [["n"]], "n=1"]) {
    // @ts-expect-error -- deliberately mistyped
    assert.throws(() => spec.read("1 1", "in.txt", { params }), TypeError);
  }
});
