// The statement stream: one flat list in text order, with blocks marked by
// openers and `close`, so that every pass is a loop over it.
import type { CallSite } from "../call-sites.js";
import { BuildError, type BuildErrorKind } from "./error.js";
import type { Ops, ValueTag } from "./ops.js";
import type { Piece } from "./pattern.js";
import type { Declared, TypeSpec } from "./types.js";

let owners = 0;

/** A new builder id. It only compares handles; it is never printed. */
export function nextOwner(): number {
  owners += 1;
  return owners;
}

/**
 * What a definition defines: `name` is `null` for the discard `_`. `levels`
 * counts the definition's own array levels, or is `null` when they are open.
 */
export interface Definition {
  readonly name: string | null;
  readonly value: ValueTag;
  readonly levels: number | null;
}

export interface Until {
  readonly kind: "until";
  readonly terminators: readonly string[];
  store: Definition | null;
}

export type RepeatCount =
  | { readonly kind: "times"; readonly count: Ops }
  | { readonly kind: "untilEof" }
  | Until;

export type SectionClause = Until | { readonly kind: "skipOther" };

export type Stmt =
  | { readonly kind: "format"; readonly name: string }
  | {
      readonly kind: "strings";
      readonly keyword: "comment" | "stop" | "tokens" | "separators" | "expect";
      readonly strings: readonly string[];
    }
  | { readonly kind: "ignoreTrailing" }
  | { readonly kind: "param"; readonly definition: Definition; readonly default: Ops | null }
  | { readonly kind: "declaration"; readonly definition: Definition; readonly declared: Declared }
  | {
      readonly kind: "list";
      readonly definition: Definition;
      readonly declared: Declared;
      /** The sentinel, or `null` for `until eol`. */
      readonly end: Ops | null;
    }
  | { readonly kind: "type"; readonly name: string; readonly declared: Declared }
  | { readonly kind: "line"; readonly keyword: "line" | "find"; readonly pieces: readonly Piece[] }
  | { readonly kind: "skipLine" }
  | { readonly kind: "skipLines"; readonly count: Ops }
  | { readonly kind: "index"; readonly position: Ops }
  | { readonly kind: "let"; readonly definition: Definition; readonly value: Ops }
  | CheckStmt
  | { readonly kind: "repeat"; readonly count: RepeatCount; readonly record: number }
  | SectionsStmt
  | ArmStmt
  | { readonly kind: "if"; readonly condition: Ops }
  | { readonly kind: "elseIf"; readonly condition: Ops }
  | { readonly kind: "else" }
  /** `else if:type`, a declaration named `if` as the else branch. */
  | {
      readonly kind: "elseDeclaration";
      readonly definition: Definition;
      readonly declared: Declared;
    }
  | { readonly kind: "instances" }
  | { readonly kind: "close" };

export interface CheckStmt {
  readonly kind: "check";
  readonly condition: Ops;
  readonly message: string | null;
}

export interface SectionsStmt {
  readonly kind: "sections";
  readonly clauses: SectionClause[];
}

export interface ArmStmt {
  readonly kind: "arm";
  readonly pieces: readonly Piece[];
  readonly optional: boolean;
  readonly last: boolean;
  body: boolean;
}

/** A handle in a pattern. */
export interface NameMention {
  readonly kind: "name";
  readonly text: string;
  readonly owner: number;
}

/** A use of a named type. */
export type NamedMention = Extract<TypeSpec, { readonly kind: "named" }>;

/**
 * Something a statement mentions: an expression, a handle in a pattern, or a
 * named type.
 */
export type Mention = Ops | NameMention | NamedMention;

export function isOps(mention: Mention): mention is Ops {
  return Array.isArray(mention);
}

export function* declaredMentions(declared: Declared): Generator<Mention> {
  yield* declared.dimensions;
  const { ty } = declared;
  if (ty.kind === "named") {
    yield ty;
  } else if (ty.kind === "range") {
    for (const bound of [ty.lower, ty.upper]) {
      if (bound !== null) {
        yield bound;
      }
    }
  }
  for (const option of [declared.layout, declared.offset]) {
    if (option !== null) {
      yield option;
    }
  }
}

function captureDeclared(piece: Piece): Declared | null {
  switch (piece.kind) {
    case "validate":
    case "discardAs":
    case "bind":
      return piece.capture;
    default:
      return null;
  }
}

export function* patternMentions(pieces: readonly Piece[]): Generator<Mention> {
  for (const piece of pieces) {
    if (piece.kind === "capture" || piece.kind === "bind" || piece.kind === "reference") {
      yield { kind: "name", text: piece.name, owner: piece.owner };
    }
    const declared = captureDeclared(piece);
    if (declared !== null) {
      yield* declaredMentions(declared);
    }
  }
}

