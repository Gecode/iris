// Conformance driver for the TypeScript package.
//
// Speaks the protocol in conformance/README.md, "Four driver commands",
// through the public gecode-iris API only, mirroring
// python/tests/conformance_driver.py and rust/iris-cli's iris-driver:
//
//   node test/conformance-driver.ts read SPEC INPUT --params '{"n": "3"}'
import { readFileSync } from "node:fs";
import { IrisError, Spec } from "gecode-iris";

const PARSE_SUCCESS = '{"iris":"0.1","parsed":true}\n';
const SUPPORTED_VERSION = "0.1";
const INPUT_OPERATIONS: ReadonlySet<string> = new Set(["read", "catalog", "dzn"]);

const EXIT_LANGUAGE_ERROR = 1;
const EXIT_MISUSE = 2;
const EXIT_HOST_FAILURE = 3;

/** The command line breaks the protocol. */
class Misuse extends Error {}

interface Request {
  readonly operation: string;
  readonly spec: string;
  /** The input path; `undefined` exactly for parse. */
  readonly input: string | undefined;
  readonly params: readonly (readonly [string, string])[];
  readonly dzn: boolean;
}

function main(args: readonly string[]): void {
  let request: Request;
  try {
    request = parseRequest(args);
  } catch (error) {
    if (error instanceof Misuse) {
      report(EXIT_MISUSE, error.message);
      return;
    }
    throw error;
  }
  try {
    process.stdout.write(successDocument(request));
  } catch (error) {
    if (error instanceof IrisError) {
      process.stdout.write(error.toJSON());
      process.exitCode = EXIT_LANGUAGE_ERROR;
    } else {
      report(
        EXIT_HOST_FAILURE,
        error instanceof Error ? `${error.name}: ${error.message}` : String(error),
      );
    }
  }
}

/** The success document: parse JSON, data JSON or dzn text. */
function successDocument(request: Request): string {
  const spec = Spec.parse(readFileSync(request.spec), request.spec);
  if (request.input === undefined) {
    return PARSE_SUCCESS;
  }
  const input = readFileSync(request.input);
  const options = { params: request.params };
  const data =
    request.operation === "catalog"
      ? spec.catalog(input, request.input, options)
      : spec.read(input, request.input, options);
  return request.dzn ? data.toDzn() : data.toJSON();
}

function parseRequest(args: readonly string[]): Request {
  const [operation, ...rest] = args;
  if (operation === undefined || (operation !== "parse" && !INPUT_OPERATIONS.has(operation))) {
    throw new Misuse("expected parse, read, catalog, or dzn");
  }
  const arity = operation === "parse" ? 1 : 2;
  const paths = rest.slice(0, arity);
  if (paths.length < arity || paths.some((path) => path.startsWith("--"))) {
    const expected = arity === 2 ? "a reader-spec path and an input path" : "a reader-spec path";
    throw new Misuse(`${operation} requires ${expected}`);
  }
  const allowed = new Set([
    "--params",
    "--version",
    ...(operation === "catalog" ? ["--format"] : []),
  ]);
  const options = parseOptions(rest.slice(arity), allowed);

  const version = options.get("--version") ?? SUPPORTED_VERSION;
  if (version !== SUPPORTED_VERSION) {
    throw new Misuse(`unsupported language version ${version}; supported: ${SUPPORTED_VERSION}`);
  }
  const params = parseParams(options.get("--params") ?? "{}");
  if (operation === "parse" && params.length > 0) {
    throw new Misuse("parse does not accept caller parameters");
  }
  const output = options.get("--format") ?? "json";
  if (output !== "json" && output !== "dzn") {
    throw new Misuse("expected catalog format json or dzn");
  }
  const [spec = "", input] = paths;
  return {
    operation,
    spec,
    input: arity === 2 ? input : undefined,
    params,
    dzn: operation === "dzn" || output === "dzn",
  };
}

/** Reads `--name value` pairs, each allowed name at most once. */
function parseOptions(args: readonly string[], allowed: ReadonlySet<string>): Map<string, string> {
  const options = new Map<string, string>();
  for (let index = 0; index < args.length; index += 2) {
    const name = args[index] ?? "";
    if (!allowed.has(name)) {
      throw new Misuse(`unexpected argument ${name}`);
    }
    if (options.has(name)) {
      throw new Misuse(`duplicate option ${name}`);
    }
    const value = args[index + 1];
    if (value === undefined) {
      throw new Misuse(`missing value for ${name}`);
    }
    options.set(name, value);
  }
  return options;
}

/**
 * Decodes a JSON object of strings with unique names, in source order.
 * JSON.parse validates the text, but its object keeps only the last of
 * duplicate names and moves integer-like keys first, so the members come
 * from the tokens of the validated text.
 */
function parseParams(text: string): [string, string][] {
  try {
    JSON.parse(text);
  } catch (error) {
    throw new Misuse(`invalid params: ${error instanceof Error ? error.message : String(error)}`);
  }
  const tokens = new JsonTokens(text);
  if (!tokens.accept("{")) {
    throw new Misuse("invalid params: expected a string-to-string parameter object");
  }
  const pairs: [string, string][] = [];
  const names = new Set<string>();
  if (!tokens.accept("}")) {
    do {
      const name = tokens.string();
      if (name === undefined || !tokens.accept(":")) {
        throw new Misuse("invalid params: expected a parameter name");
      }
      const value = tokens.string();
      if (value === undefined) {
        throw new Misuse(`invalid params: parameter '${name}' is not a string`);
      }
      if (names.has(name)) {
        throw new Misuse("invalid params: duplicate parameter");
      }
      if (!name.isWellFormed() || !value.isWellFormed()) {
        throw new Misuse("invalid params: parameters must be valid Unicode");
      }
      names.add(name);
      pairs.push([name, value]);
    } while (tokens.accept(","));
    if (!tokens.accept("}")) {
      throw new Misuse("invalid params: expected the end of the parameter object");
    }
  }
  if (!tokens.atEnd()) {
    throw new Misuse("invalid params: unexpected text after the parameter object");
  }
  return pairs;
}

/** The tokens of JSON text that JSON.parse has accepted, read from the front. */
class JsonTokens {
  static readonly #WHITESPACE = /[ \t\n\r]*/y;
  static readonly #STRING = /"(?:[^"\\]|\\.)*"/y;
  readonly #text: string;
  #position = 0;

  constructor(text: string) {
    this.#text = text;
  }

  /** Consumes `punctuation` if it is the next token. */
  accept(punctuation: string): boolean {
    this.#skipWhitespace();
    if (!this.#text.startsWith(punctuation, this.#position)) {
      return false;
    }
    this.#position += punctuation.length;
    return true;
  }

  /** Consumes and decodes the next token if it is a string. */
  string(): string | undefined {
    this.#skipWhitespace();
    const pattern = JsonTokens.#STRING;
    pattern.lastIndex = this.#position;
    const match = pattern.exec(this.#text);
    if (match === null) {
      return undefined;
    }
    this.#position = pattern.lastIndex;
    const decoded: unknown = JSON.parse(match[0]);
    return String(decoded);
  }

  atEnd(): boolean {
    this.#skipWhitespace();
    return this.#position === this.#text.length;
  }

  #skipWhitespace(): void {
    const pattern = JsonTokens.#WHITESPACE;
    pattern.lastIndex = this.#position;
    pattern.exec(this.#text);
    this.#position = pattern.lastIndex;
  }
}

function report(status: number, message: string): void {
  process.stderr.write(`conformance-driver: ${message}\n`);
  process.exitCode = status;
}

// Runs last, once the class declarations above are initialized.
main(process.argv.slice(2));
