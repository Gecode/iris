// The printer. It writes what `iris fmt` writes with no line limit, so the
// text a builder emits is already formatted.
import { CallSites, type CallSite } from "../call-sites.js";
import { BuildError } from "./error.js";
import { operandCount, type Binding, type Op, type Ops } from "./ops.js";
import type { Piece } from "./pattern.js";
import {
  continuesIf,
  declaredMentions,
  expressions,
  isIdentifier,
  isOps,
  opensBlock,
  type Definition,
  type State,
  type Stmt,
} from "./statement.js";
import { isOpenEnded, type AlphabetOption, type Declared } from "./types.js";

/**
 * The words that end a range with no upper bound, from the Specification's
 * `upper bound ahead` predicate.
 */
const STOP_SET: ReadonlySet<string> = new Set([
  "comment",
  "stop",
  "expect",
  "tokens",
  "separators",
  "ignore",
  "param",
  "list",
  "line",
  "find",
  "skip",
  "index",
  "repeat",
  "each",
  "instances",
  "sections",
  "if",
  "let",
  "check",
  "type",
  "layout",
  "offset",
  "until",
]);

/** The words the type dispatch takes before a range. */
const TYPE_WORDS: ReadonlySet<string> = new Set([
  "int",
  "float",
  "num",
  "word",
  "rest",
  "alphabet",
]);

// The Specification's ten precedence levels, loosest first.
const DISJUNCTION = 1;
const CONJUNCTION = 2;
const EQUALITY = 3;
const RELATION = 4;
const ADDITIVE = 5;
const MULTIPLICATIVE = 6;
const UNARY = 7;
const POWER = 8;
const POSTFIX = 9;
const PRIMARY = 10;

const BINARY_LEVELS: Readonly<Record<string, number>> = {
  "||": DISJUNCTION,
  "&&": CONJUNCTION,
  "==": EQUALITY,
  "!=": EQUALITY,
  "<": RELATION,
  "<=": RELATION,
  ">": RELATION,
  ">=": RELATION,
  "+": ADDITIVE,
  "-": ADDITIVE,
  "*": MULTIPLICATIVE,
  "/": MULTIPLICATIVE,
  "%": MULTIPLICATIVE,
  "^": POWER,
};
const INT_MIN = -(2n ** 63n);
const INT_MAX = 2n ** 63n - 1n;

/** The emitted text, and where each of its statements came from. */
export interface Printed {
  readonly text: string;
  readonly sites: CallSites;
}

/** Prints the statements as text. */
export function printSpec(state: State): Printed {
  const printer = new Printer(state, assignNames(state));
  printer.statements();
  if (printer.issue !== null) {
    throw printer.issue;
  }
  return printer.printed();
}

function binaryLevel(symbol: string): number {
  return BINARY_LEVELS[symbol] ?? PRIMARY;
}

function level(op: Op): number {
  switch (op.kind) {
    case "binary":
      return binaryLevel(op.symbol);
    case "int":
      return op.value < 0n && op.value !== INT_MIN ? UNARY : PRIMARY;
    case "float":
      return op.value < 0 || Object.is(op.value, -0) ? UNARY : PRIMARY;
    case "unary":
      return UNARY;
    case "index":
    case "member":
    case "listMember":
      return POSTFIX;
    default:
      return PRIMARY;
  }
}

function at<T>(values: readonly T[], index: number): T {
  const value = values[index];
  if (value === undefined) {
    throw new Error("the printer lost track of an expression");
  }
  return value;
}

/** Where each subexpression starts, indexed by the operation that ends it. */
function extents(ops: Ops): number[] {
  const starts: number[] = [];
  const stack: number[] = [];
  ops.forEach((op, index) => {
    const first = stack.length - operandCount(op);
    const start = first < stack.length ? at(stack, first) : index;
    stack.length = first;
    stack.push(start);
    starts.push(start);
  });
  return starts;
}

