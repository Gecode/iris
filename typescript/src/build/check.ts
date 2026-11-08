// The binding check: one walk over the statements, which knows the depth and
// presence of each definition, so `optional` counts wherever it is given.
import type { CallSite } from "../call-sites.js";
import { BuildError, type BuildErrorKind } from "./error.js";
import { describeTag, type ValueTag } from "./ops.js";
import type { Piece } from "./pattern.js";
import {
  declaredMentions,
  isOps,
  mentions,
  opensBlock,
  type Definition,
  type NamedMention,
  type Shape,
  type State,
  type Stmt,
} from "./statement.js";

interface Frame {
  /** Identifies this block. Open blocks are numbered in nesting order. */
  readonly block: number;
  /** The repeats around this block. */
  readonly depth: number;
  readonly required: boolean;
}

interface Bound {
  count: number;
  required: boolean;
  /** The block of a binding, which always precedes what follows it there. */
  block: number | null;
}

/** A repeat's `store`, which runs when the repeat ends. */
interface PendingStore {
  readonly stmt: number;
  readonly repeatBlock: number;
  readonly store: Definition;
  readonly frame: Frame;
}

/** A `type` statement, in the block whose end hides it. */
interface TypeDeclaration {
  readonly stmt: number;
  readonly block: number;
}

/** Checks every capture, forward handle and named type against how it was declared. */
export function checkBindings(state: State): void {
  const walk = new Walk(state);
  state.stmts.forEach((stmt, index) => {
    walk.current = index;
    const frame = walk.frame();
    for (const mention of mentions(stmt)) {
      if (!isOps(mention) && mention.kind === "named") {
        walk.typeUse(mention);
      }
    }
    walk.recordDefinitions(index, stmt, frame);
    walk.followBlocks(stmt, frame);
  });
  walk.finish();
}

function failAt(site: CallSite | null, message: string, kind: BuildErrorKind = "binding"): never {
  throw BuildError.check(kind, message, site);
}

/**
 * Whether a definition certainly has the shape a forward handle promises. A
 * value type or depth that is not known agrees only with an open promise.
 */
function agrees(promised: Shape, value: ValueTag, levels: number | null): boolean {
  let valueAgrees: boolean;
  if (promised.value === "dyn") {
    valueAgrees = true;
  } else if (promised.value === "num") {
    valueAgrees = value === "int" || value === "float" || value === "num";
  } else {
    valueAgrees = promised.value === value;
  }
  return valueAgrees && (promised.levels === null || promised.levels === levels);
}

function describeShape(value: ValueTag, levels: number | null, openDepth: string): string {
  return levels === null
    ? `${describeTag(value)} at ${openDepth} depth`
    : `${describeTag(value)} at depth ${levels}`;
}

class Walk {
  readonly state: State;
  /** The index of the statement being checked. */
  current = 0;
  readonly bound: Map<string, Bound> = new Map();
  readonly frames: Frame[] = [{ block: 0, depth: 0, required: true }];
  nextBlock = 0;
  /** The stores of the open repeats, innermost last. */
  readonly stores: PendingStore[] = [];
  /**
   * The declarations of each type name so far, latest last. Those of closed
   * blocks are dropped when a use finds them.
   */
  readonly types: Map<string, TypeDeclaration[]> = new Map();

  constructor(state: State) {
    this.state = state;
  }

  frame(): Frame {
    const frame = this.frames.at(-1);
    if (frame === undefined) {
      throw new Error("the binding walk has no open block");
    }
    return frame;
  }

  site(index: number): CallSite | null {
    return this.state.sites[index] ?? null;
  }

  /** Fails the statement being checked. */
  fail(message: string): never {
    failAt(this.site(this.current), message);
  }

  isOpen(block: number): boolean {
    return this.frames.some((frame) => frame.block === block);
  }

  /** `frame` with the number of a block of its own. */
  ownBlock(frame: Frame): Frame {
    this.nextBlock += 1;
    return { ...frame, block: this.nextBlock };
  }

  enter(frame: Frame): number {
    const own = this.ownBlock(frame);
    this.frames.push(own);
    return own.block;
  }

