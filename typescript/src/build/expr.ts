// Handles, expressions, records and the functions that combine them.
import type { Handle, HandleDepth, Presence } from "../handle.js";
import { newBinding, node, type BinarySymbol, type Node, type ValueTag } from "./ops.js";

const INT_MIN = -(2n ** 63n);
const INT_MAX = 2n ** 63n - 1n;
type Integral = "int" | "dyn";
type Numeric = "int" | "float" | "num" | "dyn";
const INTEGRAL: readonly ValueTag[] = ["int", "dyn"];
const NUMERIC: readonly ValueTag[] = ["int", "float", "num", "dyn"];

/** An operand: an expression, a handle, or a literal. */
export type Operand = Expr | number | bigint | string;
/** An integer operand. A `number` must be a safe integer. */
export type IntOperand = Expr<Integral> | number | bigint;
/** A numeric operand. */
export type NumericOperand = Expr<Numeric> | number | bigint;
/** A string operand. */
export type StringOperand = Expr<"string" | "dyn"> | string;

/**
 * What `==` and `!=` compare with a value of type `V`: a string with a
 * string, a number with a number, and anything with a dynamic value. A
 * receiver whose type may be either takes only dynamic operands.
 */
export type Equatable<V extends ValueTag> = [V] extends ["dyn"]
  ? Operand
  : [V] extends ["string"]
    ? StringOperand
    : [V] extends [Exclude<Numeric, "dyn">]
      ? NumericOperand
      : Expr<"dyn">;

/** The value type of an operand. */
export type TagOf<E> =
  E extends Expr<infer V>
    ? V
    : E extends string
      ? "string"
      : E extends number | bigint
        ? "int"
        : never;

// Module-private access to the classes' private state, set in their static
// blocks, so that the published declarations expose none of it.
let nodeOf: (expr: Expr) => Node;
let openOf: (expr: Expr) => boolean;
let makeExpr: <V extends ValueTag>(tree: Node, tag: V) => Expr<V>;
let makeLookup: <V extends ValueTag>(tree: Node, tag: V) => Lookup<V>;
let makeField: <V extends ValueTag, D extends HandleDepth, P extends Presence>(
  name: string,
  owner: number,
  tag: V,
  depth: D,
  presence: P,
) => Field<V, D, P>;
let fieldOwner: (field: Field) => number;
let recordParts: (record: Record) => { readonly id: number; readonly owner: number };
let recordListNode: (records: RecordList) => Node;
let recordExprNode: (record: RecordExpr) => Node;

/**
 * An Iris expression of value type `V`. Methods build larger expressions:
 * `a.plus(b)` is `a + b`, and `div`, `rem` and `pow` are functions.
 */
export class Expr<V extends ValueTag = ValueTag> {
  /** The value type: `int`, `float`, `num`, `string`, or `dyn` for any. */
  readonly valueType: V;
  readonly #node: Node;

  protected constructor(tree: Node, tag: V) {
    this.#node = tree;
    this.valueType = tag;
  }

  static {
    nodeOf = (expr) => expr.#node;
    openOf = (expr) => expr instanceof Lookup;
    makeExpr = (tree, tag) => new Expr(tree, tag);
  }

