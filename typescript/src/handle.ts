// Typed handles: keys into Data that fix the host type of the entry they
// name. The spec builder in gecode-iris/build returns them, and Data.get
// reads through them.
import type { BigIntValue, Value } from "./data.js";

/**
 * The value type a handle promises: `num` is an int or a float, decided by
 * the data, and `dyn` is any value type.
 */
export type HandleValue = "int" | "float" | "num" | "string" | "dyn";

/** The number of array levels a handle promises, or `null` when it is open. */
export type HandleDepth = 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | null;

/** Whether an entry always exists after a successful read. */
export type Presence = "required" | "maybe";

/**
 * The name of an entry, with the value type, depth and presence it promises.
 * `data.get(handle)` checks the promise and returns the entry with the type
 * it gives.
 */
export interface Handle<
  V extends HandleValue = HandleValue,
  D extends HandleDepth = HandleDepth,
  P extends Presence = Presence,
> {
  readonly name: string;
  readonly valueType: V;
  readonly depth: D;
  readonly presence: P;
}

type Leaf<V extends HandleValue> = [V] extends ["string"]
  ? string
  : [V] extends ["int" | "float" | "num"]
    ? number
    : Value;

type BigIntLeaf<V extends HandleValue> = [V] extends ["string"]
  ? string
  : [V] extends ["float"]
    ? number
    : [V] extends ["int"]
      ? bigint
      : [V] extends ["int" | "float" | "num"]
        ? bigint | number
        : BigIntValue;

type Levels<T> = [T, T[], T[][], T[][][], T[][][][], T[][][][][], T[][][][][][]];
type DeepLevels<T> = [T[][][][][][][], T[][][][][][][][]];

/** `T` nested in `D` arrays. */
type Nested<T, D extends HandleDepth> = D extends 0 | 1 | 2 | 3 | 4 | 5 | 6
  ? Levels<T>[D]
  : D extends 7
    ? DeepLevels<T>[0]
    : D extends 8
      ? DeepLevels<T>[1]
      : never;

type Present<P extends Presence, T> = [P] extends ["required"] ? T : T | undefined;

/**
 * The value of an entry with leaves `L` and depth `D`: `Value` when the depth
 * may be open, else the union of the views of the possible depths.
 */
type Shaped<L, D extends HandleDepth, Open> = null extends D ? Open : Nested<L, D>;

/** What `data.get(handle)` returns, with integers as `number`. */
export type ViewOf<H extends Handle> =
  H extends Handle<infer V, infer D, infer P>
    ? Present<P, [V] extends ["dyn"] ? Value : Shaped<Leaf<V>, D, Value>>
    : never;

/** What `data.get(handle, { bigint: true })` returns, with integers as `bigint`. */
export type BigIntViewOf<H extends Handle> =
  H extends Handle<infer V, infer D, infer P>
    ? Present<P, [V] extends ["dyn"] ? BigIntValue : Shaped<BigIntLeaf<V>, D, BigIntValue>>
    : never;

const HANDLE_VALUES: ReadonlySet<unknown> = new Set(["int", "float", "num", "string", "dyn"]);

/**
 * Whether `value` is a handle: an object with a string `name`, a value type,
 * a depth from 0 to 8 or `null`, and a presence.
 */
export function isHandle(value: unknown): value is Handle {
  if (typeof value !== "object" || value === null) {
    return false;
  }
  const depth: unknown = Reflect.get(value, "depth");
  const presence: unknown = Reflect.get(value, "presence");
  return (
    typeof Reflect.get(value, "name") === "string" &&
    HANDLE_VALUES.has(Reflect.get(value, "valueType")) &&
    (depth === null ||
      (typeof depth === "number" && Number.isInteger(depth) && depth >= 0 && depth <= 8)) &&
    (presence === "required" || presence === "maybe")
  );
}