/** The last operation of each of the `count` operands before `end`. */
function operands(starts: readonly number[], end: number, count: number): number[] {
  const ends: number[] = [];
  let following = end;
  for (let index = 0; index < count; index++) {
    following -= 1;
    ends.push(following);
    following = at(starts, following);
  }
  return ends.toReversed();
}

/**
 * A finite float as Rust's `{:?}` writes it: the shortest digits that read
 * back, in decimal from 1e-4 up to 1e16 and in exponent form outside, with
 * `.0` on a whole number in decimal.
 */
export function floatText(value: number): string {
  const sign = value < 0 || Object.is(value, -0) ? "-" : "";
  const [mantissa = "0", exponentText = "0"] = Math.abs(value).toExponential().split("e");
  const digits = mantissa.replace(".", "");
  const exponent = Number(exponentText);
  if (value === 0) {
    return `${sign}0.0`;
  }
  if (exponent < -4 || exponent >= 16) {
    return `${sign}${mantissa}e${exponent}`;
  }
  if (exponent < 0) {
    return `${sign}0.${"0".repeat(-exponent - 1)}${digits}`;
  }
  const whole = digits.slice(0, exponent + 1).padEnd(exponent + 1, "0");
  const fraction = digits.slice(exponent + 1);
  return `${sign}${whole}.${fraction === "" ? "0" : fraction}`;
}

function quoted(text: string): string {
  const escaped = text.replaceAll("\\", "\\\\").replaceAll('"', '\\"');
  return `"${escaped.replaceAll("\n", "\\n").replaceAll("\t", "\\t")}"`;
}

function firstWord(text: string): string {
  return /^[A-Za-z0-9_]*/u.exec(text)?.[0] ?? "";
}

/**
 * Whether a format name can be written without quotes: names and integers
 * glued with `-` or `.`, starting with a name.
 */
export function isBareFormat(name: string): boolean {
  let rest = name;
  let first = true;
  for (;;) {
    const separators = [rest.indexOf("-"), rest.indexOf(".")].filter((index) => index >= 0);
    const end = separators.length === 0 ? rest.length : Math.min(...separators);
    const part = rest.slice(0, end);
    const tail = rest.slice(end);
    const integer = /^[0-9]+$/u.test(part) && BigInt(part) <= INT_MAX;
    if (!(isIdentifier(part) || (!first && integer && !tail.startsWith(".")))) {
      return false;
    }
    if (tail === "") {
      return true;
    }
    rest = tail.slice(1);
    first = false;
  }
}

/**
 * Every expression the text of `stmt` writes: those it mentions, and the
 * types of the captures its pattern names, which print where they bind.
 */
function* printedExpressions(state: State, stmt: Stmt): Generator<Ops> {
  yield* expressions(stmt);
  if (stmt.kind !== "line" && stmt.kind !== "arm") {
    return;
  }
  for (const piece of stmt.pieces) {
    if (piece.kind === "capture") {
      const capture = state.names.get(piece.name);
      if (capture?.kind === "capture" && capture.spec !== null) {
        for (const mention of declaredMentions(capture.spec)) {
          if (isOps(mention)) {
            yield mention;
          }
        }
      }
    }
  }
}

/** Internal names, chosen after the whole stream is known. */
interface Names {
  readonly each: Map<number, string>;
  readonly list: Map<number, string>;
  readonly bindings: Map<Binding, string>;
}

/**
 * Chooses names in stream order, skipping every name in the spec, so the same
 * calls always give the same text.
 */
