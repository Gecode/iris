// Scopes, the guards of `if`, the builder, and `spec`.
import type { CallSite, CallSites } from "../call-sites.js";
import type { Data } from "../data.js";
import { IrisError } from "../errors.js";
import { isHandle, type Handle, type HandleDepth, type Presence, type ViewOf } from "../handle.js";
import { Spec, type Params } from "../spec.js";
import { checkBindings } from "./check.js";
import { BuildError } from "./error.js";
import {
  createField,
  dynField,
  Expr,
  Field,
  isOpen,
  Lookup,
  operand,
  ownerOf,
  Record,
  recordOf,
  treeOf,
  typeName,
  type DynField,
  type IntOperand,
  type Operand,
  type TagOf,
} from "./expr.js";
import { describeTag, postfix, type Ops, type ValueTag } from "./ops.js";
import { captureSpec, captureTag, Discard, patternPieces, type LinePattern } from "./pattern.js";
import { printSpec, type Printed } from "./print.js";
import { caller } from "./site.js";
import {
  ROOT_BLOCK,
  State,
  type ArmStmt,
  type Definition,
  type RepeatCount,
  type SectionClause,
  type SectionsStmt,
  type Stmt,
} from "./statement.js";
import {
  checkText,
  declarationOf,
  handleDepth,
  ListReading,
  namedType,
  scalarDeclared,
  type Add,
  type OneDepth,
  type Next,
  type ReadValue,
  type Type,
} from "./types.js";

/** The origin reported for the text a builder emits. */
const ORIGIN = "builder.iris";

/** `A` plus `B` array levels, where an open `B` gives an open depth. */
type AddDepth<A extends HandleDepth, B extends HandleDepth> = B extends number ? Add<A, B> : null;

/**
 * The array levels a `let` of `E` has in a scope at depth `D`: a lookup may
 * be an array, except a bare handle at the top level, whose depth is its own.
 */
export type LetDepth<E, D extends HandleDepth> =
  E extends Expr<"dyn">
    ? null
    : E extends Field<ValueTag, infer FD>
      ? D extends 0
        ? FD
        : null
      : E extends Lookup
        ? null
        : D;

/**
 * What `set` stores through a handle of value type `V`: a value of that type,
 * or anything for a dynamic handle. A handle whose type may be one of several
 * takes nothing.
 */
export type Settable<V extends ValueTag> = [V] extends ["dyn"]
  ? Operand
  : [V] extends ["int"]
    ? Expr<"int"> | number | bigint
    : [V] extends ["string"]
      ? Expr<"string"> | string
      : [V] extends ["float"]
        ? Expr<"float">
        : [V] extends ["num"]
          ? Expr<"num">
          : never;

/**
 * `E` when `set` may store it through a handle of depth `HD` in a scope at
 * depth `D`: where either depth is open, or they may be equal. Else `never`,
 * which rejects the value.
 */
export type Kept<E, HD extends HandleDepth, D extends HandleDepth> = null extends HD
  ? E
  : null extends LetDepth<E, D>
    ? E
    : [Extract<HD, LetDepth<E, D>>] extends [never]
      ? never
      : E;

/** The depth of `param p = default`: a scalar's, or open for a lookup. */
export type ParamDepth<E, D extends HandleDepth> =
  E extends Expr<"dyn"> ? null : E extends Lookup ? null : D;

/** The scope of a block at depth `D` with presence `P`. */
export type In<D extends HandleDepth, P extends Presence> = D extends 0
  ? Scope<0, P>
  : NestedScope<D, P>;

/** Where `store` keeps a terminator: a new name, or a handle from `forward`. */
export type StoreTarget = string | Field;

/** The options of `forward`. */
export interface ForwardOptions {
  /** The array levels of every definition; `null` for any. The default is 0. */
  readonly depth?: number | null;
  /** Whether a definition may not execute. The default is `false`. */
  readonly optional?: boolean;
}

/**
 * The value an options object `O` may give its property `K`, or `Absent`
 * where it lacks the property. A union of objects gives the union of values.
 */
type OptionValue<O, K extends string, Absent> = O extends unknown
  ? K extends keyof O
    ? O[K]
    : Absent
  : never;

/**
 * The depth of a forward handle with options `O`: the literal depth they
 * give, the default 0 where they give none, and `null` where the type of the
 * options allows more than one depth.
 */
type ForwardDepth<O> = OneDepth<
  OptionValue<O, "depth", undefined> extends infer X
    ? X extends undefined
      ? 0
      : X extends HandleDepth
        ? X
        : null
    : never
>;

/** `"maybe"` when options `O` may set `optional`, else `Otherwise`. */
type OptionalPresence<O, Otherwise extends Presence> =
  true extends OptionValue<O, "optional", false> ? "maybe" : Otherwise;

/** The options of `repeatUntil`. */
export interface RepeatUntilOptions {
  /** `store x`: keeps the terminator that ended the repeat. */
  readonly store?: StoreTarget;
}

/** One clause of `sections`, for clauses in an order other than `SectionsOptions` gives. */
export type SectionClauseOptions =
  | { readonly until: string | readonly string[]; readonly store?: StoreTarget }
  | { readonly skipOther: true };

