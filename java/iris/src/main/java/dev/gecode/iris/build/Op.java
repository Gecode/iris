package dev.gecode.iris.build;

import org.jspecify.annotations.Nullable;

/**
 * An expression operation. A statement flattens each expression into postfix operations, in which
 * every operation pops its operands' results and pushes one, so that an expression of any length
 * checks and prints without recursion.
 */
sealed interface Op {
  /** How many operands this operation pops. */
  default int operands() {
    return 0;
  }

  record IntOp(long value) implements Op {}

  record FloatOp(double value) implements Op {}

  record StrOp(String text) implements Op {}

  /** A host constant such as {@code gecode.int_max}. */
  record HostOp(String name) implements Op {}

  record NameOp(Handle handle) implements Op {}

  record BindingOp(Binding binding) implements Op {}

  /** The {@code each} name of a repeat. */
  record RecordOp(RecordName record) implements Op {}

  /** The {@code as} name of a repeat. */
  record RecordListOp(RecordName record) implements Op {}

  record UnaryOp(String symbol) implements Op {
    @Override
    public int operands() {
      return 1;
    }
  }

  record BinaryOp(String symbol) implements Op {
    @Override
    public int operands() {
      return 2;
    }
  }

  record IndexOp() implements Op {
    static final IndexOp INSTANCE = new IndexOp();

    @Override
    public int operands() {
      return 2;
    }
  }

  /** {@code .name} on the preceding operand. */
  record MemberOp(Handle handle) implements Op {
    @Override
    public int operands() {
      return 1;
    }
  }

  /** {@code .records} on the preceding operand, with a repeat's {@code as} name. */
  record ListMemberOp(RecordName record) implements Op {
    @Override
    public int operands() {
      return 1;
    }
  }

  record CallOp(String function, int arguments) implements Op {
    @Override
    public int operands() {
      return arguments;
    }
  }

  record ComprehensionOp(String function, Binding binding, @Nullable String name) implements Op {
    @Override
    public int operands() {
      return 2;
    }
  }

  /**
   * A comprehension variable. Printed names never depend on its identity; they are numbered by
   * first appearance in the text.
   */
  final class Binding {}
}