/** Every expression, handle and named type a statement mentions. */
export function* mentions(stmt: Stmt): Generator<Mention> {
  switch (stmt.kind) {
    case "param":
      if (stmt.default !== null) {
        yield stmt.default;
      }
      break;
    case "declaration":
    case "type":
    case "elseDeclaration":
      yield* declaredMentions(stmt.declared);
      break;
    case "list":
      yield* declaredMentions(stmt.declared);
      if (stmt.end !== null) {
        yield stmt.end;
      }
      break;
    case "line":
    case "arm":
      yield* patternMentions(stmt.pieces);
      break;
    case "if":
    case "elseIf":
    case "check":
      yield stmt.condition;
      break;
    case "let":
      yield stmt.value;
      break;
    case "skipLines":
      yield stmt.count;
      break;
    case "index":
      yield stmt.position;
      break;
    case "repeat":
      if (stmt.count.kind === "times") {
        yield stmt.count.count;
      }
      break;
    default:
      break;
  }
}

/** The expressions a statement mentions. */
export function* expressions(stmt: Stmt): Generator<Ops> {
  for (const mention of mentions(stmt)) {
    if (isOps(mention)) {
      yield mention;
    }
  }
}

/** Whether a statement starts a block that `close` ends. */
export function opensBlock(stmt: Stmt): boolean {
  switch (stmt.kind) {
    case "arm":
      return stmt.body;
    case "repeat":
    case "sections":
    case "if":
    case "elseIf":
    case "else":
    case "instances":
      return true;
    default:
      return false;
  }
}

/** Whether a statement continues the line of the preceding `}`. */
export function continuesIf(stmt: Stmt): boolean {
  return stmt.kind === "elseIf" || stmt.kind === "else" || stmt.kind === "elseDeclaration";
}

/** What a forward handle or capture promises about its definitions. */
export interface Shape {
  readonly value: ValueTag;
  readonly levels: number | null;
  readonly required: boolean;
}

export type NameState =
  | { readonly kind: "defined" }
  | { readonly kind: "forward"; readonly shape: Shape }
  /** From `undefined`; nothing may define it. */
  | { readonly kind: "undefined" }
  | { readonly kind: "capture"; readonly spec: Declared | null; readonly shape: Shape };

export interface RecordInfo {
  each: string | null;
  list: string | null;
  /** For a record from `forwardRecord`, the call that made it. */
  forward: { readonly site: CallSite | null } | null;
  bound: boolean;
  /** The forward record this repeat was bound to. */
  link: number | null;
}

/** A forward handle or capture, with the call that made it. */
export interface DeclaredName {
  readonly name: string;
  readonly site: CallSite | null;
}

/** The block that the last `}` closed, for `else` to continue. */
export interface ClosedBlock {
  readonly parent: number;
  readonly opener: number;
  readonly close: number;
}

export const ROOT_BLOCK = 0;

export function isIdentifier(name: string): boolean {
  return /^[A-Za-z_][A-Za-z0-9_]*$/u.test(name);
}

/** The builder's state, shared by its scopes and guards. */
export class State {
  readonly id: number = nextOwner();
  readonly stmts: Stmt[] = [];
  /** The call site of each statement. A `}` has the call that wrote its block. */
  readonly sites: (CallSite | null)[] = [];
  /** The ids of the open blocks, innermost last. */
  readonly open: number[] = [ROOT_BLOCK];
  nextBlock: number = ROOT_BLOCK + 1;
  readonly names: Map<string, NameState> = new Map();
  /** Forward handles and captures in creation order, for reporting. */
  readonly declared: DeclaredName[] = [];
  readonly types: Set<string> = new Set();
  readonly records: RecordInfo[] = [];
  closed: ClosedBlock | null = null;
  issue: BuildError | null = null;

  /**
   * Reports a failed check. The first one is kept, since the checks after it
   * may only be its consequences, and `build()` throws it again.
   */
  fail(kind: BuildErrorKind, message: string, site: CallSite | null): never {
    const error = BuildError.check(kind, message, site);
    this.issue ??= error;
    throw error;
  }

  openBlock(): number {
    const block = this.nextBlock;
    this.nextBlock += 1;
    this.open.push(block);
    return block;
  }

  closeBlock(parent: number, opener: number, site: CallSite | null): void {
    this.open.pop();
    this.pushClose(parent, opener, site);
  }

  pushClose(parent: number, opener: number, site: CallSite | null): void {
    this.stmts.push({ kind: "close" });
    this.sites.push(site);
    this.closed = { parent, opener, close: this.stmts.length - 1 };
  }