/** The clauses of `sections`: `until`, with its `store`, then `skip other`. */
export interface SectionsOptions {
  readonly until?: string | readonly string[];
  readonly store?: StoreTarget;
  readonly skipOther?: boolean;
}

/** The options of an arm of `sections`. */
export interface ArmOptions {
  /** The arm need not match, so what it defines may be absent. */
  readonly optional?: boolean;
  /** Sections end after this arm. */
  readonly last?: boolean;
}

interface ScopeCore {
  readonly state: State;
  readonly block: number;
  /** The record of the nearest repeat. */
  readonly record: number | null;
  /** The repeats around the block. */
  readonly depth: number;
  /** Whether the block's statements always execute. */
  readonly required: boolean;
}

let coreOf: (scope: Scope<HandleDepth, Presence>) => ScopeCore;
let makeScope: (core: ScopeCore) => Scope<HandleDepth, Presence>;

/**
 * An open block. `D` counts the repeats around it, and `P` says whether its
 * statements always execute.
 *
 * A scope accepts statements only while it is the innermost open block, and
 * a block's callback must be synchronous.
 */
export class Scope<D extends HandleDepth = 0, P extends Presence = "required"> {
  readonly #core: ScopeCore;

  protected constructor(core: ScopeCore) {
    this.#core = core;
  }

  static {
    coreOf = (scope) => scope.#core;
    makeScope = (core) => (core.depth === 0 ? new Scope(core) : NestedScope.create(core));
  }

  /** `x: type`, reading a value into a new name. */
  read<V extends ReadValue, N extends HandleDepth>(
    target: string,
    ty: Type<V, N>,
  ): Field<V, AddDepth<D, N>, P>;
  /** `x: type` through a handle: binds a forward handle, or redefines a name. */
  read(target: Field, ty: Type): DynField;
  /** `_: type`, reading a value without storing it. */
  read(target: Discard, ty: Type): undefined;
  read(target: string | Field | Discard, ty: Type): Field | undefined {
    const site = caller();
    const reading = declarationOf(ty, "a read's type");
    const own = reading.dimensions.length;
    return define(
      this.#core,
      site,
      target,
      ty.valueType,
      own,
      (definition) => ({ kind: "declaration", definition, declared: reading }),
      this.#core.depth + own,
    );
  }

  /** `list x: type until end`. */
  list<V extends ReadValue>(target: string, reading: ListReading<V>): Field<V, Next<D>, P>;
  list(target: Field, reading: ListReading): DynField;
  list(target: Discard, reading: ListReading): undefined;
  list(target: string | Field | Discard, reading: ListReading): Field | undefined {
    const site = caller();
    if (!(reading instanceof ListReading)) {
      throw new TypeError("a list reads `type.until(end)`, such as `int.until(0)`");
    }
    const { declared: listed, end } = reading.parts();
    return define(
      this.#core,
      site,
      target,
      reading.valueType,
      1,
      (definition) => ({ kind: "list", definition, declared: listed, end }),
      this.#core.depth + 1,
    );
  }