  /** This expression with no static type, accepted anywhere. */
  dynamic(): Lookup<"dyn"> {
    return makeLookup(this.#node, "dyn");
  }

  /** `this + other`. */
  plus(this: Expr<Integral>, other: IntOperand): Expr<"int"> {
    return binary(this, other, "+", INTEGRAL);
  }

  /** `this - other`. */
  minus(this: Expr<Integral>, other: IntOperand): Expr<"int"> {
    return binary(this, other, "-", INTEGRAL);
  }

  /** `this * other`. */
  times(this: Expr<Integral>, other: IntOperand): Expr<"int"> {
    return binary(this, other, "*", INTEGRAL);
  }

  /** `this && other`. */
  and(this: Expr<Integral>, other: IntOperand): Expr<"int"> {
    return binary(this, other, "&&", INTEGRAL);
  }

  /** `this || other`. */
  or(this: Expr<Integral>, other: IntOperand): Expr<"int"> {
    return binary(this, other, "||", INTEGRAL);
  }

  /** `this < other`. */
  lt(this: Expr<Numeric>, other: NumericOperand): Expr<"int"> {
    return binary(this, other, "<", NUMERIC);
  }

  /** `this <= other`. */
  le(this: Expr<Numeric>, other: NumericOperand): Expr<"int"> {
    return binary(this, other, "<=", NUMERIC);
  }

  /** `this > other`. */
  gt(this: Expr<Numeric>, other: NumericOperand): Expr<"int"> {
    return binary(this, other, ">", NUMERIC);
  }

  /** `this >= other`. */
  ge(this: Expr<Numeric>, other: NumericOperand): Expr<"int"> {
    return binary(this, other, ">=", NUMERIC);
  }

  /** `this == other`: both strings, both numbers, or one dynamic. */
  eq<W extends ValueTag>(this: Expr<W>, other: Equatable<W>): Expr<"int"> {
    return equality(this, other, "==");
  }

  /** `this != other`: both strings, both numbers, or one dynamic. */
  ne<W extends ValueTag>(this: Expr<W>, other: Equatable<W>): Expr<"int"> {
    return equality(this, other, "!=");
  }

  /** `-this`, of the same value type. */
  neg<W extends Numeric>(this: Expr<W>): Expr<W> {
    if (!NUMERIC.includes(this.valueType)) {
      throw new TypeError(`unary \`-\` takes a number, not a ${describeValue(this)}`);
    }
    return makeExpr(node({ kind: "unary", symbol: "-" }, [this.#node]), this.valueType);
  }

  /** `!this`. */
  not(this: Expr<Integral>): Expr<"int"> {
    if (!INTEGRAL.includes(this.valueType)) {
      throw new TypeError(`\`not\` (Iris \`!\`) takes an int, not a ${describeValue(this)}`);
    }
    return makeExpr(node({ kind: "unary", symbol: "!" }, [this.#node]), "int");
  }

  /**
   * Throws a `TypeError`: an expression is not text. Write a pattern with the
   * `p` tag to match text that holds a value.
   */
  toString(): never {
    throw new TypeError(
      "an Iris expression cannot be converted to a string: write a pattern with the `p` tag",
    );
  }
}

/**
 * A lookup: a handle, an index or a field. It may be an array, so it can be
 * indexed and have its fields looked up.
 */
export class Lookup<V extends ValueTag = ValueTag> extends Expr<V> {
  static {
    makeLookup = (tree, tag) => new Lookup(tree, tag);
  }

  /** `this[index]`. */
  at(index: IntOperand): Lookup<V> {
    return makeLookup(
      node({ kind: "index" }, [nodeOf(this), nodeOf(indexOperand(index))]),
      this.valueType,
    );
  }

  /** `this.name`: the field `handle` of the record this selects. */
  get<F extends ValueTag>(handle: Field<F>): Lookup<F> {
    return member(nodeOf(this), handle);
  }
}

/**
 * A typed handle: the name of an entry, with the value type, depth and
 * presence it promises. In the builder it is an expression; after reading,
 * `data.get(handle)` reads its entry.
 */
export class Field<
  V extends ValueTag = ValueTag,
  D extends HandleDepth = HandleDepth,
  P extends Presence = Presence,
>
  extends Lookup<V>
  implements Handle<V, D, P>
{
  /** The entry name. */
  readonly name: string;
  /** The array levels of the entry, or `null` when they are open. */
  readonly depth: D;
  /** Whether the entry always exists after a successful read. */
  readonly presence: P;
  readonly #owner: number;

  private constructor(name: string, owner: number, tag: V, depth: D, presence: P) {
    super(node({ kind: "name", text: name, owner }), tag);
    this.name = name;
    this.depth = depth;
    this.presence = presence;
    this.#owner = owner;
  }

  static {
    makeField = (name, owner, tag, depth, presence) => new Field(name, owner, tag, depth, presence);
    fieldOwner = (field) => field.#owner;
  }
}

/** A handle whose value type and depth are open. */
export type DynField = Field<"dyn", null, "maybe">;

/**
 * The records of a repeat. `record.get(field)` is the field of the current
 * record, `record.list` all the records, and a comprehension over a record
 * gives its body one record at a time.
 */
export class Record {
  readonly #id: number;
  readonly #owner: number;

  private constructor(id: number, owner: number) {
    this.#id = id;
    this.#owner = owner;
  }

  static {
    recordParts = (record) => ({ id: record.#id, owner: record.#owner });
  }

  /** @internal */
  static create(id: number, owner: number): Record {
    return new Record(id, owner);
  }

  /** The field `handle` of the current record. */
  get<F extends ValueTag>(handle: Field<F>): Lookup<F> {
    return member(this.#node(), handle);
  }

  /** All the records of this repeat, printed as its `as` name. */
  get list(): RecordList {
    return RecordList.create(node({ kind: "recordList", record: this.#id, owner: this.#owner }));
  }

  /** The records of the nested repeat `inner` in the current record. */
  records(inner: Record): RecordList {
    return listMember(this.#node(), inner);
  }

  #node(): Node {
    return node({ kind: "record", record: this.#id, owner: this.#owner });
  }
}

/** A list of records: all the records of a repeat, or a nested list. */
export class RecordList {
  readonly #node: Node;

  private constructor(tree: Node) {
    this.#node = tree;
  }

  static {
    recordListNode = (records) => records.#node;
  }

  /** @internal */
  static create(tree: Node): RecordList {
    return new RecordList(tree);
  }

  /** `this[index]`: one record. */
  at(index: IntOperand): RecordExpr {
    return RecordExpr.create(node({ kind: "index" }, [this.#node, nodeOf(indexOperand(index))]));
  }
}

/** One record: selected by index, or a comprehension's variable. */
export class RecordExpr {
  readonly #node: Node;

  private constructor(tree: Node) {
    this.#node = tree;
  }

  static {
    recordExprNode = (record) => record.#node;
  }

  /** @internal */
  static create(tree: Node): RecordExpr {
    return new RecordExpr(tree);
  }

  /** The field `handle` of this record. */
  get<F extends ValueTag>(handle: Field<F>): Lookup<F> {
    return member(this.#node, handle);
  }

  /** `this[index]`, for a record that is itself a list. */
  at(index: IntOperand): RecordExpr {
    return RecordExpr.create(node({ kind: "index" }, [this.#node, nodeOf(indexOperand(index))]));
  }

  /** The records of the nested repeat `inner` in this record. */
  records(inner: Record): RecordList {
    return listMember(this.#node, inner);
  }
}

/** A domain whose comprehension variable has a chosen name. */
export class Over<T extends Domain = Domain> {
  readonly #domain: T;
  readonly #name: string | null;

  private constructor(domain: T, name: string | null) {
    this.#domain = domain;
    this.#name = name;
  }

  /** @internal */
  static create<T extends Domain>(domain: T, name: string | null): Over<T> {
    return new Over(domain, name);
  }

  /** @internal The domain and the chosen name. */
  parts(): { readonly domain: T; readonly name: string | null } {
    return { domain: this.#domain, name: this.#name };
  }
}

/** What a comprehension ranges over. */
export type Domain = Expr | Record | RecordList;

/** What a comprehension gives its body for one element of `D`. */
export type Item<D> =
  D extends Over<infer T>
    ? Item<T>
    : D extends Record | RecordList
      ? RecordExpr
      : D extends Expr<infer V>
        ? Lookup<V>
        : never;

/** @internal The tree of an expression. */
export function treeOf(expr: Expr): Node {
  return nodeOf(expr);
}

/** @internal Whether an expression looks up a stored value, which may be an array. */
export function isOpen(expr: Expr): boolean {
  return openOf(expr);
}

/** @internal */
export function createField<V extends ValueTag, D extends HandleDepth, P extends Presence>(
  name: string,
  owner: number,
  tag: V,
  depth: D,
  presence: P,
): Field<V, D, P> {
  return makeField(name, owner, tag, depth, presence);
}

/** @internal A handle whose value type and depth are open. */
export function dynField(name: string, owner: number): DynField {
  return makeField(name, owner, "dyn", null, "maybe");
}

/** @internal */
export function ownerOf(field: Field): number {
  return fieldOwner(field);
}

/** @internal */
export function recordOf(record: Record): { readonly id: number; readonly owner: number } {
  return recordParts(record);
}

function describeValue(expr: Expr): string {
  return expr.valueType === "dyn" ? "dynamic value" : expr.valueType;
}

/** An Iris integer: a safe integer `number`, or a `bigint` in the 64-bit range. */
export function lit(value: number | bigint): Expr<"int">;
/** An Iris string. */
export function lit(value: string): Expr<"string">;
export function lit(value: number | bigint | string): Expr<"int" | "string">;
export function lit(value: number | bigint | string): Expr<"int" | "string"> {
  if (typeof value === "string") {
    return makeExpr(node({ kind: "string", text: value }), "string");
  }
  return makeExpr(node({ kind: "int", value: integer(value) }), "int");
}

/** An Iris float, which prints with a decimal point or an exponent. */
export function floatLit(value: number): Expr<"float"> {
  if (typeof value !== "number") {
    throw new TypeError(`a float literal is a number, not ${typeName(value)}`);
  }
  return makeExpr(node({ kind: "float", value }), "float");
}

function integer(value: unknown): bigint {
  if (typeof value === "bigint") {
    if (value < INT_MIN || value > INT_MAX) {
      throw new RangeError(`the integer ${value} is outside the 64-bit range of Iris`);
    }
    return value;
  }
  if (typeof value === "number") {
    if (!Number.isInteger(value)) {
      throw new TypeError(`${value} is not an integer: use floatLit for an Iris float`);
    }
    if (!Number.isSafeInteger(value)) {
      throw new RangeError(`${value} is not a safe integer: write it as a bigint`);
    }
    return BigInt(value);
  }
  throw new TypeError(`an Iris integer is a number or a bigint, not ${typeName(value)}`);
}

/** @internal An expression, or a literal as one. */
export function operand(value: unknown): Expr {
  if (value instanceof Expr) {
    return value;
  }
  if (typeof value === "number" || typeof value === "bigint" || typeof value === "string") {
    return lit(value);
  }
  if (typeof value === "boolean") {
    throw new TypeError("Iris has no bool literal: use 1 and 0");
  }
  throw new TypeError(
    `an Iris operand is an expression, a handle, a number, a bigint or a string, not ${typeName(value)}`,
  );
}

/** @internal A name for a value's type, for messages. */
export function typeName(value: unknown): string {
  if (value === null) {
    return "null";
  }
  if (typeof value === "object" || typeof value === "function") {
    const name: unknown = Reflect.get(Object(value), "constructor")?.name;
    return typeof name === "string" && name !== "" ? `a ${name}` : `a ${typeof value}`;
  }
  return `a ${typeof value}`;
}

function binary(
  left: Expr,
  right: unknown,
  symbol: BinarySymbol,
  accepted: readonly ValueTag[],
): Expr<"int"> {
  const other = operand(right);
  for (const [side, expr] of [
    ["left", left],
    ["right", other],
  ] as const) {
    if (!accepted.includes(expr.valueType)) {
      const wanted = accepted === INTEGRAL ? "an int" : "a number";
      throw new TypeError(
        `the ${side} operand of \`${symbol}\` must be ${wanted}, not a ${describeValue(expr)}`,
      );
    }
  }
  return makeExpr(node({ kind: "binary", symbol }, [nodeOf(left), nodeOf(other)]), "int");
}

function equatable(left: ValueTag, right: ValueTag): boolean {
  if (left === "dyn" || right === "dyn") {
    return true;
  }
  return (left === "string") === (right === "string");
}

function equality(left: Expr, right: unknown, symbol: "==" | "!="): Expr<"int"> {
  const other = operand(right);
  if (!equatable(left.valueType, other.valueType)) {
    throw new TypeError(
      `\`${symbol}\` compares two strings or two numbers, not a ${describeValue(left)} with a ${describeValue(other)}`,
    );
  }
  return makeExpr(node({ kind: "binary", symbol }, [nodeOf(left), nodeOf(other)]), "int");
}

function integral(value: unknown, role: string): Expr {
  const expr = operand(value);
  if (!INTEGRAL.includes(expr.valueType)) {
    throw new TypeError(`${role} must be an int, not a ${describeValue(expr)}`);
  }
  return expr;
}

function indexOperand(value: unknown): Expr {
  return integral(value, "an index");
}

function arithmetic(symbol: BinarySymbol, left: unknown, right: unknown): Expr<"int"> {
  const leftExpr = integral(left, `the left operand of \`${symbol}\``);
  const rightExpr = integral(right, `the right operand of \`${symbol}\``);
  return makeExpr(node({ kind: "binary", symbol }, [nodeOf(leftExpr), nodeOf(rightExpr)]), "int");
}

/** Iris `left / right`, integer division rounding toward zero. */
export function div(left: IntOperand, right: IntOperand): Expr<"int"> {
  return arithmetic("/", left, right);
}

/** Iris `left % right`, the remainder with the sign of `left`. */
export function rem(left: IntOperand, right: IntOperand): Expr<"int"> {
  return arithmetic("%", left, right);
}

/** Iris `base ^ exponent`. */
export function pow(base: IntOperand, exponent: IntOperand): Expr<"int"> {
  return arithmetic("^", base, exponent);
}

function member<F extends ValueTag>(base: Node, handle: Field<F>): Lookup<F> {
  if (!(handle instanceof Field)) {
    throw new TypeError(`a field is a handle, not ${typeName(handle)}`);
  }
  return makeLookup(
    node({ kind: "member", text: handle.name, owner: fieldOwner(handle) }, [base]),
    handle.valueType,
  );
}

function listMember(base: Node, inner: unknown): RecordList {
  if (!(inner instanceof Record)) {
    throw new TypeError(`\`records\` takes a record, not ${typeName(inner)}`);
  }
  const { id, owner } = recordParts(inner);
  return RecordList.create(node({ kind: "listMember", record: id, owner }, [base]));
}

/** `domain`, with its comprehension variable printed as `name`. */
export function over<T extends Domain>(domain: T, name?: string): Over<T> {
  domainNode(domain);
  if (name !== undefined && typeof name !== "string") {
    throw new TypeError(`a comprehension variable's name is a string, not ${typeName(name)}`);
  }
  return Over.create(domain, name ?? null);
}

function domainNode(domain: unknown): Node {
  if (domain instanceof Over) {
    return domainNode(domain.parts().domain);
  }
  if (domain instanceof Record) {
    return recordListNode(domain.list);
  }
  if (domain instanceof RecordList) {
    return recordListNode(domain);
  }
  if (domain instanceof Expr) {
    return nodeOf(domain);
  }
  throw new TypeError(
    `a domain is an expression, a handle, a record or a record list, not ${typeName(domain)}`,
  );
}

function item(domain: unknown, tree: Node): unknown {
  if (domain instanceof Over) {
    return item(domain.parts().domain, tree);
  }
  if (domain instanceof Record || domain instanceof RecordList) {
    return RecordExpr.create(tree);
  }
  if (domain instanceof Expr) {
    return makeLookup(tree, domain.valueType);
  }
  return undefined;
}

function comprehension(name: string, domain: unknown, body: unknown): Expr<"int"> {
  const tree = domainNode(domain);
  if (typeof body !== "function") {
    throw new TypeError(`\`${name}\` takes a domain and a function of its variable`);
  }
  const chosen = domain instanceof Over ? domain.parts().name : null;
  const binding = newBinding();
  // The overloads that call this check the body's parameter type.
  const element = item(domain, node({ kind: "binding", binding }));
  const result = operand(Reflect.apply(body, undefined, [element]));
  return makeExpr(
    node({ kind: "comprehension", function: name, binding, name: chosen }, [tree, nodeOf(result)]),
    "int",
  );
}

function call(name: string, args: readonly unknown[]): Expr<"int"> {
  const trees = args.map((argument) => nodeOf(operand(argument)));
  return makeExpr(node({ kind: "call", function: name, arguments: trees.length }, trees), "int");
}

/** `all(x in domain: body)`. */
export function all<D extends Domain | Over>(
  domain: D,
  body: (element: Item<D>) => Operand,
): Expr<"int"> {
  return comprehension("all", domain, body);
}

/** `any(x in domain: body)`. */
export function any<D extends Domain | Over>(
  domain: D,
  body: (element: Item<D>) => Operand,
): Expr<"int"> {
  return comprehension("any", domain, body);
}

/** `sum(x in domain: body)`, given a domain and a function of its variable. */
export function sum<D extends Domain | Over>(
  domain: D,
  body: (element: Item<D>) => Operand,
): Expr<"int">;
/** `sum(a, b, ...)`. */
export function sum(...args: Operand[]): Expr<"int">;
export function sum(...args: unknown[]): Expr<"int"> {
  const [domain, body] = args;
  if (args.length === 2 && typeof body === "function") {
    return comprehension("sum", domain, body);
  }
  return call("sum", args);
}

/** `count(domain)`, or `count(x in domain: body)` given a function. */
export function count<D extends Domain | Over>(
  domain: D,
  body?: (element: Item<D>) => Operand,
): Expr<"int"> {
  if (body !== undefined) {
    return comprehension("count", domain, body);
  }
  return makeExpr(
    node({ kind: "call", function: "count", arguments: 1 }, [domainNode(domain)]),
    "int",
  );
}

/** `size(domain)`. */
export function size(domain: Domain | Over): Expr<"int"> {
  return makeExpr(
    node({ kind: "call", function: "size", arguments: 1 }, [domainNode(domain)]),
    "int",
  );
}

/** `min(a, b, ...)`. */
export function min(...args: Operand[]): Expr<"int"> {
  return call("min", args);
}

/** `max(a, b, ...)`. */
export function max(...args: Operand[]): Expr<"int"> {
  return call("max", args);
}

/** `distinct(a, b, ...)`. */
export function distinct(...args: Operand[]): Expr<"int"> {
  return call("distinct", args);
}

/**
 * `defined(name)`: a handle, record, record list or comprehension variable.
 * `defined(record)` tests the record list.
 */
export function defined(name: Expr | Record | RecordList | RecordExpr): Expr<"int"> {
  let tree: Node;
  if (name instanceof Record) {
    tree = recordListNode(name.list);
  } else if (name instanceof RecordList) {
    tree = recordListNode(name);
  } else if (name instanceof RecordExpr) {
    tree = recordExprNode(name);
  } else if (name instanceof Expr) {
    tree = nodeOf(name);
  } else {
    throw new TypeError(`\`defined\` takes a handle or a record, not ${typeName(name)}`);
  }
  return makeExpr(node({ kind: "call", function: "defined", arguments: 1 }, [tree]), "int");
}

function host(name: string): Expr<"int"> {
  return makeExpr(node({ kind: "host", name }), "int");
}

/** Gecode's host constants, for ranges and checks. */
export const gecode: {
  /** `gecode.int_max`, 2147483646. */
  readonly intMax: Expr<"int">;
  /** `gecode.int_min`, -2147483646. */
  readonly intMin: Expr<"int">;
} = Object.freeze({ intMax: host("gecode.int_max"), intMin: host("gecode.int_min") });
