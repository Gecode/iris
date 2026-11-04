/** A position in a reader spec or an input; each field is `null` when unknown. */
export interface Location {
  readonly origin: string | null;
  readonly line: number | null;
  readonly column: number | null;
}

/**
 * An Iris language error, from parsing a reader spec or from reading an input.
 *
 * The fields come from the canonical error document, which `toJSON()`
 * returns unchanged.
 */
export class IrisError extends Error {
  override readonly name = "IrisError" as const;
  /** The stable error code, such as `E-CHECK-FAILED`. */
  readonly code: string;
  /** The input location, for errors found while reading an input. */
  readonly inputLocation: Location | null;
  /** The reader spec location responsible for the error. */
  readonly specLocation: Location | null;
  /** What was being read, when known. */
  readonly context: string | null;
  /** The record indices of a failed check over records. */
  readonly recordPath: readonly number[] | null;
  /** The text `iris read` prints: location, message, input excerpt and caret. */
  readonly diagnostic: string;
  readonly #document: string;

  private constructor(fields: ErrorFields, document: string, diagnostic: string) {
    super(fields.message);
    this.code = fields.code;
    this.inputLocation = fields.inputLocation;
    this.specLocation = fields.specLocation;
    this.context = fields.context;
    this.recordPath = fields.recordPath;
    this.diagnostic = diagnostic;
    this.#document = document;
  }

  /** @internal Builds the error from the canonical document and its display text. */
  static fromDocument(document: string, diagnostic: string): IrisError {
    return new IrisError(parseDocument(document), document, diagnostic);
  }

  /** The canonical JSON error document. */
  toJSON(): string {
    return this.#document;
  }
}

interface ErrorFields {
  readonly code: string;
  readonly message: string;
  readonly inputLocation: Location | null;
  readonly specLocation: Location | null;
  readonly context: string | null;
  readonly recordPath: readonly number[] | null;
}

function parseDocument(document: string): ErrorFields {
  const fields: unknown = JSON.parse(document);
  if (typeof fields !== "object" || fields === null) {
    throw malformed("the document is not an object");
  }
  const member = (name: string): unknown => (name in fields ? Reflect.get(fields, name) : null);
  const recordPath = member("record_path");
  return {
    code: requireString(member("code"), "code"),
    message: requireString(member("message"), "message"),
    inputLocation: parseLocation(member("input_location")),
    specLocation: parseLocation(member("spec_location")),
    context: optionalString(member("context"), "context"),
    recordPath:
      recordPath === null
        ? null
        : Object.freeze(requireArray(recordPath, "record_path").map(requireIndex)),
  };
}

function parseLocation(location: unknown): Location | null {
  if (location === null) {
    return null;
  }
  if (typeof location !== "object") {
    throw malformed("a location is not an object");
  }
  const member = (name: string): unknown => (name in location ? Reflect.get(location, name) : null);
  return Object.freeze({
    origin: optionalString(member("origin"), "origin"),
    line: optionalIndex(member("line")),
    column: optionalIndex(member("column")),
  });
}

function requireString(value: unknown, name: string): string {
  if (typeof value !== "string") {
    throw malformed(`${name} is not a string`);
  }
  return value;
}

function optionalString(value: unknown, name: string): string | null {
  return value === null ? null : requireString(value, name);
}

function requireArray(value: unknown, name: string): unknown[] {
  if (!Array.isArray(value)) {
    throw malformed(`${name} is not an array`);
  }
  return value;
}

function requireIndex(value: unknown): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw malformed("an index is not a non-negative integer");
  }
  return value;
}

function optionalIndex(value: unknown): number | null {
  return value === null ? null : requireIndex(value);
}

function malformed(problem: string): Error {
  return new Error(`malformed error document: ${problem}`);
}