  checkInnermost(block: number, site: CallSite | null): void {
    if (this.open.at(-1) !== block) {
      this.fail(
        "scope",
        "a statement went to a scope that is not the innermost open block; " +
          "use the scope that the block's callback received",
        site,
      );
    }
  }

  isLast(index: number): boolean {
    return index + 1 === this.stmts.length;
  }

  /** Checks that a guard's statement at `index` is still the last one. */
  checkLast(index: number, site: CallSite | null): void {
    if (!this.isLast(index)) {
      this.fail(
        "scope",
        "a guard was used after later statements; " +
          "finish a statement before starting the next one",
        site,
      );
    }
  }

  /** Removes the `}` at `index`, so that a block can take its body. */
  reopen(index: number, site: CallSite | null): void {
    this.checkLast(index, site);
    this.stmts.pop();
    this.sites.pop();
  }

  checkOwner(owner: number, what: string, name: string, site: CallSite | null): void {
    if (owner !== this.id) {
      this.fail("provenance", `${what} \`${name}\` does not come from this builder`, site);
    }
  }

  checkRecord(owner: number, site: CallSite | null): void {
    if (owner !== this.id) {
      this.fail("provenance", "a record handle does not come from this builder", site);
    }
  }

  checkMention(mention: Mention, site: CallSite | null): void {
    if (isOps(mention)) {
      for (const op of mention) {
        if (op.kind === "name" || op.kind === "member") {
          this.checkOwner(op.owner, "the handle", op.text, site);
        } else if (op.kind === "record" || op.kind === "recordList" || op.kind === "listMember") {
          this.checkRecord(op.owner, site);
        }
      }
    } else if (mention.kind === "name") {
      this.checkOwner(mention.owner, "the handle", mention.text, site);
    } else {
      this.checkOwner(mention.owner, "the type", mention.name, site);
    }
  }

  /** Checks the handles of a capture's type, which a pattern prints later. */
  checkCapture(spec: Declared | null, site: CallSite | null): void {
    if (spec !== null) {
      for (const mention of declaredMentions(spec)) {
        this.checkMention(mention, site);
      }
    }
  }

  /** Checks the handles of `stmt` and appends it to the block `block`. */
  push(block: number, stmt: Stmt, site: CallSite | null): number {
    this.checkInnermost(block, site);
    for (const mention of mentions(stmt)) {
      this.checkMention(mention, site);
    }
    this.stmts.push(stmt);
    this.sites.push(site);
    return this.stmts.length - 1;
  }

  /** Follows a repeat's record to the forward record it was bound to. */
  recordId(record: number): number {
    return this.records[record]?.link ?? record;
  }

  newRecord(): number {
    this.records.push({ each: null, list: null, forward: null, bound: false, link: null });
    return this.records.length - 1;
  }

  /**
   * Checks the name of a definition. A declaration, list or capture named `_`
   * stores nothing, and `let`, `param` and `type` reject `_`, so no
   * definition gives a handle for it.
   */
  definitionName(name: unknown, site: CallSite | null): string {
    if (name === "_") {
      this.fail(
        "unprintable",
        "`_` names nothing here: use `discard()` to read without storing, " +
          'or `forward("_", "string")` with `store` for the entry named `_`',
        site,
      );
    }
    return this.spelledName(name, site);
  }

  /**
   * Checks a name that may be `_`: a repeat's `each` or `as` name, a forward
   * handle for `store`, or a name left undefined.
   */
  spelledName(name: unknown, site: CallSite | null): string {
    if (typeof name !== "string") {
      throw new TypeError(`a name is a string, not ${name === null ? "null" : typeof name}`);
    }
    if (name.startsWith("gecode.")) {
      this.fail(
        "unprintable",
        `a definition cannot be named \`${name}\`: the prefix \`gecode.\` is reserved`,
        site,
      );
    }
    if (!isIdentifier(name)) {
      this.fail("unprintable", `\`${name}\` is not an Iris name`, site);
    }
    return name;
  }

  defineNew(name: string, site: CallSite | null): string {
    const checked = this.definitionName(name, site);
    if (this.names.has(checked)) {
      this.fail(
        "redefinition",
        `\`${checked}\` is already defined; to redefine it, pass its handle as the target, or use \`set\``,
        site,
      );
    }
    this.names.set(checked, { kind: "defined" });
    return checked;
  }

  /** Declares a checked name for a forward handle, a capture, or `undefined`. */
  declare(name: string, state: NameState, site: CallSite | null): string {
    if (this.names.has(name)) {
      this.fail("redefinition", `\`${name}\` is already declared`, site);
    }
    this.names.set(name, state);
    this.declared.push({ name, site });
    return name;
  }
}
