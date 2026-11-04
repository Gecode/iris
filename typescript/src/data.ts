import { renderDzn, renderJson } from "./backend.js";
import { decode, type Column, type ValueType } from "./transfer.js";

export type { ValueType };

/** A named item in the data, with its value type and depth. */
export interface Entry {
  readonly name: string;
  readonly valueType: ValueType;
  /** 0 for a scalar, 1 for an array, 2 for an array of arrays, and so on. */
  readonly depth: number;
}

/** Indices, counted from 0, that select a row or a single value of an entry. */
export type Path = readonly number[];

/** A value with integers as `number`; arrays are nested and may be ragged. */
export type Value = number | string | Value[];

/** A value with integers as `bigint` and floats as `number`. */
export type BigIntValue = bigint | number | string | BigIntValue[];

export interface ValueOptions {
  /** Return integers as `bigint`, which is always exact. */
  readonly bigint?: boolean;
}

/**
 * The data from one read or catalog.
 *
 * Every accessor returns a copy. Integer accessors that return `number`
 * throw `RangeError` for values beyond `Number.MAX_SAFE_INTEGER` in
 * magnitude; the `bigint` accessors are exact. An unknown entry or an index
 * out of bounds throws `RangeError`, and a selection of the wrong value type
 * or depth throws `TypeError`.
 */
export class Data {
  /** The entries in the order their names were defined. */
  readonly entries: readonly Entry[];
  readonly #buffer: Uint8Array;
  readonly #columns: ReadonlyMap<string, Column>;

  private constructor(buffer: Uint8Array) {
    const columns = decode(buffer);
    this.#buffer = buffer;
    this.#columns = new Map(columns.map((column) => [column.name, column]));
    this.entries = Object.freeze(
      columns.map((column) =>
        Object.freeze({
          name: column.name,
          valueType: column.leaves.valueType,
          depth: column.levels.length,
        }),
      ),
    );
  }

  /** @internal Wraps the transfer bytes of rust/iris-js. */
  static fromTransfer(buffer: Uint8Array): Data {
    return new Data(buffer);
  }

  /** The entry with this name, or `undefined` when there is none. */
  entry(name: string): Entry | undefined {
    return this.entries.find((entry) => entry.name === name);
  }

  has(name: string): boolean {
    return this.#columns.has(name);
  }

  /** The number of elements in the array at `path`. */
  size(name: string, path: Path = []): number {
    const selection = this.#select(name, path);
    const children = selection.children();
    if (children === undefined) {
      throw new TypeError(`${selection.where}: expected an array`);
    }
    return children.end - children.start;
  }

  /** An integer as `number`. */
  int(name: string, path: Path = []): number {
    const selection = this.#leaf(name, path, "int");
    return safeNumber(selection.int(selection.row), selection.where);
  }

  /** An integer as `bigint`. */
  bigInt(name: string, path: Path = []): bigint {
    const selection = this.#leaf(name, path, "int");
    return selection.int(selection.row);
  }

  float(name: string, path: Path = []): number {
    const selection = this.#leaf(name, path, "float");
    return selection.float(selection.row);
  }

  string(name: string, path: Path = []): string {
    const selection = this.#leaf(name, path, "string");
    return selection.string(selection.row);
  }

  /** An integer row as `number`s. */
  ints(name: string, path: Path = []): number[] {
    const { selection, start, end } = this.#row(name, path, "int");
    return Array.from({ length: end - start }, (_, index) =>
      safeNumber(selection.int(start + index), selection.where),
    );
  }

  /** An integer row as a new `BigInt64Array`. */
  bigInts(name: string, path: Path = []): BigInt64Array {
    const { selection, start, end } = this.#row(name, path, "int");
    return selection.ints().slice(start, end);
  }

  floats(name: string, path: Path = []): number[] {
    const { selection, start, end } = this.#row(name, path, "float");
    return Array.from({ length: end - start }, (_, index) => selection.float(start + index));
  }

  strings(name: string, path: Path = []): string[] {
    const { selection, start, end } = this.#row(name, path, "string");
    return Array.from({ length: end - start }, (_, index) => selection.string(start + index));
  }

  /** The value at `path`: a scalar, or nested arrays for an array. */
  value(name: string, path?: Path, options?: { readonly bigint?: false }): Value;
  value(name: string, path: Path | undefined, options: { readonly bigint: true }): BigIntValue;
  value(name: string, path?: Path, options?: ValueOptions): Value | BigIntValue;
  value(name: string, path: Path = [], options: ValueOptions = {}): Value | BigIntValue {
    return this.#select(name, path).value(options.bigint ?? false);
  }

