// Reading types and the options of a declaration.
import type { HandleDepth } from "../handle.js";
import { operand, treeOf, typeName, type Expr, type IntOperand, type Operand } from "./expr.js";
import { postfix, type Ops } from "./ops.js";

/** The value types a reading type reads. */
export type ReadValue = "int" | "float" | "num" | "string";

const NEXT = [1, 2, 3, 4, 5, 6, 7, 8, null] as const;

/** One more array level; beyond 8 the depth is open. */
export type Next<D extends HandleDepth> = D extends number ? (typeof NEXT)[D] : null;

/** `D` when it is one depth, else `null`. */
export type OneDepth<D extends HandleDepth, All extends HandleDepth = D> = D extends unknown
  ? [All] extends [D]
    ? D
    : null
  : never;

/**
 * `A` plus `B` array levels. A `B` known only at run time, or one of several
 * counts, gives an open depth.
 */
export type Add<A extends HandleDepth, B extends number> = number extends B
  ? null
  : OneDepth<B extends unknown ? AddLevels<A, B> : never>;

/** `A` plus the one count `B`. */
type AddLevels<A extends HandleDepth, B extends number, C extends 0[] = []> = C["length"] extends B
  ? A
  : A extends null
    ? null
    : AddLevels<Next<A>, B, [...C, 0]>;

/** @internal One more array level, or `null` past 8. */
export function nextDepth(depth: number | null): HandleDepth {
  return depth === null ? null : (NEXT[depth] ?? null);
}

/** @internal `depth` as a handle depth: `null` past 8. */
export function handleDepth(depth: number | null): HandleDepth {
  return depth === null ? null : depth === 0 ? 0 : nextDepth(depth - 1);
}

export type AlphabetOption =
  | { readonly kind: "blank"; readonly characters: string }
  | { readonly kind: "strip"; readonly characters: string }
  | { readonly kind: "base"; readonly value: bigint };

export type TypeSpec =
  /** `int`, `float`, `num`, `word` or `rest`. */
  | { readonly kind: "base"; readonly name: string }
  | {
      readonly kind: "alphabet";
      readonly symbols: string;
      readonly options: readonly AlphabetOption[];
    }
  /** A use of the named type that the statement `declaration` declares. */
  | {
      readonly kind: "named";
      readonly name: string;
      readonly owner: number;
      readonly declaration: number;
    }
  | {
      readonly kind: "range";
      readonly lower: Ops | null;
      readonly upper: Ops | null;
      readonly inclusive: boolean;
    };

/** A type with the dimensions and options of a declaration. */
export interface Declared {
  readonly ty: TypeSpec;
  readonly dimensions: readonly Ops[];
  readonly layout: Ops | null;
  readonly offset: Ops | null;
}

/** @internal Whether the text of this type ends in `..`, with no upper bound. */
export function isOpenEnded(declared: Declared): boolean {
  return (
    declared.layout === null &&
    declared.offset === null &&
    declared.ty.kind === "range" &&
    declared.ty.upper === null
  );
}

function plain(ty: TypeSpec): Declared {
  return { ty, dimensions: [], layout: null, offset: null };
}

function ops(value: unknown): Ops {
  return postfix(treeOf(operand(value)));
}

let declaredOf: (type: Type) => Declared;
let makeType: <V extends ReadValue, N extends HandleDepth>(
  tag: V,
  declaration: Declared,
  depth: N,
) => Type<V, N>;

/** `depth` with one more array level, as `Next` types it. */
function nextLevel<D extends HandleDepth>(depth: D): Next<D>;
function nextLevel(depth: HandleDepth): HandleDepth {
  return nextDepth(depth);
}

/** `depth` with `count` more array levels, as `Add` types it. */
function addLevels<D extends HandleDepth, B extends number>(depth: D, count: B): Add<D, B>;
function addLevels(depth: HandleDepth, count: number): HandleDepth {
  return depth === null ? null : handleDepth(depth + count);
}

/**
 * A reading type that reads values of type `V` into `N` array levels. `dim`
 * adds a dimension, and `layout` and `offset` add the declaration's options.
 */
export class Type<V extends ReadValue = ReadValue, N extends HandleDepth = HandleDepth> {
  /** The value type it reads. */
  readonly valueType: V;
  /** The array levels it reads into: its dimensions, or `null` past 8. */
  readonly depth: N;
  readonly #declared: Declared;

  protected constructor(tag: V, declaration: Declared, depth: N) {
    this.valueType = tag;
    this.depth = depth;
    this.#declared = declaration;
  }

