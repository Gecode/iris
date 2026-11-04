import { BoundSpec } from "./backend.js";
import { Data } from "./data.js";

/** Caller parameters: an object of strings, or name/value pairs applied in order. */
export type Params = Readonly<Record<string, string>> | Iterable<readonly [string, string]>;

export interface ReadOptions {
  readonly params?: Params;
}

const utf8 = new TextEncoder();

/**
 * A parsed and checked reader spec, read any number of times.
 *
 * Disposing a spec releases its WebAssembly memory early; later use throws
 * `TypeError`. A spec that is never disposed is released when it is
 * garbage-collected. A spec keeps its text and origin, so it keeps working
 * when the package replaces the WebAssembly instance.
 */
export class Spec {
  #bound: BoundSpec | undefined;

  private constructor(bound: BoundSpec) {
    this.#bound = bound;
  }

  /** Parses and checks a reader spec given as text or UTF-8 bytes. */
  static parse(text: string | Uint8Array, origin: string = "<string>"): Spec {
    // A copy, so the caller may reuse the bytes after parsing. The Uint8Array
    // constructor copies; a subclass's slice, such as Buffer's, may not.
    const source = text instanceof Uint8Array ? new Uint8Array(text) : toBytes(text, "spec text");
    return new Spec(new BoundSpec(source, checkedText(origin, "origin")));
  }

  /** Reads one instance from input text or bytes. */
  read(input: Uint8Array | string, origin: string = "<input>", options: ReadOptions = {}): Data {
    return this.#run("read", input, origin, options);
  }

  /** Lists the instances in an input, with the same arguments as `read`. */
  catalog(input: Uint8Array | string, origin: string = "<input>", options: ReadOptions = {}): Data {
    return this.#run("catalog", input, origin, options);
  }

  [Symbol.dispose](): void {
    this.#bound?.dispose();
    this.#bound = undefined;
  }

  #run(
    operation: "read" | "catalog",
    input: Uint8Array | string,
    origin: string,
    options: ReadOptions,
  ): Data {
    if (this.#bound === undefined) {
      throw new TypeError("the spec has been disposed");
    }
    const params = flattenParams(options.params);
    const bytes = toBytes(input, "input");
    return Data.fromTransfer(this.#bound[operation](bytes, checkedText(origin, "origin"), params));
  }
}

function toBytes(source: string | Uint8Array, name: string): Uint8Array {
  if (source instanceof Uint8Array) {
    return source;
  }
  return utf8.encode(checkedText(source, name));
}

/** Rejects lone surrogates, which UTF-8 would otherwise replace with U+FFFD. */
function checkedText(text: unknown, name: string): string {
  if (typeof text !== "string") {
    throw new TypeError(`${name} must be a string`);
  }
  if (!text.isWellFormed()) {
    throw new TypeError(`${name} is not well-formed Unicode`);
  }
  return text;
}

function flattenParams(params: Params | undefined): string[] {
  if (params === undefined) {
    return [];
  }
  if (typeof params !== "object" || params === null) {
    throw new TypeError("params must be an object or an iterable of name/value pairs");
  }
  const pairs: Iterable<readonly unknown[]> =
    Symbol.iterator in params ? params : Object.entries(params);
  const flattened: string[] = [];
  for (const pair of pairs) {
    if (!Array.isArray(pair) || pair.length !== 2) {
      throw new TypeError("each parameter must be a [name, value] pair");
    }
    const [name, value]: unknown[] = pair;
    flattened.push(
      checkedText(name, "a parameter name"),
      checkedText(value, `parameter '${String(name)}'`),
    );
  }
  return flattened;
}