  /** `let x = value`. */
  let<E extends Operand>(target: string, value: E): Field<TagOf<E>, LetDepth<E, D>, P>;
  /** `let x = value` through a handle, which may change its type. */
  let(target: Field, value: Operand): DynField;
  let(target: Discard, value: Operand): undefined;
  let(target: string | Field | Discard, value: Operand): Field | undefined {
    const site = caller();
    const expr = operand(value);
    const own = letLevels(expr, this.#core.depth);
    const ops = postfix(treeOf(expr));
    return define(
      this.#core,
      site,
      target,
      expr.valueType,
      own,
      (definition) => ({ kind: "let", definition, value: ops }),
      own === null ? null : this.#core.depth + own,
    );
  }

  /**
   * `let x = value` through a handle, keeping its type. The value must have
   * the handle's value type and, where both are known, its depth. To change
   * either, use `let` with the handle as the target, which returns a dynamic
   * handle.
   */
  set<V extends ValueTag, HD extends HandleDepth, E extends Settable<V>>(
    handle: Field<V, HD>,
    value: Kept<E, HD, D>,
  ): void;
  set(handle: Field, value: Operand): void {
    const site = caller();
    if (!(handle instanceof Field)) {
      throw new TypeError(`\`set\` takes a handle, not ${typeName(handle)}`);
    }
    const expr = operand(value);
    if (handle.valueType !== "dyn" && expr.valueType !== handle.valueType) {
      throw new TypeError(
        `\`set\` keeps the handle's value type ${describeTag(handle.valueType)}, but this value ` +
          `is ${describeTag(expr.valueType)}; to change it, redefine the name with \`let(handle, value)\``,
      );
    }
    const own = letLevels(expr, this.#core.depth);
    const total = own === null ? null : this.#core.depth + own;
    if (handle.depth !== null && total !== null && handle.depth !== total) {
      throw new TypeError(
        `\`set\` keeps the handle's depth ${handle.depth}, but this value has depth ${total}; ` +
          "to change the shape, redefine the name with `let(handle, value)`",
      );
    }
    const ops = postfix(treeOf(expr));
    define(
      this.#core,
      site,
      handle,
      handle.valueType,
      own,
      (definition) => ({ kind: "let", definition, value: ops }),
      null,
    );
  }

  /** `param p`, whose type the caller decides. */
  param(target: string): Field<"dyn", null, P>;
  param(target: Field): DynField;
  /** `param p = default`. */
  param<E extends Operand>(target: string, value: E): Field<TagOf<E>, ParamDepth<E, D>, P>;
  param(target: Field, value: Operand): DynField;
  param(target: string | Field, ...rest: [] | [Operand]): Field | undefined {
    const site = caller();
    const core = this.#core;
    if (rest.length === 0) {
      const name = targetName(core.state, target, site);
      core.state.push(
        core.block,
        { kind: "param", definition: { name, value: "dyn", levels: null }, default: null },
        site,
      );
      if (name === null) {
        return undefined;
      }
      const required = core.required && !(target instanceof Field);
      return createField(name, core.state.id, "dyn", null, required ? "required" : "maybe");
    }
    const expr = operand(rest[0]);
    const scalar = expr.valueType !== "dyn" && !isOpen(expr);
    const ops = postfix(treeOf(expr));
    return define(
      core,
      site,
      target,
      expr.valueType,
      scalar ? 0 : null,
      (definition) => ({ kind: "param", definition, default: ops }),
      scalar ? core.depth : null,
    );
  }

  /** `check condition`, with an optional message for when it fails. */
  check(condition: Operand, message?: string): void {
    const site = caller();
    if (message !== undefined && typeof message !== "string") {
      throw new TypeError("a check's message is a string");
    }
    push(
      this.#core,
      { kind: "check", condition: conditionOps(condition), message: message ?? null },
      site,
    );
  }

  /** `expect "text"...`. */
  expect(...words: string[]): void {
    const site = caller();
    push(
      this.#core,
      { kind: "strings", keyword: "expect", strings: strings(words, "expected words") },
      site,
    );
  }

  /** `line "pattern"`. */
  line(pattern: LinePattern): void {
    const site = caller();
    push(this.#core, { kind: "line", keyword: "line", pieces: patternPieces(pattern) }, site);
  }

  /** `find "pattern"`. */
  find(pattern: LinePattern): void {
    const site = caller();
    push(this.#core, { kind: "line", keyword: "find", pieces: patternPieces(pattern) }, site);
  }

  /** `skip line`. */
  skipLine(): void {
    const site = caller();
    push(this.#core, { kind: "skipLine" }, site);
  }

  /** `skip count lines`. */
  skipLines(count: IntOperand): void {
    const site = caller();
    push(this.#core, { kind: "skipLines", count: opsOf(count) }, site);
  }

  /** `index position`. */
  index(position: IntOperand): void {
    const site = caller();
    push(this.#core, { kind: "index", position: opsOf(position) }, site);
  }

  /**
   * `type name = type`, usable wherever a type is. The type it returns
   * belongs to this declaration: `build()` reports a use where another
   * declaration of the name is visible.
   */
  defineType<V extends ReadValue>(name: string, ty: Type<V, 0>): Type<V, 0> {
    const site = caller();
    const definition = scalarDeclared(ty, "a named type");
    const { state } = this.#core;
    const checked = state.definitionName(name, site);
    state.types.add(checked);
    const declaration = push(
      this.#core,
      { kind: "type", name: checked, declared: definition },
      site,
    );
    return namedType(ty.valueType, {
      ty: { kind: "named", name: checked, owner: state.id, declaration },
      dimensions: [],
      layout: null,
      offset: null,
    });
  }

  /** `repeat count { ... }`, whose block is the next depth. */
  repeat<R = undefined>(count: IntOperand, body?: (scope: In<Next<D>, P>) => R): R {
    const site = caller();
    return repeat(this.#core, site, { kind: "times", count: opsOf(count) }, body);
  }

  /** `repeat until eof { ... }`. */
  repeatUntilEof<R = undefined>(body?: (scope: In<Next<D>, P>) => R): R {
    const site = caller();
    return repeat(this.#core, site, { kind: "untilEof" }, body);
  }

  /**
   * `repeat until "text"... { ... }`. `store` keeps the terminator that ended
   * it in a name; to read it, pass a handle from `forward`.
   */
  repeatUntil<R = undefined>(
    terminators: string | readonly string[],
    body?: (scope: In<Next<D>, P>) => R,
  ): R;
  repeatUntil<R = undefined>(
    terminators: string | readonly string[],
    options: RepeatUntilOptions,
    body?: (scope: In<Next<D>, P>) => R,
  ): R;
  repeatUntil(terminators: string | readonly string[], ...rest: unknown[]): unknown {
    const site = caller();
    const [first, second] = rest;
    const options: unknown = typeof first === "function" ? {} : (first ?? {});
    const body = typeof first === "function" ? first : second;
    const store = optionsField(options, "store");
    const count: RepeatCount = {
      kind: "until",
      terminators: stringList(terminators, "terminators"),
      store: store === undefined ? null : storeDefinition(this.#core.state, store, site),
    };
    return repeat(this.#core, site, count, body);
  }

  /**
   * `sections`: the callback adds the arms. `until` and `skipOther` add
   * clauses in that order; pass a list of clauses for any other order.
   */
  sections<R = undefined>(body?: (arms: Arms<D, P>) => R): R;
  sections<R = undefined>(
    options: SectionsOptions | readonly SectionClauseOptions[],
    body?: (arms: Arms<D, P>) => R,
  ): R;
  sections(...args: unknown[]): unknown {
    const site = caller();
    const core = this.#core;
    const [first, second] = args;
    const options: unknown = typeof first === "function" ? {} : (first ?? {});
    const body = typeof first === "function" ? first : second;
    const stmt: SectionsStmt = {
      kind: "sections",
      clauses: sectionClauses(core.state, options, site),
    };
    const index = push(core, stmt, site);
    return enter(core.state, core.block, index, site, body, (block) =>
      Arms.create({ ...core, block }),
    );
  }

  /** `if condition { ... }`. What its block defines may be absent. */
  if<R>(condition: Operand, body: (scope: In<D, "maybe">) => R): If<D, P, R> {
    const site = caller();
    const core = this.#core;
    const index = push(core, { kind: "if", condition: conditionOps(condition) }, site);
    const result = enter(core.state, core.block, index, site, body, (block) =>
      makeScope({ ...core, block, required: false }),
    );
    return If.create<D, P, R>(core, result);
  }

  /**
   * A handle for a pattern to bind, at this scope's depth and presence.
   * Without a type the capture is `{x}`, which reads a word.
   */
  capture(name: string): Field<"string", D, P>;
  capture<V extends ReadValue>(name: string, ty: Type<V, 0>): Field<V, D, P>;
  capture(name: string, ty?: Type): Field {
    const site = caller();
    return declareCapture(this.#core, site, name, ty, this.#core.required);
  }

  /** `capture` for an optional arm or a conditional block. */
  optionalCapture(name: string): Field<"string", D, "maybe">;
  optionalCapture<V extends ReadValue>(name: string, ty: Type<V, 0>): Field<V, D, "maybe">;
  optionalCapture(name: string, ty?: Type): Field {
    const site = caller();
    return declareCapture(this.#core, site, name, ty, false);
  }

  /**
   * A handle for a name that a later statement defines. Without a value type
   * it promises nothing; with one, every definition must have it, at
   * `options.depth` (default 0), and one must always execute unless
   * `options.optional`.
   *
   * `build()` reports a forward handle that nothing defines, and a definition
   * that does not have its type, unless an earlier one always precedes it.
   * `forward("_", "string")` is the handle of the entry that `store _` keeps.
   */
  forward(name: string): DynField;
  forward<V extends ValueTag>(name: string, valueType: V): Field<V, 0, "required">;
  forward<V extends ValueTag, const O extends ForwardOptions>(
    name: string,
    valueType: V,
    options: O,
  ): Field<V, ForwardDepth<O>, OptionalPresence<O, "required">>;
  forward(name: string, valueType?: ValueTag, options: ForwardOptions = {}): Field {
    const site = caller();
    const { state } = this.#core;
    const spelled = state.spelledName(name, site);
    if (valueType === undefined) {
      state.declare(
        spelled,
        { kind: "forward", shape: { value: "dyn", levels: null, required: false } },
        site,
      );
      return dynField(spelled, state.id);
    }
    if (!VALUE_TAGS.includes(valueType)) {
      throw new TypeError(`a forward handle's value type is one of ${VALUE_TAGS.join(", ")}`);
    }
    const levels = options.depth === undefined ? 0 : options.depth;
    if (levels !== null && (!Number.isSafeInteger(levels) || levels < 0)) {
      throw new TypeError("a forward handle's depth is a non-negative integer or null");
    }
    const required = options.optional !== true;
    state.declare(
      spelled,
      { kind: "forward", shape: { value: valueType, levels, required } },
      site,
    );
    return createField(
      spelled,
      state.id,
      valueType,
      handleDepth(levels),
      required ? "required" : "maybe",
    );
  }

  /**
   * A handle for a name deliberately left undefined, as in
   * `check !defined(name)`. Defining it fails `build()`.
   */
  undefined(name: string): DynField {
    const site = caller();
    const { state } = this.#core;
    const spelled = state.declare(state.spelledName(name, site), { kind: "undefined" }, site);
    return dynField(spelled, state.id);
  }

  /**
   * The record of a repeat that comes later in the text, printed with the
   * names given here. Bind it with `recordAs`.
   */
  forwardRecord(each: string, list: string): Record {
    const site = caller();
    const { state } = this.#core;
    const eachName = state.spelledName(each, site);
    const listName = state.spelledName(list, site);
    const record = state.newRecord();
    const info = state.records[record];
    if (info !== undefined) {
      info.each = eachName;
      info.list = listName;
      info.forward = { site };
    }
    return Record.create(record, state.id);
  }
}

/** A scope inside at least one repeat. */
export class NestedScope<D extends HandleDepth = 1, P extends Presence = "required"> extends Scope<
  D,
  P
> {
  /** @internal */
  static create(core: ScopeCore): NestedScope<HandleDepth, Presence> {
    return new NestedScope(core);
  }

  /** The record of the nearest repeat. */
  get record(): Record {
    const core = coreOf(this);
    return Record.create(nestedRecord(core), core.state.id);
  }

  /** Prints this repeat's `each` name as `name`, which may be `_`. */
  eachName(name: string): void {
    const site = caller();
    const core = coreOf(this);
    const info = core.state.records[core.state.recordId(nestedRecord(core))];
    const spelled = core.state.spelledName(name, site);
    if (info !== undefined) {
      info.each = spelled;
    }
  }

  /** Prints this repeat's `as` name as `name`, which may be `_`. */
  asName(name: string): void {
    const site = caller();
    const core = coreOf(this);
    const info = core.state.records[core.state.recordId(nestedRecord(core))];
    const spelled = core.state.spelledName(name, site);
    if (info !== undefined) {
      info.list = spelled;
    }
  }

  /** Binds this repeat to a record from `forwardRecord`. */
  recordAs(record: Record): void {
    const site = caller();
    const { state, ...core } = coreOf(this);
    if (!(record instanceof Record)) {
      throw new TypeError(`\`recordAs\` takes a record, not ${typeName(record)}`);
    }
    const { id, owner } = recordOf(record);
    state.checkRecord(owner, site);
    const info = state.records[id];
    if (info === undefined || info.forward === null || info.bound) {
      return state.fail(
        "binding",
        "`recordAs` needs a record from `forwardRecord` that is not bound yet",
        site,
      );
    }
    info.bound = true;
    const own = state.records[nestedRecord({ state, ...core })];
    if (own !== undefined) {
      own.link = id;
    }
  }
}

/** The block of a `sections` statement, which holds only arms. */
export class Arms<D extends HandleDepth = 0, P extends Presence = "required"> {
  readonly #core: ScopeCore;

  private constructor(core: ScopeCore) {
    this.#core = core;
  }

  /** @internal */
  static create(core: ScopeCore): Arms<HandleDepth, Presence> {
    return new Arms(core);
  }

  /**
   * An arm with this pattern, and a block when there is a callback. What an
   * optional arm defines may be absent, and sections end after a `last` arm.
   */
  arm<R = undefined>(pattern: LinePattern, body?: (scope: In<D, P>) => R): R;
  arm<const O extends ArmOptions, R = undefined>(
    pattern: LinePattern,
    options: O,
    body?: (scope: In<D, OptionalPresence<O, P>>) => R,
  ): R;
  arm(pattern: LinePattern, ...rest: unknown[]): unknown {
    const site = caller();
    const core = this.#core;
    const [first, second] = rest;
    const options: unknown = typeof first === "function" ? {} : (first ?? {});
    const body = typeof first === "function" ? first : second;
    const optional = optionsField(options, "optional") === true;
    const stmt: ArmStmt = {
      kind: "arm",
      pieces: patternPieces(pattern),
      optional,
      last: optionsField(options, "last") === true,
      body: body !== undefined,
    };
    const index = core.state.push(core.block, stmt, site);
    if (body === undefined) {
      return undefined;
    }
    return enter(core.state, core.block, index, site, body, (block) =>
      makeScope({ ...core, block, required: core.required && !optional }),
    );
  }
}

/** The guard of an `if`, holding what its branches returned. */
export class If<D extends HandleDepth, P extends Presence, R> {
  readonly #core: ScopeCore;
  readonly #close: number;
  readonly #result: R;
  /** Keeps `P` in the type; the presence of the scope around the `if`. */
  declare readonly presence: P;

  private constructor(core: ScopeCore, result: R) {
    this.#core = core;
    this.#close = core.state.stmts.length - 1;
    this.#result = result;
  }

  /** @internal */
  static create<D extends HandleDepth, P extends Presence, R>(
    core: ScopeCore,
    result: R,
  ): If<D, P, R> {
    return new If(core, result);
  }

  /** What the branches returned so far. */
  get result(): R {
    return this.#result;
  }

  /** `else if condition { ... }`. */
  elseIf<R2>(condition: Operand, body: (scope: In<D, "maybe">) => R2): If<D, P, [R, R2]> {
    const site = caller();
    const index = this.#continuation({ kind: "elseIf", condition: conditionOps(condition) }, site);
    const otherwise = this.#enter(index, site, body);
    return new If(this.#core, [this.#result, otherwise]);
  }

  /** `else { ... }`. */
  else<R2>(body: (scope: In<D, "maybe">) => R2): [R, R2] {
    const site = caller();
    const index = this.#continuation({ kind: "else" }, site);
    return [this.#result, this.#enter(index, site, body)];
  }

  /**
   * `else if:type`: the else branch is a declaration of the name `if`, which
   * the target must have.
   */
  elseReadIf<V extends ReadValue, N extends HandleDepth>(
    target: "if",
    ty: Type<V, N>,
  ): [R, Field<V, AddDepth<D, N>, "maybe">];
  elseReadIf(target: Field, ty: Type): [R, DynField];
  elseReadIf(target: string | Field, ty: Type): [R, Field | undefined] {
    const site = caller();
    const core = this.#core;
    core.state.checkLast(this.#close, site);
    const reading = declarationOf(ty, "a read's type");
    const own = reading.dimensions.length;
    const name = target instanceof Field ? target.name : target;
    if (name !== "if") {
      core.state.fail(
        "unprintable",
        "`elseReadIf` declares the name `if`, which its target must have",
        site,
      );
    }
    const handle = define(
      core,
      site,
      target,
      ty.valueType,
      own,
      (definition) => ({ kind: "elseDeclaration", definition, declared: reading }),
      core.depth + own,
      false,
    );
    return [this.#result, handle];
  }

  #continuation(stmt: Stmt, site: CallSite | null): number {
    const { state, block } = this.#core;
    state.checkLast(this.#close, site);
    return state.push(block, stmt, site);
  }

  #enter<R2>(index: number, site: CallSite | null, body: (scope: In<D, "maybe">) => R2): R2 {
    const core = this.#core;
    return enter(core.state, core.block, index, site, body, (block) =>
      makeScope({ ...core, block, required: false }),
    );
  }
}

let formatOf: (builder: SpecBuilder, name: string, site: CallSite | null) => void;
let printedOf: (builder: SpecBuilder) => Printed;

/**
 * Builds a reader spec. It is the root scope, with the settings and
 * statements that only the top level allows.
 */
export class SpecBuilder extends Scope {
  /** A builder; `format` adds `format name` first. */
  constructor(format?: string) {
    const site = caller();
    super({ state: new State(), block: ROOT_BLOCK, record: null, depth: 0, required: true });
    if (format !== undefined) {
      this.#format(format, site);
    }
  }

  static {
    formatOf = (builder, name, site) => builder.#format(name, site);
    printedOf = (builder) => builder.#printed();
  }

  /** `format name`, a name of this format. */
  format(name: string): void {
    const site = caller();
    this.#format(name, site);
  }

  /** `comment "text"...`. */
  comment(...markers: string[]): void {
    const site = caller();
    push(
      coreOf(this),
      { kind: "strings", keyword: "comment", strings: strings(markers, "comment markers") },
      site,
    );
  }

  /** `stop "text"...`. */
  stop(...markers: string[]): void {
    const site = caller();
    push(
      coreOf(this),
      { kind: "strings", keyword: "stop", strings: strings(markers, "stop markers") },
      site,
    );
  }

  /** `tokens "characters"`. */
  tokens(characters: string): void {
    const site = caller();
    push(
      coreOf(this),
      { kind: "strings", keyword: "tokens", strings: strings([characters], "token characters") },
      site,
    );
  }

  /** `separators "characters"`. */
  separators(characters: string): void {
    const site = caller();
    push(
      coreOf(this),
      {
        kind: "strings",
        keyword: "separators",
        strings: strings([characters], "separator characters"),
      },
      site,
    );
  }

  /** `ignore trailing`. */
  ignoreTrailing(): void {
    const site = caller();
    push(coreOf(this), { kind: "ignoreTrailing" }, site);
  }

  /** `instances { ... }`. */
  instances<R = undefined>(body?: (scope: Scope) => R): R {
    const site = caller();
    const core = coreOf(this);
    const index = push(core, { kind: "instances" }, site);
    return enter(core.state, core.block, index, site, body, (block) =>
      makeScope({ ...core, block, record: null, depth: 0, required: true }),
    );
  }

  /** The text this builder emits, without parsing it. */
  source(): string {
    return this.#printed().text;
  }

  /**
   * Prints the text and parses it. The spec is an ordinary one, whose errors
   * while reading name the call site of their statement.
   */
  build(): Spec {
    return parsePrinted(this.#printed());
  }

  #format(name: string, site: CallSite | null): void {
    push(coreOf(this), { kind: "format", name: checkText(name, "a format name") }, site);
  }

  #printed(): Printed {
    const { state } = coreOf(this);
    if (state.issue !== null) {
      throw state.issue;
    }
    checkBindings(state);
    return printSpec(state);
  }
}

/**
 * The data that `read` gives for the handles `H`: an object with the
 * properties of `H` that are handles, each typed by its handle, or the view
 * of `H` itself when it is a handle.
 */
export type DataOf<H> = H extends Handle
  ? ViewOf<H>
  : H extends object
    ? { -readonly [K in keyof H as H[K] extends Handle ? K : never]: ViewOf<Extract<H[K], Handle>> }
    : never;

/** A built reader spec, with the handles its callback returned. */
export class BuiltSpec<H> {
  /** The parsed spec. */
  readonly spec: Spec;
  /** What the callback returned. */
  readonly handles: H;
  /** The text the builder emitted. */
  readonly source: string;

  private constructor(spec: Spec, handles: H, source: string) {
    this.spec = spec;
    this.handles = handles;
    this.source = source;
  }

  /** @internal */
  static create<H>(spec: Spec, handles: H, source: string): BuiltSpec<H> {
    return new BuiltSpec(spec, handles, source);
  }

  /**
   * Reads one instance, and returns the entries that the handles name, each
   * with the type its handle gives. Integers are `number`s; for `bigint`s,
   * read with `spec` and use `data.get(handle, { bigint: true })`.
   */
  read(input: Uint8Array | string, params?: Params, origin?: string): DataOf<H> {
    const data = this.spec.read(input, origin, params === undefined ? {} : { params });
    return views(data, this.handles);
  }

  /** Lists the instances in an input. */
  catalog(input: Uint8Array | string, params?: Params, origin?: string): Data {
    return this.spec.catalog(input, origin, params === undefined ? {} : { params });
  }
}

/**
 * Builds a reader spec: the callback writes its statements on the builder
 * and returns the handles to export, which `read` gives typed data for.
 */
export function spec<H>(body: (builder: SpecBuilder) => H): BuiltSpec<H>;
export function spec<H>(format: string, body: (builder: SpecBuilder) => H): BuiltSpec<H>;
export function spec(...args: unknown[]): BuiltSpec<unknown> {
  const site = caller();
  const [first, second] = args;
  const body = typeof first === "function" ? first : second;
  if (typeof body !== "function") {
    throw new TypeError("spec takes a format name and a function that writes the spec");
  }
  const builder = new SpecBuilder();
  if (typeof first !== "function") {
    formatOf(builder, checkText(first, "a format name"), site);
  }
  const handles: unknown = Reflect.apply(body, undefined, [builder]);
  checkSynchronous(coreOf(builder).state, handles, site);
  const printed = printedOf(builder);
  return BuiltSpec.create(parsePrinted(printed), handles, printed.text);
}

/**
 * The call site of each statement in the text `builder` emits. The package
 * does not export it; the call-site tests import it.
 */
export function callSitesOf(builder: SpecBuilder): CallSites {
  return printedOf(builder).sites;
}

function parsePrinted({ text, sites }: Printed): Spec {
  try {
    return Spec.parseBuilt(text, ORIGIN, sites);
  } catch (error) {
    throw error instanceof IrisError ? BuildError.fromError(error, text) : error;
  }
}

function views(data: Data, handles: unknown): never;
function views<H>(data: Data, handles: H): DataOf<H>;
function views(data: Data, handles: unknown): unknown {
  if (isHandle(handles)) {
    return data.get(handles);
  }
  if (typeof handles !== "object" || handles === null) {
    return {};
  }
  // Own properties, so a key such as `__proto__` stays an ordinary property.
  return Object.fromEntries(
    Object.entries(handles).flatMap(([key, value]) =>
      isHandle(value) ? [[key, data.get(value)]] : [],
    ),
  );
}

const VALUE_TAGS: readonly ValueTag[] = ["int", "float", "num", "string", "dyn"];

function opsOf(value: unknown): Ops {
  return postfix(treeOf(operand(value)));
}

function conditionOps(value: unknown): Ops {
  if (typeof value === "boolean") {
    throw new TypeError(
      "a condition is an Iris expression, not a boolean: compare with methods such as `eq` and `lt`",
    );
  }
  return opsOf(value);
}

function strings(values: readonly unknown[], role: string): string[] {
  return values.map((value) => {
    if (typeof value !== "string") {
      throw new TypeError(`${role} are strings, not ${typeName(value)}`);
    }
    return value;
  });
}

function stringList(value: unknown, role: string): string[] {
  return typeof value === "string"
    ? [value]
    : Array.isArray(value)
      ? strings(value, role)
      : strings([value], role);
}

function optionsField(options: unknown, name: string): unknown {
  if (typeof options !== "object" || options === null) {
    throw new TypeError(`options must be an object, not ${typeName(options)}`);
  }
  return Reflect.get(options, name);
}

function push(core: ScopeCore, stmt: Stmt, site: CallSite | null): number {
  return core.state.push(core.block, stmt, site);
}

/**
 * The array levels a `let` of `value` adds in a scope at `depth`, as far as
 * its type says: a lookup may be an array, except a bare handle at the top
 * level, whose depth is its own.
 */
function letLevels(value: Expr, depth: number): number | null {
  if (value.valueType === "dyn") {
    return null;
  }
  if (value instanceof Field) {
    return depth === 0 ? value.depth : null;
  }
  return isOpen(value) ? null : 0;
}

/** Resolves a definition's target: `null` for the discard. */
function targetName(state: State, target: unknown, site: CallSite | null): string | null {
  if (target instanceof Discard) {
    return null;
  }
  if (typeof target === "string") {
    return state.defineNew(target, site);
  }
  if (target instanceof Field) {
    state.checkOwner(ownerOf(target), "the handle", target.name, site);
    return target.name;
  }
  throw new TypeError(`a target is a new name, a handle or discard(), not ${typeName(target)}`);
}

/** Resolves `target`, pushes its statement, and returns its handle. */
function define(
  core: ScopeCore,
  site: CallSite | null,
  target: unknown,
  value: ValueTag,
  levels: number | null,
  statement: (definition: Definition) => Stmt,
  handleLevels: number | null,
  required: boolean = core.required,
): Field | undefined {
  const name = targetName(core.state, target, site);
  core.state.push(core.block, statement({ name, value, levels }), site);
  if (name === null) {
    return undefined;
  }
  if (target instanceof Field) {
    return dynField(name, core.state.id);
  }
  return createField(
    name,
    core.state.id,
    value,
    handleDepth(handleLevels),
    required ? "required" : "maybe",
  );
}

/** The definition a terminator's `store` makes. */
function storeDefinition(state: State, target: unknown, site: CallSite | null): Definition {
  if (target instanceof Discard) {
    state.fail(
      "unprintable",
      "`store _` keeps an entry named `_`, so `discard()` cannot spell it: " +
        'pass a handle from `forward("_", "string")`',
      site,
    );
  }
  return { name: targetName(state, target, site), value: "string", levels: 0 };
}

function sectionClauses(state: State, options: unknown, site: CallSite | null): SectionClause[] {
  if (Array.isArray(options)) {
    return options.map((clause: unknown): SectionClause => {
      if (optionsField(clause, "skipOther") === true) {
        return { kind: "skipOther" };
      }
      const store = optionsField(clause, "store");
      return {
        kind: "until",
        terminators: stringList(optionsField(clause, "until"), "terminators"),
        store: store === undefined ? null : storeDefinition(state, store, site),
      };
    });
  }
  const clauses: SectionClause[] = [];
  const until = optionsField(options, "until");
  const store = optionsField(options, "store");
  if (until !== undefined) {
    clauses.push({
      kind: "until",
      terminators: stringList(until, "terminators"),
      store: store === undefined ? null : storeDefinition(state, store, site),
    });
  } else if (store !== undefined) {
    state.fail("unprintable", "`store` needs an `until` before it", site);
  }
  if (optionsField(options, "skipOther") === true) {
    clauses.push({ kind: "skipOther" });
  }
  return clauses;
}

function declareCapture(
  core: ScopeCore,
  site: CallSite | null,
  name: string,
  ty: unknown,
  required: boolean,
): Field {
  const { state } = core;
  const declaration = captureSpec(ty);
  const tag = captureTag(ty);
  state.checkCapture(declaration, site);
  const checked = state.definitionName(name, site);
  state.declare(
    checked,
    { kind: "capture", spec: declaration, shape: { value: tag, levels: core.depth, required } },
    site,
  );
  return createField(
    checked,
    state.id,
    tag,
    handleDepth(core.depth),
    required ? "required" : "maybe",
  );
}

function nestedRecord(core: ScopeCore): number {
  if (core.record === null) {
    throw new Error("a nested scope is inside a repeat");
  }
  return core.record;
}

function repeat<R>(
  core: ScopeCore,
  site: CallSite | null,
  count: RepeatCount,
  body: ((scope: never) => R) | undefined,
): R;
function repeat(core: ScopeCore, site: CallSite | null, count: RepeatCount, body: unknown): unknown;
function repeat(
  core: ScopeCore,
  site: CallSite | null,
  count: RepeatCount,
  body: unknown,
): unknown {
  const record = core.state.newRecord();
  const index = push(core, { kind: "repeat", count, record }, site);
  return enter(core.state, core.block, index, site, body, (block) =>
    makeScope({ ...core, block, record, depth: core.depth + 1 }),
  );
}

/**
 * Runs a block's callback in a new block after the opener at `opener`, and
 * closes the block with the call site of the call that wrote it. The result
 * is whatever the callback returned; the overloads that call this type it.
 */
function enter<R>(
  state: State,
  parent: number,
  opener: number,
  site: CallSite | null,
  body: ((scope: never) => R) | undefined,
  inner: (block: number) => unknown,
): R;
function enter(
  state: State,
  parent: number,
  opener: number,
  site: CallSite | null,
  body: unknown,
  inner: (block: number) => unknown,
): unknown;
function enter(
  state: State,
  parent: number,
  opener: number,
  site: CallSite | null,
  body: unknown,
  inner: (block: number) => unknown,
): unknown {
  if (body !== undefined && typeof body !== "function") {
    throw new TypeError(`a block's body is a function, not ${typeName(body)}`);
  }
  const block = state.openBlock();
  let result: unknown;
  try {
    result = body === undefined ? undefined : Reflect.apply(body, undefined, [inner(block)]);
  } finally {
    state.closeBlock(parent, opener, site);
  }
  checkSynchronous(state, result, site);
  return result;
}

function checkSynchronous(state: State, result: unknown, site: CallSite | null): void {
  if (
    (typeof result === "object" || typeof result === "function") &&
    result !== null &&
    typeof Reflect.get(result, "then") === "function"
  ) {
    state.fail(
      "scope",
      "a block's callback returned a promise; a block closes when its callback returns, " +
        "so write its statements synchronously",
      site,
    );
  }
}
