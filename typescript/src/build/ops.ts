// Expression operations and the tree that holds them.
//
// An expression is a tree of nodes, so that composing two expressions takes
// constant time. A statement flattens each of its expressions once into
// postfix operations, in which every operation pops its operands' results and
// pushes one, so that a chain of any length checks and prints without
// recursion.

/** A value type as data: `dyn` is a value with no static type. */
export type ValueTag = "int" | "float" | "num" | "string" | "dyn";

const DESCRIPTIONS: Readonly<Record<ValueTag, string>> = {
  int: "int",
  float: "float",
  num: "num",
  string: "string",
  dyn: "dynamic",
};

export function describeTag(tag: ValueTag): string {
  return DESCRIPTIONS[tag];
}

/**
 * A comprehension variable. Printed names never depend on its identity; they
 * are numbered by first appearance in the text.
 */
export type Binding = symbol;

export function newBinding(): Binding {
  return Symbol("binding");
}

export type Op =
  | { readonly kind: "int"; readonly value: bigint }
  | { readonly kind: "float"; readonly value: number }
  | { readonly kind: "string"; readonly text: string }
  /** A host constant such as `gecode.int_max`. */
  | { readonly kind: "host"; readonly name: string }
  | { readonly kind: "name"; readonly text: string; readonly owner: number }
  | { readonly kind: "binding"; readonly binding: Binding }
  /** The `each` name of a repeat. */
  | { readonly kind: "record"; readonly record: number; readonly owner: number }
  /** The `as` name of a repeat. */
  | { readonly kind: "recordList"; readonly record: number; readonly owner: number }
  | { readonly kind: "unary"; readonly symbol: "-" | "!" }
  | { readonly kind: "binary"; readonly symbol: BinarySymbol }
  | { readonly kind: "index" }
  /** `.name` on the preceding operand. */
  | { readonly kind: "member"; readonly text: string; readonly owner: number }
  /** `.records` on the preceding operand, with a repeat's `as` name. */
  | { readonly kind: "listMember"; readonly record: number; readonly owner: number }
  | { readonly kind: "call"; readonly function: string; readonly arguments: number }
  | {
      readonly kind: "comprehension";
      readonly function: string;
      readonly binding: Binding;
      readonly name: string | null;
    };

export type BinarySymbol =
  | "||"
  | "&&"
  | "=="
  | "!="
  | "<"
  | "<="
  | ">"
  | ">="
  | "+"
  | "-"
  | "*"
  | "/"
  | "%"
  | "^";

/** One expression as postfix operations. */
export type Ops = readonly Op[];

export interface Node {
  readonly op: Op;
  readonly children: readonly Node[];
}

export function node(op: Op, children: readonly Node[] = []): Node {
  return { op, children };
}

/** How many operands an operation pops. */
export function operandCount(op: Op): number {
  switch (op.kind) {
    case "call":
      return op.arguments;
    case "unary":
    case "member":
    case "listMember":
      return 1;
    case "binary":
    case "index":
    case "comprehension":
      return 2;
    default:
      return 0;
  }
}

/** The operations of a tree in postfix order. */
export function postfix(root: Node): Op[] {
  const ops: Op[] = [];
  const pending: [Node, boolean][] = [[root, false]];
  for (let item = pending.pop(); item !== undefined; item = pending.pop()) {
    const [current, expanded] = item;
    if (expanded || current.children.length === 0) {
      ops.push(current.op);
      continue;
    }
    pending.push([current, true]);
    for (let index = current.children.length - 1; index >= 0; index--) {
      const child = current.children[index];
      if (child !== undefined) {
        pending.push([child, false]);
      }
    }
  }
  return ops;
}