function assignNames(state: State): Names {
  const needsEach = new Set<number>();
  const needsList = new Set<number>();
  const taken = new Set<string>([...state.names.keys(), ...state.types]);
  for (const record of state.records) {
    for (const name of [record.each, record.list]) {
      if (name !== null) {
        taken.add(name);
      }
    }
  }
  for (const stmt of state.stmts) {
    for (const ops of printedExpressions(state, stmt)) {
      for (const op of ops) {
        if (op.kind === "record") {
          needsEach.add(state.recordId(op.record));
        } else if (op.kind === "recordList" || op.kind === "listMember") {
          needsList.add(state.recordId(op.record));
        } else if (op.kind === "comprehension" && op.name !== null) {
          taken.add(op.name);
        }
      }
    }
  }
  const fresh = new Fresh(taken);
  const names: Names = { each: new Map(), list: new Map(), bindings: new Map() };
  for (const stmt of state.stmts) {
    if (stmt.kind === "repeat") {
      const recordId = state.recordId(stmt.record);
      const info = state.records[recordId];
      if (info?.each === null && needsEach.has(recordId)) {
        names.each.set(recordId, fresh.name("record"));
      }
      if (info?.list === null && needsList.has(recordId)) {
        names.list.set(recordId, fresh.name("records"));
      }
    }
    for (const ops of printedExpressions(state, stmt)) {
      nameBindings(names, ops, fresh);
    }
  }
  return names;
}

function nameBindings(names: Names, ops: Ops, fresh: Fresh): void {
  for (const op of ops) {
    if (op.kind === "comprehension" && op.name !== null) {
      names.bindings.set(op.binding, op.name);
    }
  }
  for (const op of ops) {
    if ((op.kind === "comprehension" || op.kind === "binding") && !names.bindings.has(op.binding)) {
      names.bindings.set(op.binding, fresh.name("element"));
    }
  }
}

class Fresh {
  readonly #taken: Set<string>;
  readonly #next: Map<string, number> = new Map();

  constructor(taken: Set<string>) {
    this.#taken = taken;
  }

  name(prefix: string): string {
    let number = this.#next.get(prefix) ?? 1;
    for (;;) {
      const name = `${prefix}_${number}`;
      number += 1;
      if (!this.#taken.has(name)) {
        this.#taken.add(name);
        this.#next.set(prefix, number);
        return name;
      }
    }
  }
}

type Task =
  | { readonly kind: "text"; readonly text: string }
  | { readonly kind: "expression"; readonly end: number; readonly minimum: number }
  /** `.name` after the operand just written. */
  | { readonly kind: "member"; readonly name: string }
  /** `.records` after the operand just written, with a repeat's `as` name. */
  | { readonly kind: "listMember"; readonly record: number };

class Printer {
  readonly state: State;
  readonly names: Names;
  readonly out: string[] = [];
  /** The UTF-16 code units written so far. */
  length = 0;
  /** The offset of each statement, for its call site. */
  readonly starts: number[] = [];
  readonly sites: (CallSite | null)[] = [];
  /** The index of the statement being printed. */
  current = 0;
  indent = 0;
  issue: BuildError | null = null;

  constructor(state: State, names: Names) {
    this.state = state;
    this.names = names;
  }

  write(text: string): void {
    this.out.push(text);
    this.length += text.length;
  }

  captureOutput(): number {
    return this.out.length;
  }

  /** Removes and returns what was written since `mark`. */
  takeOutput(mark: number): string {
    const text = this.out.splice(mark).join("");
    this.length -= text.length;
    return text;
  }

  /** The last `count` code units written, or fewer at the start. */
  tail(count: number): string {
    const parts: string[] = [];
    let gathered = 0;
    for (let index = this.out.length - 1; index >= 0 && gathered < count; index--) {
      const chunk = at(this.out, index);
      parts.push(chunk);
      gathered += chunk.length;
    }
    return parts.toReversed().join("").slice(-count);
  }

  markSite(index: number): void {
    this.starts.push(this.length);
    this.sites.push(this.state.sites[index] ?? null);
  }

  printed(): Printed {
    const text = this.out.join("");
    return { text, sites: new CallSites(byteOffsets(text, this.starts), this.sites) };
  }

  unprintable(message: string): void {
    this.issue ??= BuildError.check("unprintable", message, this.state.sites[this.current] ?? null);
  }

  startLine(): void {
    this.write("  ".repeat(this.indent));
  }

