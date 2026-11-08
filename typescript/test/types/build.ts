// Type tests for the spec builder's declarations, checked by `npm run
// test:types` and never run. Each @ts-expect-error line must report an error.
import type { BigIntValue, Data, Value } from "gecode-iris";
import {
  atLeast,
  bind,
  discard,
  float,
  floatLit,
  int,
  lit,
  num,
  p,
  pattern,
  rest,
  spec,
  SpecBuilder,
  sum,
  validate,
  word,
  type ArmOptions,
  type DataOf,
  type DynField,
  type Expr,
  type Field,
  type ForwardOptions,
  type Handle,
  type NestedScope,
  type Scope,
  type Type,
  type BigIntViewOf,
  type ViewOf,
} from "gecode-iris/build";
import { spec as inlineSpec } from "gecode-iris/build/inline";
import { initialize, spec as manualSpec } from "gecode-iris/build/manual";

/** True when each of `A` and `B` is assignable to the other. */
type Equal<A, B> = [A] extends [B] ? ([B] extends [A] ? true : false) : false;

/** Compiles only when `A` and `B` are the same type. */
function same<A, B>(check: Equal<A, B>): void {
  void check;
}

export function handleTypes(): void {
  const b = new SpecBuilder();
  const n = b.read("n", int);
  same<typeof n, Field<"int", 0, "required">>(true);
  const grid = b.read("grid", float.dims(2, n));
  same<typeof grid, Field<"float", 2, "required">>(true);
  const sizes: number[] = [1, 2, 3];
  const deep = b.read("deep", int.dims(...sizes));
  same<typeof deep, Field<"int", null, "required">>(true);
  const someSizes: [] | [number] = sizes.length > 0 ? [2] : [];
  const oneOrTwo = b.read("one_or_two", int.dims(...someSizes));
  same<typeof oneOrTwo, Field<"int", null, "required">>(true);
  const list = b.list("list", word.until("end"));
  same<typeof list, Field<"string", 1, "required">>(true);
  const rows = b.repeat(n, (r) => {
    same<typeof r, NestedScope>(true);
    const cells = r.read("cells", num.dim(3));
    same<typeof cells, Field<"num", 2, "required">>(true);
    return r.read("row", int);
  });
  same<typeof rows, Field<"int", 1, "required">>(true);
  const maybe = b.if(n, (t) => {
    same<typeof t, Scope<0, "maybe">>(true);
    return t.read("maybe", rest);
  }).result;
  same<typeof maybe, Field<"string", 0, "maybe">>(true);
  const both = b
    .if(n, (t) => t.read("a", int))
    .elseIf(n, (t) => t.read("b", float))
    .else((t) => t.read("c", word));
  same<
    typeof both,
    [[Field<"int", 0, "maybe">, Field<"float", 0, "maybe">], Field<"string", 0, "maybe">]
  >(true);
  const [, branch] = b.if(n, () => {}).elseReadIf("if", int);
  same<typeof branch, Field<"int", 0, "maybe">>(true);
  const arm = b.sections((arms) => arms.arm("x", { optional: true }, (s) => s.read("x", int)));
  same<typeof arm, Field<"int", 0, "maybe">>(true);
  const lastArm = b.sections((arms) => arms.arm("y", { last: true }, (s) => s.read("y", int)));
  same<typeof lastArm, Field<"int", 0, "required">>(true);
  const armOptions: ArmOptions = { last: true };
  const wideArm = b.sections((arms) => arms.arm("z", armOptions, (s) => s.read("z", int)));
  same<typeof wideArm, Field<"int", 0, "maybe">>(true);

  const total = b.let("total", n.plus(1));
  same<typeof total, Field<"int", 0, "required">>(true);
  const copy = b.let("copy", grid);
  same<typeof copy, Field<"float", 2, "required">>(true);
  const element = b.let("element", grid.at(0));
  same<typeof element, Field<"float", null, "required">>(true);
  const text = b.let("text", "words");
  same<typeof text, Field<"string", 0, "required">>(true);
  const loose = b.let(n, floatLit(1.5));
  same<typeof loose, DynField>(true);
  b.read(discard(), int);
  const param = b.param("p");
  same<typeof param, Field<"dyn", null, "required">>(true);
  const withDefault = b.param("q", 3);
  same<typeof withDefault, Field<"int", 0, "required">>(true);

  const later = b.forward("later");
  same<typeof later, DynField>(true);
  const typed = b.forward("typed", "int", { depth: 2, optional: true });
  same<typeof typed, Field<"int", 2, "maybe">>(true);
  const scalar = b.forward("scalar", "string");
  same<typeof scalar, Field<"string", 0, "required">>(true);
  const settled = b.forward("settled", "int", { depth: 1, optional: false });
  same<typeof settled, Field<"int", 1, "required">>(true);
  const wide: ForwardOptions = { optional: true };
  const unsure = b.forward("unsure", "int", wide);
  same<typeof unsure, Field<"int", null, "maybe">>(true);
  const flag: boolean = sizes.length > 0;
  const flagged = b.forward("flagged", "int", { optional: flag });
  same<typeof flagged, Field<"int", 0, "maybe">>(true);
  const depth: 0 | null = sizes.length > 0 ? null : 0;
  const either = b.forward("either", "int", { depth });
  same<typeof either, Field<"int", null, "required">>(true);
  const choice = sizes.length > 0 ? { depth: 1 } : { optional: true };
  const chosen = b.forward("chosen", "int", choice);
  same<typeof chosen, Field<"int", null, "maybe">>(true);
  const capture = b.capture("c", atLeast(1));
  same<typeof capture, Field<"int", 0, "required">>(true);
  const captured = b.capture("w");
  same<typeof captured, Field<"string", 0, "required">>(true);
  const optional = b.optionalCapture("o", float);
  same<typeof optional, Field<"float", 0, "maybe">>(true);
  const named = b.defineType("t", atLeast(0));
  same<typeof named, Type<"int", 0>>(true);
  const handle: Handle<"int", 0, "required"> = n;
  void handle;
}

