package dev.gecode.iris.build;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Deque;
import java.util.List;

/**
 * An expression tree, so that composing two expressions takes constant time.
 *
 * @param op the operation
 * @param children its operands
 */
record Node(Op op, List<Node> children) {
  Node(Op op) {
    this(op, List.of());
  }

  Node(Op op, Node child) {
    this(op, List.of(child));
  }

  Node(Op op, Node left, Node right) {
    this(op, List.of(left, right));
  }

  /** The operations of this tree in postfix order. */
  List<Op> postfix() {
    List<Op> ops = new ArrayList<>();
    Deque<Node> pending = new ArrayDeque<>();
    Deque<Boolean> expanded = new ArrayDeque<>();
    pending.push(this);
    expanded.push(false);
    while (!pending.isEmpty()) {
      Node current = pending.pop();
      boolean done = expanded.pop();
      if (done || current.children.isEmpty()) {
        ops.add(current.op);
        continue;
      }
      pending.push(current);
      expanded.push(true);
      for (Node child : current.children.reversed()) {
        pending.push(child);
        expanded.push(false);
      }
    }
    return List.copyOf(ops);
  }
}