  statements(): void {
    const { stmts } = this.state;
    let index = 0;
    let continuing = false;
    while (index < stmts.length) {
      const stmt = at(stmts, index);
      this.current = index;
      this.markSite(index);
      index += 1;
      if (stmt.kind === "close") {
        this.indent -= 1;
        this.startLine();
        this.write("}");
        continuing = this.afterClose(stmts, index);
        continue;
      }
      if (!continuing) {
        this.startLine();
      }
      continuing = false;
      if (stmt.kind === "format" && index >= 2 && endsOpen(at(stmts, index - 2))) {
        this.unprintable(
          "a range with no upper bound cannot come directly before `format`: " +
            "`format` would be its upper bound",
        );
      }
      this.statement(stmt);
      if (!opensBlock(stmt)) {
        this.write("\n");
        continue;
      }
      const each = this.eachLine(stmt);
      if (each === null && stmts[index]?.kind === "close") {
        this.write(" {");
        this.markSite(index);
        this.write("}");
        index += 1;
        continuing = this.afterClose(stmts, index);
        continue;
      }
      this.write(" {\n");
      this.indent += 1;
      if (each !== null) {
        this.startLine();
        this.write(`each ${each}\n`);
      }
    }
  }

  /** Ends the line after `}`, unless an `else` continues it. */
  afterClose(stmts: readonly Stmt[], following: number): boolean {
    const next = stmts[following];
    const continues = next !== undefined && continuesIf(next);
    this.write(continues ? " " : "\n");
    return continues;
  }

  recordEach(record: number): string | null {
    const id = this.state.recordId(record);
    return this.state.records[id]?.each ?? this.names.each.get(id) ?? null;
  }

  recordList(record: number): string | null {
    const id = this.state.recordId(record);
    return this.state.records[id]?.list ?? this.names.list.get(id) ?? null;
  }

  eachLine(stmt: Stmt): string | null {
    return stmt.kind === "repeat" ? this.recordEach(stmt.record) : null;
  }

  statement(stmt: Stmt): void {
    switch (stmt.kind) {
      case "format":
        this.write(`format ${isBareFormat(stmt.name) ? stmt.name : quoted(stmt.name)}`);
        break;
      case "strings":
        this.write(stmt.keyword);
        this.strings(stmt.strings);
        break;
      case "ignoreTrailing":
        this.write("ignore trailing");
        break;
      case "param":
        this.write("param ");
        this.target(stmt.definition);
        if (stmt.default !== null) {
          this.write(" = ");
          this.expression(stmt.default);
        }
        break;
      case "declaration":
        if (stmt.definition.name === "format" && this.indent === 0) {
          this.unprintable(
            "an entry named `format` has no spelling at the top level: " +
              "use a `let` or a `list`, or another name",
          );
        }
        this.declaration(stmt.definition, stmt.declared);
        break;
      case "elseDeclaration":
        this.write("else ");
        this.declaration(stmt.definition, stmt.declared);
        break;
      case "list":
        this.list(stmt.definition, stmt.declared, stmt.end);
        break;
      case "type":
        this.write(`type ${stmt.name} = `);
        this.typeWithOptions(stmt.declared);
        break;
      case "line":
        this.write(`${stmt.keyword} `);
        this.pattern(stmt.pieces);
        break;
      case "skipLine":
        this.write("skip line");
        break;
      case "skipLines":
        this.write("skip ");
        this.guarded(stmt.count, new Set(["line"]));
        this.write(" lines");
        break;
      case "index":
        this.write("index ");
        this.expression(stmt.position);
        break;
      case "let":
        this.write("let ");
        this.target(stmt.definition);
        this.write(" = ");
        this.expression(stmt.value);
        break;
      case "check":
        this.write("check ");
        this.expression(stmt.condition);
        if (stmt.message !== null) {
          this.write(` ${quoted(stmt.message)}`);
        }
        break;
      case "repeat":
        this.repeat(stmt);
        break;
      case "sections":
        this.write("sections");
        for (const clause of stmt.clauses) {
          if (clause.kind === "skipOther") {
            this.write(" skip other");
          } else {
            this.write(" until");
            this.strings(clause.terminators);
            this.store(clause.store);
          }
        }
        break;
      case "arm":
        this.pattern(stmt.pieces);
        this.write(stmt.optional ? " optional" : "");
        this.write(stmt.last ? " last" : "");
        break;
      case "if":
        this.write("if ");
        this.expression(stmt.condition);
        break;
      case "elseIf":
        this.write("else if ");
        this.expression(stmt.condition);
        break;
      case "else":
        this.write("else");
        break;
      case "instances":
        this.write("instances");
        break;
      case "close":
        break;
    }
  }