export function dataTypes(data: Data): void {
  const b = new SpecBuilder();
  const n = b.read("n", int);
  const grid = b.read("grid", float.dims(2, 2));
  const names = b.list("names", word.until("end"));
  const mixed = b.read("mixed", num.dim(2));
  const maybe = b.if(n, (t) => t.read("maybe", int)).result;
  const sizes: number[] = [1];
  const open = b.read("open", int.dims(...sizes));
  const loose = b.forward("loose");
  const count = data.get(n);
  same<typeof count, number>(true);
  const values = data.get(grid);
  same<typeof values, number[][]>(true);
  const words = data.get(names);
  same<typeof words, string[]>(true);
  const numbers = data.get(mixed);
  same<typeof numbers, number[]>(true);
  const absent = data.get(maybe);
  same<typeof absent, number | undefined>(true);
  const any = data.get(open);
  same<typeof any, Value>(true);
  const dynamic = data.get(loose);
  same<typeof dynamic, Value | undefined>(true);
  const big = data.get(n, { bigint: true });
  same<typeof big, bigint>(true);
  const bigMixed = data.get(mixed, { bigint: true });
  same<typeof bigMixed, (bigint | number)[]>(true);
  const bigAny = data.get(loose, { bigint: true });
  same<typeof bigAny, BigIntValue | undefined>(true);
  same<ViewOf<typeof grid>, number[][]>(true);
  same<ViewOf<Handle<"int", 0 | null, "required">>, Value>(true);
  same<BigIntViewOf<Handle<"int", 0 | null, "required">>, BigIntValue>(true);
  same<ViewOf<Handle<"int", 0 | 1, "required">>, number | number[]>(true);
  same<BigIntViewOf<Handle<"string", 1 | 2, "maybe">>, string[] | string[][] | undefined>(true);
  same<ViewOf<Handle>, Value | undefined>(true);
  void [count, values, words, numbers, absent, any, dynamic, big, bigMixed, bigAny];
  // @ts-expect-error an optional entry may be absent
  const required: number = data.get(maybe);
  // @ts-expect-error the entry holds strings
  const notNumbers: number[] = data.get(names);
  void [required, notNumbers];
}