  static {
    declaredOf = (type) => type.#declared;
    makeType = (tag, declaration, depth) => new Type(tag, declaration, depth);
  }

  /** Adds a dimension: `x[size]: type`. */
  dim(size: IntOperand): Type<V, Next<N>> {
    const declaration = this.#declared;
    return makeType(
      this.valueType,
      { ...declaration, dimensions: [...declaration.dimensions, ops(size)] },
      nextLevel(this.depth),
    );
  }

  /** Adds several dimensions. */
  dims<const S extends readonly IntOperand[]>(...sizes: S): Type<V, Add<N, S["length"]>> {
    const declaration = this.#declared;
    const added = sizes.map((size) => ops(size));
    return makeType(
      this.valueType,
      { ...declaration, dimensions: [...declaration.dimensions, ...added] },
      addLevels<N, S["length"]>(this.depth, sizes.length),
    );
  }

  /** `layout expression`. */
  layout(layout: Operand): Type<V, N> {
    return makeType(this.valueType, { ...this.#declared, layout: ops(layout) }, this.depth);
  }

  /** `offset expression`. */
  offset(offset: Operand): Type<V, N> {
    return makeType(this.valueType, { ...this.#declared, offset: ops(offset) }, this.depth);
  }

  /** A list of this type ending at `end`: a sentinel, or `eol`. */
  until(this: Type<V, 0>, end: Operand | Eol): ListReading<V> {
    if (this.#declared.dimensions.length > 0) {
      throw new TypeError("a list reads scalars: its type has no dimensions");
    }
    return ListReading.create(this.valueType, this.#declared, end instanceof Eol ? null : ops(end));
  }
}

/**
 * `alphabet "symbols"`: reads each symbol as its index. `blank`, `strip` and
 * `base` add options in call order.
 */
export class Alphabet extends Type<"int", 0> {
  /** @internal */
  static create(declaration: Declared): Alphabet {
    return new Alphabet("int", declaration, 0);
  }

  #option(option: AlphabetOption): Alphabet {
    const declaration = declaredOf(this);
    const { ty } = declaration;
    const options = ty.kind === "alphabet" ? ty.options : [];
    const symbols = ty.kind === "alphabet" ? ty.symbols : "";
    return Alphabet.create({
      ...declaration,
      ty: { kind: "alphabet", symbols, options: [...options, option] },
    });
  }

  /** `blank "characters"`. */
  blank(characters: string): Alphabet {
    return this.#option({ kind: "blank", characters: text(characters, "blank characters") });
  }

  /** `strip "characters"`. */
  strip(characters: string): Alphabet {
    return this.#option({ kind: "strip", characters: text(characters, "strip characters") });
  }

  /** `base value`. */
  base(value: number | bigint): Alphabet {
    if (typeof value === "number" ? !Number.isSafeInteger(value) : typeof value !== "bigint") {
      throw new TypeError(`an alphabet base is an integer, not ${typeName(value)}`);
    }
    return this.#option({ kind: "base", value: BigInt(value) });
  }
}

/** The options of `alphabet`, added in this order. */
export interface AlphabetOptions {
  readonly blank?: string;
  readonly strip?: string;
  readonly base?: number | bigint;
}

/** `alphabet "symbols"`, with the options given. */
export function alphabet(symbols: string, options: AlphabetOptions = {}): Alphabet {
  let result = Alphabet.create(
    plain({ kind: "alphabet", symbols: text(symbols, "an alphabet's symbols"), options: [] }),
  );
  if (options.blank !== undefined) {
    result = result.blank(options.blank);
  }
  if (options.strip !== undefined) {
    result = result.strip(options.strip);
  }
  if (options.base !== undefined) {
    result = result.base(options.base);
  }
  return result;
}

/** A range bound: an int or `num` expression, or an integer. */
export type Bound = Expr<"int" | "num" | "dyn"> | number | bigint;

/** `lo..hi`, an integer range. `below` gives it an exclusive upper bound. */
export class Range extends Type<"int", 0> {
  /** @internal */
  static create(declaration: Declared): Range {
    return new Range("int", declaration, 0);
  }

  /** `lo..<hi`: this range's lower bound with the exclusive upper bound `hi`. */
  below(hi: Bound): Type<"int", 0> {
    const { ty } = declaredOf(this);
    if (ty.kind !== "range" || ty.upper !== null) {
      throw new TypeError("`below` gives a range its upper bound, but this range has one");
    }
    return makeType(
      "int",
      plain({ kind: "range", lower: ty.lower, upper: bound(hi), inclusive: false }),
      0,
    );
  }
}

function bound(value: unknown): Ops {
  const expr = operand(value);
  if (expr.valueType !== "int" && expr.valueType !== "num" && expr.valueType !== "dyn") {
    throw new TypeError(`a range bound is an int or num, not a ${expr.valueType}`);
  }
  return postfix(treeOf(expr));
}

/**
 * `lo..hi`, both included. Either end may be `undefined`, but not both;
 * `range(lo).below(hi)` is `lo..<hi`.
 */
export function range(lo: Bound | undefined, hi?: Bound): Range {
  if (lo === undefined && hi === undefined) {
    throw new TypeError("a range needs at least one bound; use `int` for any integer");
  }
  return Range.create(
    plain({
      kind: "range",
      lower: lo === undefined ? null : bound(lo),
      upper: hi === undefined ? null : bound(hi),
      inclusive: true,
    }),
  );
}

/** `lo..`, the integers from `lo` up. */
export function atLeast(lo: Bound): Range {
  return range(lo);
}

/** `..hi`, the integers up to `hi`. */
export function atMost(hi: Bound): Range {
  return range(undefined, hi);
}

/** `..<hi`, the integers below `hi`. */
export function below(hi: Bound): Type<"int", 0> {
  return makeType(
    "int",
    plain({ kind: "range", lower: null, upper: bound(hi), inclusive: false }),
    0,
  );
}

/** The type and end of a `list` statement. */
export class ListReading<V extends ReadValue = ReadValue> {
  readonly valueType: V;
  readonly #declared: Declared;
  readonly #end: Ops | null;

  private constructor(tag: V, declared: Declared, end: Ops | null) {
    this.valueType = tag;
    this.#declared = declared;
    this.#end = end;
  }

  /** @internal */
  static create<V extends ReadValue>(tag: V, declared: Declared, end: Ops | null): ListReading<V> {
    return new ListReading(tag, declared, end);
  }

  /** @internal The declared type and the sentinel, or `null` for `until eol`. */
  parts(): { readonly declared: Declared; readonly end: Ops | null } {
    return { declared: this.#declared, end: this.#end };
  }

  /** `offset expression`, after the list's end. */
  offset(offset: Operand): ListReading<V> {
    return new ListReading(this.valueType, { ...this.#declared, offset: ops(offset) }, this.#end);
  }
}

/** The end-of-line marker of a list. */
export class Eol {
  static readonly #instance: Eol = new Eol();

  private constructor() {}

  /** @internal */
  static get instance(): Eol {
    return Eol.#instance;
  }
}

/** `eol`, the end of a list at the end of its line. */
export const eol: Eol = Eol.instance;
/** `int`. */
export const int: Type<"int", 0> = makeType("int", plain({ kind: "base", name: "int" }), 0);
/** `float`. */
export const float: Type<"float", 0> = makeType("float", plain({ kind: "base", name: "float" }), 0);
/** `num`: an int or a float, decided by the data. */
export const num: Type<"num", 0> = makeType("num", plain({ kind: "base", name: "num" }), 0);
/** `word`. */
export const word: Type<"string", 0> = makeType("string", plain({ kind: "base", name: "word" }), 0);
/** `rest`, the rest of the line. */
export const rest: Type<"string", 0> = makeType("string", plain({ kind: "base", name: "rest" }), 0);

/** @internal The declaration of a type, checked to be one. */
export function declarationOf(ty: unknown, role: string): Declared {
  if (!(ty instanceof Type)) {
    throw new TypeError(`${role} is a type such as int, not ${typeName(ty)}`);
  }
  return declaredOf(ty);
}

/** @internal The declaration of a type without dimensions. */
export function scalarDeclared(ty: unknown, role: string): Declared {
  const result = declarationOf(ty, role);
  if (result.dimensions.length > 0) {
    throw new TypeError(`${role} has no dimensions`);
  }
  return result;
}

/** @internal A type of value type `tag` with another declaration. */
export function namedType<V extends ReadValue>(tag: V, declaration: Declared): Type<V, 0> {
  return makeType(tag, declaration, 0);
}

/** @internal */
export function checkText(value: unknown, role: string): string {
  return text(value, role);
}

function text(value: unknown, role: string): string {
  if (typeof value !== "string") {
    throw new TypeError(`${role} must be a string, not ${typeName(value)}`);
  }
  return value;
}