  strings(strings: readonly string[]): void {
    for (const text of strings) {
      this.write(` ${quoted(text)}`);
    }
  }

  target(definition: Definition): void {
    this.write(definition.name ?? "_");
  }

  declaration(definition: Definition, declared: Declared): void {
    this.target(definition);
    for (const dimension of declared.dimensions) {
      this.write("[");
      this.expression(dimension);
      this.write("]");
    }
    this.write(": ");
    this.typeWithOptions(declared);
  }

  list(definition: Definition, declared: Declared, end: Ops | null): void {
    if (declared.layout !== null) {
      this.unprintable("a `list` has no `layout`");
    }
    this.write("list ");
    this.target(definition);
    this.write(": ");
    this.typeName(declared);
    this.write(" until ");
    if (end === null) {
      this.write("eol");
    } else {
      this.guarded(end, new Set(["eol"]));
    }
    this.options(declared);
  }

  typeWithOptions(declared: Declared): void {
    this.typeName(declared);
    this.options(declared);
  }

  options(declared: Declared): void {
    for (const [keyword, option] of [
      [" layout ", declared.layout],
      [" offset ", declared.offset],
    ] as const) {
      if (option !== null) {
        this.write(keyword);
        this.expression(option);
      }
    }
  }

  typeName(declared: Declared): void {
    const { ty } = declared;
    switch (ty.kind) {
      case "base":
      case "named":
        this.write(ty.name);
        break;
      case "alphabet":
        this.write(`alphabet ${quoted(ty.symbols)}`);
        for (const option of ty.options) {
          this.alphabetOption(option);
        }
        break;
      case "range":
        if (ty.lower !== null) {
          this.lowerBound(ty.lower);
        }
        this.write(ty.inclusive ? ".." : "..<");
        if (ty.upper !== null) {
          this.guarded(ty.upper, STOP_SET);
        }
        break;
    }
  }

  alphabetOption(option: AlphabetOption): void {
    switch (option.kind) {
      case "blank":
        this.write(` blank ${quoted(option.characters)}`);
        break;
      case "strip":
        this.write(` strip ${quoted(option.characters)}`);
        break;
      case "base":
        if (option.value === INT_MIN) {
          this.unprintable("the alphabet base -9223372036854775808 has no spelling");
        } else {
          this.write(` base ${option.value}`);
        }
        break;
    }
  }

  lowerBound(ops: Ops): void {
    const text = this.expressionText(ops);
    const word = firstWord(text);
    this.write(TYPE_WORDS.has(word) || this.state.types.has(word) ? `(${text})` : text);
  }

  /**
   * Writes an expression in parentheses if its first word is one of `words`,
   * which would otherwise be read as a keyword.
   */
  guarded(ops: Ops, words: ReadonlySet<string>): void {
    const text = this.expressionText(ops);
    this.write(words.has(firstWord(text)) ? `(${text})` : text);
  }

  expressionText(ops: Ops): string {
    const mark = this.captureOutput();
    this.expression(ops);
    return this.takeOutput(mark);
  }

  repeat(stmt: Extract<Stmt, { readonly kind: "repeat" }>): void {
    this.write("repeat ");
    const { count } = stmt;
    if (count.kind === "times") {
      this.guarded(count.count, new Set(["until"]));
    } else if (count.kind === "untilEof") {
      this.write("until eof");
    } else {
      this.write("until");
      this.strings(count.terminators);
      this.store(count.store);
    }
    const records = this.recordList(stmt.record);
    if (records !== null) {
      this.write(` as ${records}`);
    }
  }

