// Line patterns: a string, the `p` tagged template, or `pattern(...parts)`.
import { Field, ownerOf, typeName } from "./expr.js";
import type { ValueTag } from "./ops.js";
import { scalarDeclared, Type, type Declared } from "./types.js";

export type Piece =
  | { readonly kind: "text"; readonly text: string }
  /** A handle from `capture`, printed from its declaration. */
  | { readonly kind: "capture"; readonly name: string; readonly owner: number }
  /** `{x:type}` through an existing handle. */
  | {
      readonly kind: "bind";
      readonly name: string;
      readonly owner: number;
      readonly value: ValueTag;
      readonly capture: Declared | null;
    }
  /** `{$x}`. */
  | { readonly kind: "reference"; readonly name: string; readonly owner: number }
  /** `{:type}`. */
  | { readonly kind: "validate"; readonly capture: Declared }
  /** `{}`. */
  | { readonly kind: "blank" }
  /** `{_}`. */
  | { readonly kind: "discard" }
  /** `{_:type}`. */
  | { readonly kind: "discardAs"; readonly capture: Declared }
  /** The trailing `...`. */
  | { readonly kind: "restOfLine" };

/** A part of a pattern other than text or a capture handle, such as `ref(h)`. */
export class Part {
  readonly #piece: Piece;

  private constructor(piece: Piece) {
    this.#piece = piece;
  }

  /** @internal */
  static create(piece: Piece): Part {
    return new Part(piece);
  }

  /** @internal */
  piece(): Piece {
    return this.#piece;
  }
}

/**
 * The discard `_`: a target that reads without storing, and `{_}` in a
 * pattern.
 */
export class Discard {
  static readonly #instance: Discard = new Discard();

  private constructor() {}

  /** @internal */
  static get instance(): Discard {
    return Discard.#instance;
  }
}

/** `_`: read a value without storing it, or `{_}` in a pattern. */
export function discard(): Discard {
  return Discard.instance;
}

/** What a pattern holds: text, capture handles, parts and `discard()`. */
export type PatternPart = string | Field | Part | Discard;

/** A line pattern, made by `p` or `pattern`. */
export class Pattern {
  readonly #pieces: readonly Piece[];

  private constructor(pieces: readonly Piece[]) {
    this.#pieces = pieces;
  }

  /** @internal */
  static create(parts: readonly unknown[]): Pattern {
    return new Pattern(joined(parts.map(pieceOf)));
  }

  /** @internal */
  pieces(): readonly Piece[] {
    return this.#pieces;
  }
}

/** The pattern of a line: a string, or a pattern from `p` or `pattern`. */
export type LinePattern = string | Pattern;

/**
 * A pattern as a tagged template: `` p`n = ${n}` ``. Each substitution is a
 * capture handle, a part such as `ref(h)`, `discard()`, or text.
 */
export function p(strings: TemplateStringsArray, ...parts: PatternPart[]): Pattern {
  const all: unknown[] = [];
  strings.forEach((text, index) => {
    if (typeof text !== "string") {
      throw new TypeError("a pattern's text has an invalid escape sequence");
    }
    all.push(text);
    if (index < parts.length) {
      all.push(parts[index]);
    }
  });
  return Pattern.create(all);
}

/** A pattern from text and parts, in order. */
export function pattern(...parts: PatternPart[]): Pattern {
  return Pattern.create(parts);
}

/** @internal The pieces of a line pattern. */
export function patternPieces(value: unknown): readonly Piece[] {
  if (typeof value === "string") {
    return joined([{ kind: "text", text: value }]);
  }
  if (value instanceof Pattern) {
    return value.pieces();
  }
  throw new TypeError(
    `a pattern is a string, or a pattern from \`p\` or \`pattern\`, not ${typeName(value)}`,
  );
}

/** @internal The type of a capture: any type without dimensions, or `null` for `{x}`. */
export function captureSpec(ty: unknown): Declared | null {
  return ty === undefined ? null : scalarDeclared(ty, "a capture's type");
}

/** @internal */
export function captureTag(ty: unknown): ValueTag {
  return ty instanceof Type ? ty.valueType : "string";
}

/**
 * `{x:type}`, binding a forward or redefined handle; without a type, `{x}`,
 * which reads a word.
 */
export function bind(handle: Field, ty?: Type<"int" | "float" | "num" | "string", 0>): Part {
  if (!(handle instanceof Field)) {
    throw new TypeError(`bind takes a handle, not ${typeName(handle)}`);
  }
  const capture = captureSpec(ty);
  return Part.create({
    kind: "bind",
    name: handle.name,
    owner: ownerOf(handle),
    value: captureTag(ty),
    capture,
  });
}

/** `{$x}`, matching the current value of `x`. */
export function ref(handle: Field): Part {
  if (!(handle instanceof Field)) {
    throw new TypeError(`ref takes a handle, not ${typeName(handle)}`);
  }
  return Part.create({ kind: "reference", name: handle.name, owner: ownerOf(handle) });
}

/** `{:type}`, which checks the text without storing it. */
export function validate(ty: Type<"int" | "float" | "num" | "string", 0>): Part {
  return Part.create({ kind: "validate", capture: scalarDeclared(ty, "a capture's type") });
}

/** `{}`, which matches a word without storing it. */
export function blank(): Part {
  return Part.create({ kind: "blank" });
}

/** `{_:type}`, a discarded capture with a type. */
export function discardAs(ty: Type<"int" | "float" | "num" | "string", 0>): Part {
  return Part.create({ kind: "discardAs", capture: scalarDeclared(ty, "a capture's type") });
}

/** The trailing `...` of a pattern. */
export const restOfLine: Part = Part.create({ kind: "restOfLine" });

function pieceOf(value: unknown): Piece {
  if (typeof value === "string") {
    return { kind: "text", text: value };
  }
  if (value instanceof Field) {
    return { kind: "capture", name: value.name, owner: ownerOf(value) };
  }
  if (value instanceof Part) {
    return value.piece();
  }
  if (value instanceof Discard) {
    return { kind: "discard" };
  }
  throw new TypeError(
    `a pattern part is text, a capture handle, a part such as ref(h), or discard(), not ${typeName(value)}`,
  );
}

/** The pieces with adjacent text joined and empty text dropped. */
function joined(pieces: readonly Piece[]): Piece[] {
  const result: Piece[] = [];
  for (const current of pieces) {
    const last = result.at(-1);
    if (current.kind === "text" && last?.kind === "text") {
      result[result.length - 1] = { kind: "text", text: last.text + current.text };
    } else if (!(current.kind === "text" && current.text === "")) {
      result.push(current);
    }
  }
  return result;
}