  recordBinding(name: string, frame: Frame): number {
    const precedes = this.bindingPrecedes(name);
    let bound = this.bound.get(name);
    if (bound === undefined) {
      bound = { count: 0, required: false, block: null };
      this.bound.set(name, bound);
    }
    bound.count += 1;
    bound.required ||= frame.required;
    if (!precedes) {
      bound.block = frame.block;
    }
    return bound.count;
  }

  /** Whether a binding of `name` always executes before the current statement. */
  bindingPrecedes(name: string): boolean {
    const bound = this.bound.get(name);
    return bound !== undefined && bound.block !== null && this.isOpen(bound.block);
  }

  /** A statement defines `name` in `frame`, with `levels` in all. */
  bind(name: string, value: ValueTag, levels: number | null, frame: Frame): void {
    const state = this.state.names.get(name);
    if (state?.kind === "undefined") {
      this.fail(
        `\`${name}\` comes from \`undefined\`, so nothing may define it; ` +
          "use `forward` for a name that is defined",
      );
    }
    if (state?.kind !== "forward") {
      return;
    }
    if (agrees(state.shape, value, levels)) {
      this.recordBinding(name, frame);
    } else if (!this.bindingPrecedes(name)) {
      // After a binding, this is an explicit redefinition, which leaves the
      // handle stale.
      const { shape } = state;
      this.fail(
        `the forward handle \`${name}\` is ${describeShape(shape.value, shape.levels, "any")}, ` +
          `but this definition is ${describeShape(value, levels, "an unknown")}; ` +
          "declare it with the definition's type, or with no value type",
      );
    }
  }

  /** A statement other than `store` defines `definition` in `frame`. */
  definition(definition: Definition, frame: Frame): void {
    const { name } = definition;
    if (name === null) {
      return;
    }
    if (name === "_") {
      this.fail(
        "only `store` defines the entry named `_`: a declaration, a list or a " +
          "capture discards it, and `let` and `param` reject it",
      );
    }
    const levels = definition.levels === null ? null : frame.depth + definition.levels;
    this.bind(name, definition.value, levels, frame);
  }

  /**
   * A `store` defines `store` in `frame`. No repeat collects `_` into a
   * column, so inside one it lasts for the iteration and leaves no entry.
   */
  store(store: Definition, frame: Frame): void {
    const { name } = store;
    if (name === null) {
      return;
    }
    if (name === "_" && frame.depth > 0) {
      this.bind(name, store.value, store.levels, { ...frame, required: false });
      return;
    }
    const levels = store.levels === null ? null : frame.depth + store.levels;
    this.bind(name, store.value, levels, frame);
  }

  pattern(pieces: readonly Piece[], frame: Frame): void {
    for (const piece of pieces) {
      if (piece.kind === "capture") {
        this.capture(piece.name, frame);
      } else if (piece.kind === "bind") {
        this.definition({ name: piece.name, value: piece.value, levels: 0 }, frame);
      }
    }
  }

  /** A pattern binds the capture `name` in `frame`. */
  capture(name: string, frame: Frame): void {
    const state = this.state.names.get(name);
    if (state?.kind !== "capture") {
      this.fail(
        `\`${name}\` is not a capture; use \`bind\` to define it in a pattern, ` +
          "or `ref` to match its value",
      );
    }
    if (this.recordBinding(name, frame) > 1) {
      this.fail(
        `the capture \`${name}\` is bound twice; a capture belongs to one pattern, ` +
          "so use `ref` to match its value again",
      );
    }
    const { shape } = state;
    if (shape.levels !== null && shape.levels !== frame.depth) {
      this.fail(
        `the capture \`${name}\` is bound in a different scope: create it with ` +
          "`capture` in the scope of the pattern",
      );
    }
    if (shape.required !== frame.required) {
      const form = frame.required ? "capture" : "optionalCapture";
      this.fail(`the capture \`${name}\` is bound where its presence differs: use \`${form}\``);
    }
    if (state.spec !== null) {
      for (const mention of declaredMentions(state.spec)) {
        if (!isOps(mention) && mention.kind === "named") {
          this.typeUse(mention);
        }
      }
    }
  }