  store(store: Definition | null): void {
    if (store !== null) {
      this.write(" store ");
      this.target(store);
    }
  }

  captureType(capture: Declared | null): string | null {
    if (capture === null) {
      return null;
    }
    const mark = this.captureOutput();
    this.typeWithOptions(capture);
    const text = this.takeOutput(mark);
    if (text.includes("}")) {
      this.unprintable(`the capture type \`${text}\` contains \`}\``);
    }
    return text;
  }

  pattern(pieces: readonly Piece[]): void {
    const text: string[] = [];
    for (const piece of pieces) {
      switch (piece.kind) {
        case "text":
          if (piece.text.includes("{") || piece.text.includes("...")) {
            this.unprintable(
              `the pattern text \`${piece.text}\` contains \`{\` or \`...\`, ` +
                "which have no literal spelling",
            );
          }
          text.push(piece.text);
          break;
        case "capture": {
          const state = this.state.names.get(piece.name);
          text.push(this.namedCapture(piece.name, state?.kind === "capture" ? state.spec : null));
          break;
        }
        case "bind":
          text.push(this.namedCapture(piece.name, piece.capture));
          break;
        case "reference":
          text.push(`{$${piece.name}}`);
          break;
        case "validate":
          text.push(this.namedCapture("", piece.capture));
          break;
        case "blank":
          text.push("{}");
          break;
        case "discard":
          text.push("{_}");
          break;
        case "discardAs":
          text.push(this.namedCapture("_", piece.capture));
          break;
        case "restOfLine":
          text.push("...");
          break;
      }
    }
    this.write(quoted(text.join("")));
  }

  namedCapture(name: string, capture: Declared | null): string {
    const ty = this.captureType(capture);
    return ty === null ? `{${name}}` : `{${name}:${ty}}`;
  }

  literal(op: Extract<Op, { readonly kind: "int" | "float" | "string" }>): void {
    if (op.kind === "int") {
      this.write(op.value === INT_MIN ? "(-9223372036854775807 - 1)" : String(op.value));
    } else if (op.kind === "float") {
      if (Number.isFinite(op.value)) {
        this.write(floatText(op.value));
      } else {
        this.unprintable(`the float ${rustFloat(op.value)} has no spelling`);
      }
    } else {
      this.write(quoted(op.text));
    }
  }

  /**
   * Writes `.name` after the operand just written. `gecode.x` is one host
   * constant token, so after a whole name `gecode` the dot keeps its spaces.
   */
  member(name: string): void {
    const before = this.tail(7);
    const whole =
      before.endsWith("gecode") && !(before.length === 7 && /^[A-Za-z0-9_]/u.test(before));
    this.write((whole ? " . " : ".") + name);
  }

  expression(ops: Ops): void {
    const starts = extents(ops);
    const tasks: Task[] = [{ kind: "expression", end: ops.length - 1, minimum: DISJUNCTION }];
    for (let task = tasks.pop(); task !== undefined; task = tasks.pop()) {
      switch (task.kind) {
        case "text":
          this.write(task.text);
          break;
        case "expression":
          if (level(at(ops, task.end)) < task.minimum) {
            this.write("(");
            tasks.push({ kind: "text", text: ")" });
          }
          this.operation(ops, starts, task.end, tasks);
          break;
        case "member":
          this.member(task.name);
          break;
        case "listMember":
          this.member(this.recordList(task.record) ?? "");
          break;
      }
    }
  }