  /** All entries as an object from name to value, in entry order. */
  toObject(options?: { readonly bigint?: false }): Record<string, Value>;
  toObject(options: { readonly bigint: true }): Record<string, BigIntValue>;
  toObject(options?: ValueOptions): Record<string, Value | BigIntValue>;
  toObject(options: ValueOptions = {}): Record<string, Value | BigIntValue> {
    return Object.fromEntries(
      this.entries.map(({ name }) => [name, this.#select(name, []).value(options.bigint ?? false)]),
    );
  }

  /** The canonical JSON document, ending in a newline. */
  toJSON(): string {
    return renderJson(this.#buffer);
  }

  /** The MiniZinc data text defined in the Specification's appendix B. */
  toDzn(): string {
    return renderDzn(this.#buffer);
  }

  #select(name: string, path: Path): Selection {
    const column = this.#columns.get(name);
    if (column === undefined) {
      throw new RangeError(`unknown entry '${name}'`);
    }
    if (!Array.isArray(path)) {
      throw new TypeError(`entry '${name}': a path must be an array of indices`);
    }
    const selection = new Selection(column, describe(name, path));
    for (const index of path) {
      selection.descend(index);
    }
    return selection;
  }

  #leaf(name: string, path: Path, valueType: ValueType): Selection {
    const selection = this.#select(name, path);
    if (selection.depth !== 0) {
      throw new TypeError(`${selection.where}: expected a scalar`);
    }
    selection.requireType(valueType);
    return selection;
  }

  #row(name: string, path: Path, valueType: ValueType): Row {
    const selection = this.#select(name, path);
    const children = selection.children();
    if (children === undefined) {
      throw new TypeError(`${selection.where}: expected an array`);
    }
    if (selection.depth !== 1) {
      throw new TypeError(
        `${selection.where}: expected a row, but the selection has depth ${selection.depth}`,
      );
    }
    selection.requireType(valueType);
    return { selection, ...children };
  }
}

interface Range {
  readonly start: number;
  readonly end: number;
}

interface Row extends Range {
  readonly selection: Selection;
}

/**
 * A row of an entry at one level. When the level equals the entry's depth,
 * the row is the index of a leaf.
 */
class Selection {
  readonly column: Column;
  /** The entry and path, for messages. */
  readonly where: string;
  #level = 0;
  #row = 0;

  constructor(column: Column, where: string) {
    this.column = column;
    this.where = where;
  }

  get row(): number {
    return this.#row;
  }

  /** The depth of the selected value. */
  get depth(): number {
    return this.column.levels.length - this.#level;
  }

  /** Selects element `index` of the selected array. */
  descend(index: number): void {
    if (!Number.isInteger(index)) {
      throw new TypeError(`${this.where}: path indices must be integers`);
    }
    const children = this.children();
    if (children === undefined) {
      throw new TypeError(`${this.where}: expected an array`);
    }
    if (index < 0 || index >= children.end - children.start) {
      throw new RangeError(`${this.where}: path index out of bounds`);
    }
    this.#level += 1;
    this.#row = children.start + index;
  }

  /**
   * The rows of the next level that hold the elements of the selected
   * array, or `undefined` for a scalar.
   */
  children(): Range | undefined {
    const offsets = this.column.levels[this.#level];
    if (offsets === undefined) {
      return undefined;
    }
    return { start: at(offsets, this.#row), end: at(offsets, this.#row + 1) };
  }

  requireType(valueType: ValueType): void {
    if (this.column.leaves.valueType !== valueType) {
      throw new TypeError(`${this.where}: expected ${LEAF_NAMES[valueType]} leaves`);
    }
  }

  ints(): BigInt64Array {
    const { leaves } = this.column;
    if (leaves.valueType !== "int") {
      throw inconsistent();
    }
    return leaves.values;
  }

  int(index: number): bigint {
    return at(this.ints(), index);
  }

  float(index: number): number {
    const { leaves } = this.column;
    if (leaves.valueType !== "float") {
      throw inconsistent();
    }
    return at(leaves.values, index);
  }

  string(index: number): string {
    const { leaves } = this.column;
    if (leaves.valueType !== "string") {
      throw inconsistent();
    }
    return at(leaves.values, index);
  }

  leaf(index: number, bigint: boolean): bigint | number | string {
    const { valueType } = this.column.leaves;
    if (valueType === "int") {
      return bigint ? this.int(index) : safeNumber(this.int(index), this.where);
    }
    return valueType === "float" ? this.float(index) : this.string(index);
  }

  /**
   * The selected value, built from the leaves outward one level at a time,
   * so that its depth is not limited by the call stack.
   */
  value(bigint: boolean): BigIntValue {
    const { levels } = this.column;
    // The rows that the selection spans at each level below it, outermost first.
    const spans: Range[] = [];
    let span: Range = { start: this.#row, end: this.#row + 1 };
    for (let level = this.#level; level < levels.length; level++) {
      spans.push(span);
      const offsets = at(levels, level);
      span = { start: at(offsets, span.start), end: at(offsets, span.end) };
    }
    const leaves = span;
    let values: BigIntValue[] = Array.from({ length: leaves.end - leaves.start }, (_, index) =>
      this.leaf(leaves.start + index, bigint),
    );
    for (let level = levels.length - 1; level >= this.#level; level--) {
      values = group(values, at(levels, level), at(spans, level - this.#level));
    }
    return at(values, 0);
  }
}

/**
 * Splits `elements`, the children of the rows in `rows`, into one array per
 * row according to the level's cumulative `offsets`.
 */
function group(elements: BigIntValue[], offsets: Uint32Array, rows: Range): BigIntValue[] {
  const base = at(offsets, rows.start);
  return Array.from({ length: rows.end - rows.start }, (_, index) =>
    elements.slice(
      at(offsets, rows.start + index) - base,
      at(offsets, rows.start + index + 1) - base,
    ),
  );
}

const LEAF_NAMES: Readonly<Record<ValueType, string>> = {
  int: "integer",
  float: "float",
  string: "string",
};

function safeNumber(value: bigint, where: string): number {
  if (value > BigInt(Number.MAX_SAFE_INTEGER) || value < BigInt(Number.MIN_SAFE_INTEGER)) {
    throw new RangeError(
      `${where}: integer ${value} is outside the safe range of number; use a bigint accessor`,
    );
  }
  return Number(value);
}

function at<T>(values: ArrayLike<T>, index: number): T {
  const value = values[index];
  if (value === undefined) {
    throw inconsistent();
  }
  return value;
}

/** A failed lookup that the decoded structure rules out. */
function inconsistent(): Error {
  return new Error("inconsistent data");
}

function describe(name: string, path: Path): string {
  return path.length === 0 ? `entry '${name}'` : `entry '${name}' at path [${path.join(", ")}]`;
}