export function builtSpecs(input: string): void {
  const built = spec("pair", (b) => {
    const n = b.read("n", int);
    return { n, values: b.read("values", float.dim(n)), note: "kept out" };
  });
  const data = built.read(input);
  same<typeof data, { n: number; values: number[] }>(true);
  same<DataOf<Field<"int", 1, "maybe">>, number[] | undefined>(true);
  const single: string = spec((b) => b.read("x", word)).read(input);
  const inline: number = inlineSpec((b) => b.read("x", int)).read(input);
  const manual: number = manualSpec((b) => b.read("x", int)).read(input);
  void [single, inline, manual, initialize];
  const handle: Handle<"int", 0, "required"> = {
    name: "n",
    valueType: "int",
    depth: 0,
    presence: "required",
  };
  const structural: number = spec(() => handle).read(input);
  const wrapped = spec(() => ({ n: handle })).read(input);
  same<typeof wrapped, { n: number }>(true);
  void structural;
}

export function expressions(): void {
  const b = new SpecBuilder();
  const x = b.read("x", int);
  const f = b.read("f", float);
  const s = b.read("s", word);
  const a = b.read("a", int.dim(3));
  const compared: Expr<"int"> = f.lt(floatLit(1.5));
  const equal: Expr<"int"> = s.eq("a");
  const mixed: Expr<"int"> = x.eq(f).and(x.ne(s.dynamic())).and(s.dynamic().eq(1));
  const rows = b.repeat(x, (r) => {
    const cell = r.read("cell", int);
    r.set(cell, 1);
    return cell;
  });
  const sizes: number[] = [1, 2];
  const open = b.read("open", int.dims(...sizes));
  same<typeof open, Field<"int", null, "required">>(true);
  b.set(open, 1);
  b.set(rows, rows);
  b.set(b.forward("any"), a);
  const total: Expr<"int"> = sum(a, (value) => value.times(2));
  const dynamic: Expr<"int"> = s.dynamic().plus(1);
  b.line(p`x = ${b.capture("y")} ${bind(b.forward("z", "string"), word)}`);
  b.line(pattern("v ", validate(atLeast(x))));
  void [compared, equal, mixed, total, dynamic];
}

export function misuse(): void {
  const b = new SpecBuilder();
  const x = b.read("x", int);
  const s = b.read("s", word);
  const f = b.read("f", float);
  const a = b.read("a", int.dim(2));
  // @ts-expect-error a list reads scalars
  int.dim(2).until(0);
  // @ts-expect-error a float is not an int
  b.set(x, floatLit(1.5));
  // @ts-expect-error a string is not an int
  b.set(x, "text");
  // @ts-expect-error an array keeps its depth
  b.set(a, 1);
  // @ts-expect-error a scalar keeps its depth
  b.set(x, a);
  const either: Expr<"int" | "string"> = lit(s.name.length > 0 ? 1 : "a");
  // @ts-expect-error an int is not compared with a string
  x.eq(s);
  // @ts-expect-error a string is not compared with an int
  s.ne(x);
  // @ts-expect-error a float is not compared with a string
  f.eq("a");
  // @ts-expect-error a value of either type is compared only with a dynamic value
  either.eq(1);
  either.eq(x.dynamic());
  // @ts-expect-error arithmetic takes ints
  s.plus(1);
  // @ts-expect-error arithmetic takes ints
  f.times(2);
  // @ts-expect-error a word is not an int
  x.plus(s);
  // @ts-expect-error comparisons of order take numbers
  s.lt("a");
  // @ts-expect-error a boolean is not an Iris expression
  b.check(true);
  // @ts-expect-error a boolean is not a literal
  lit(true);
  // @ts-expect-error not a value type
  b.forward("y", "bool");
  // @ts-expect-error a capture reads a scalar
  b.capture("c", int.dim(2));
  // @ts-expect-error validate takes a scalar
  validate(int.dim(2));
  // @ts-expect-error a number is not a pattern part
  pattern("x", 1);
  // @ts-expect-error a store needs a name
  b.repeatUntil("END", { store: discard() });
  // @ts-expect-error the top level is in no repeat
  void b.record;
  // @ts-expect-error the top level is in no repeat
  b.eachName("row");
  // @ts-expect-error the else branch declares the name `if`
  b.if(x, () => {}).elseReadIf("other", int);
  // @ts-expect-error a type has no addition
  b.read("t", a);
  b.if(x, (t) => {
    // @ts-expect-error a conditional block is not a repeat
    void t.record;
  });
}