  operation(ops: Ops, starts: readonly number[], end: number, tasks: Task[]): void {
    const op = at(ops, end);
    switch (op.kind) {
      case "binary": {
        const [left = 0, right = 0] = operands(starts, end, 2);
        const symbolLevel = binaryLevel(op.symbol);
        const [leftMinimum, rightMinimum] =
          op.symbol === "^" ? [POSTFIX, UNARY] : [symbolLevel, symbolLevel + 1];
        tasks.push({ kind: "expression", end: right, minimum: rightMinimum });
        tasks.push({ kind: "text", text: ` ${op.symbol} ` });
        tasks.push({ kind: "expression", end: left, minimum: leftMinimum });
        break;
      }
      case "name":
        this.write(op.text);
        break;
      case "int":
      case "float":
      case "string":
        this.literal(op);
        break;
      case "host":
        this.write(op.name);
        break;
      case "binding":
        this.write(this.names.bindings.get(op.binding) ?? "");
        break;
      case "record":
        this.write(this.recordEach(op.record) ?? "");
        break;
      case "recordList":
        this.write(this.recordList(op.record) ?? "");
        break;
      case "unary":
        this.write(op.symbol);
        tasks.push({ kind: "expression", end: end - 1, minimum: UNARY });
        break;
      case "index": {
        const [base = 0, index = 0] = operands(starts, end, 2);
        tasks.push({ kind: "text", text: "]" });
        tasks.push({ kind: "expression", end: index, minimum: DISJUNCTION });
        tasks.push({ kind: "text", text: "[" });
        tasks.push({ kind: "expression", end: base, minimum: POSTFIX });
        break;
      }
      case "member":
        tasks.push({ kind: "member", name: op.text });
        this.memberBase(ops, end - 1, tasks);
        break;
      case "listMember":
        tasks.push({ kind: "listMember", record: op.record });
        this.memberBase(ops, end - 1, tasks);
        break;
      case "call": {
        this.write(`${op.function}(`);
        tasks.push({ kind: "text", text: ")" });
        const args = operands(starts, end, op.arguments);
        for (let position = args.length - 1; position >= 0; position--) {
          tasks.push({ kind: "expression", end: at(args, position), minimum: DISJUNCTION });
          if (position > 0) {
            tasks.push({ kind: "text", text: ", " });
          }
        }
        break;
      }
      case "comprehension": {
        const [domain = 0, body = 0] = operands(starts, end, 2);
        const name = this.names.bindings.get(op.binding) ?? "";
        if (!isIdentifier(name)) {
          this.unprintable(`\`${name}\` is not an Iris name`);
        }
        this.write(`${op.function}(${name} in `);
        tasks.push({ kind: "text", text: ")" });
        tasks.push({ kind: "expression", end: body, minimum: DISJUNCTION });
        tasks.push({ kind: "text", text: ": " });
        tasks.push({ kind: "expression", end: domain, minimum: DISJUNCTION });
        break;
      }
    }
  }

  /**
   * A number before `.` would lex as a malformed number, so it takes
   * parentheses at any level.
   */
  memberBase(ops: Ops, base: number, tasks: Task[]): void {
    const kind = at(ops, base).kind;
    const minimum = kind === "int" || kind === "float" ? PRIMARY + 1 : POSTFIX;
    tasks.push({ kind: "expression", end: base, minimum });
  }
}

function rustFloat(value: number): string {
  if (Number.isNaN(value)) {
    return "NaN";
  }
  return value > 0 ? "inf" : "-inf";
}

function endsOpen(stmt: Stmt): boolean {
  if (stmt.kind === "declaration" || stmt.kind === "elseDeclaration" || stmt.kind === "type") {
    return isOpenEnded(stmt.declared);
  }
  return false;
}

const utf8 = new TextEncoder();

function isAscii(text: string): boolean {
  for (let index = 0; index < text.length; index++) {
    if (text.charCodeAt(index) > 0x7f) {
      return false;
    }
  }
  return true;
}

/** Offsets in UTF-16 code units of `text`, which ascend, as UTF-8 byte offsets. */
function byteOffsets(text: string, offsets: readonly number[]): number[] {
  if (isAscii(text)) {
    return [...offsets];
  }
  const result: number[] = [];
  let previous = 0;
  let total = 0;
  for (const offset of offsets) {
    total += utf8.encode(text.slice(previous, offset)).length;
    previous = offset;
    result.push(total);
  }
  return result;
}