  /** A statement uses the named type `named`, which must be the one visible. */
  typeUse(named: NamedMention): void {
    let declarations = this.types.get(named.name);
    if (declarations === undefined) {
      declarations = [];
      this.types.set(named.name, declarations);
    }
    while (declarations.length > 0 && !this.isOpen(declarations.at(-1)?.block ?? -1)) {
      declarations.pop();
    }
    if (declarations.at(-1)?.stmt !== named.declaration) {
      failAt(
        this.site(this.current),
        `the declaration of the type \`${named.name}\` that this handle came from ` +
          "is not the one visible here",
        "scope",
      );
    }
  }

  finish(): void {
    for (const { name, site } of this.state.declared) {
      const bound = this.bound.get(name) ?? { count: 0, required: false, block: null };
      const state = this.state.names.get(name);
      if (state?.kind === "capture" && bound.count === 0) {
        failAt(site, `the capture \`${name}\` is never bound; add it to a pattern`);
      }
      if (state?.kind === "forward" && bound.count === 0) {
        failAt(
          site,
          `the forward handle \`${name}\` is never bound; define it, ` +
            "or use `undefined` for a name left undefined",
        );
      }
      if (state?.kind === "forward" && state.shape.required && !bound.required) {
        failAt(
          site,
          `the forward handle \`${name}\` is required, but every definition ` +
            "may not execute; declare it with `{ optional: true }`",
        );
      }
    }
    for (const record of this.state.records) {
      if (record.forward !== null && !record.bound) {
        failAt(
          record.forward.site,
          `the forward record \`${record.each ?? ""}\` is never bound; ` +
            "use `recordAs` in its repeat",
        );
      }
    }
  }

  /** Records what `stmt`, at `index`, defines in `frame`. */
  recordDefinitions(index: number, stmt: Stmt, frame: Frame): void {
    switch (stmt.kind) {
      case "param":
      case "declaration":
      case "list":
      case "let":
        this.definition(stmt.definition, frame);
        break;
      case "elseDeclaration":
        this.definition(stmt.definition, this.ownBlock({ ...frame, required: false }));
        break;
      case "type": {
        const declarations = this.types.get(stmt.name) ?? [];
        declarations.push({ stmt: index, block: frame.block });
        this.types.set(stmt.name, declarations);
        break;
      }
      case "line":
        this.pattern(stmt.pieces, frame);
        break;
      case "sections": {
        // End of file and `last` end sections without storing.
        const optional = this.ownBlock({ ...frame, required: false });
        for (const clause of stmt.clauses) {
          if (clause.kind === "until" && clause.store !== null) {
            this.store(clause.store, optional);
          }
        }
        break;
      }
      default:
        break;
    }
  }

  /**
   * Opens or closes the block `stmt` starts or ends, and checks an arm's
   * pattern in its own block.
   */
  followBlocks(stmt: Stmt, frame: Frame): void {
    if (stmt.kind === "close") {
      const closed = this.frames.pop();
      const pending = this.stores.at(-1);
      if (pending !== undefined && pending.repeatBlock === closed?.block) {
        this.stores.pop();
        this.current = pending.stmt;
        this.store(pending.store, pending.frame);
      }
    } else if (stmt.kind === "repeat") {
      const repeatBlock = this.enter({ ...frame, depth: frame.depth + 1 });
      const { count } = stmt;
      if (count.kind === "until" && count.store !== null) {
        this.stores.push({ stmt: this.current, repeatBlock, store: count.store, frame });
      }
    } else if (stmt.kind === "if" || stmt.kind === "elseIf" || stmt.kind === "else") {
      this.enter({ ...frame, required: false });
    } else if (stmt.kind === "arm") {
      const arm = this.ownBlock({ ...frame, required: frame.required && !stmt.optional });
      this.pattern(stmt.pieces, arm);
      if (stmt.body) {
        this.frames.push(arm);
      }
    } else if (opensBlock(stmt)) {
      this.enter(frame);
    }
  }
}
