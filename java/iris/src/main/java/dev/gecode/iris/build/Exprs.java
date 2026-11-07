package dev.gecode.iris.build;

import dev.gecode.iris.Field;
import java.util.List;
import java.util.function.Function;
import org.jspecify.annotations.Nullable;

/** The expressions that are not handles, and the operations that combine expressions. */
final class Exprs {
  private Exprs() {}

  /** An integer expression; {@code open} if it looks up a stored value, which may be an array. */
  record IntImpl(Node node, boolean open) implements IntExpr {}

  record FloatImpl(Node node, boolean open) implements FloatExpr {}

  record NumImpl(Node node, boolean open) implements NumExpr {}

  record StringImpl(Node node, boolean open) implements StringExpr {}

  record DynImpl(Node node) implements DynExpr {}

  static Node node(Expr expr) {
    return switch (expr) {
      case IntImpl e -> e.node();
      case FloatImpl e -> e.node();
      case NumImpl e -> e.node();
      case StringImpl e -> e.node();
      case DynImpl e -> e.node();
      case IntField<?> f -> f.node();
      case FloatField<?> f -> f.node();
      case NumField<?> f -> f.node();
      case StringField<?> f -> f.node();
      case DynField f -> f.node();
    };
  }

  static Tag tag(Expr expr) {
    return switch (expr) {
      case IntExpr e -> Tag.INT;
      case FloatExpr e -> Tag.FLOAT;
      case NumExpr e -> Tag.NUM;
      case StringExpr e -> Tag.STR;
      case DynExpr e -> Tag.DYN;
    };
  }

  /** Whether an expression looks up a stored value, which may be an array. */
  static boolean open(Expr expr) {
    return switch (expr) {
      case IntImpl e -> e.open();
      case FloatImpl e -> e.open();
      case NumImpl e -> e.open();
      case StringImpl e -> e.open();
      default -> true;
    };
  }

  /** The handle of a field from this package. */
  static Handle handle(Field<?> field) {
    return switch (field) {
      case IntField<?> f -> f.handle;
      case FloatField<?> f -> f.handle;
      case NumField<?> f -> f.handle;
      case StringField<?> f -> f.handle;
      case DynField f -> f.handle;
      default ->
          throw new IllegalArgumentException(
              "a handle comes from dev.gecode.iris.build, not " + field.getClass().getName());
    };
  }

  /** A field for {@code handle}, of the class its value type gives. */
  static Field<?> field(Handle handle) {
    return switch (handle.tag()) {
      case INT -> new IntField<>(handle);
      case FLOAT -> new FloatField<>(handle);
      case NUM -> new NumField<>(handle);
      case STR -> new StringField<>(handle);
      case DYN -> new DynField(handle);
    };
  }

  /**
   * The array levels a {@code let} of {@code value} adds in a block at {@code depth}, as far as its
   * type says: a lookup may be an array, except a bare handle at the top level, whose depth is its
   * own.
   */
  static @Nullable Integer letLevels(Expr value, int depth) {
    if (value instanceof DynExpr) {
      return null;
    }
    if (value instanceof Field<?> field) {
      return depth == 0 ? handle(field).levels() : null;
    }
    return open(value) ? null : 0;
  }

  static IntExpr intExpr(Node node) {
    return new IntImpl(node, false);
  }

  static Node literal(long value) {
    return new Node(new Op.IntOp(value));
  }

  static Node literal(double value) {
    return new Node(new Op.FloatOp(value));
  }

  static Node literal(String value) {
    return new Node(new Op.StrOp(value));
  }

  static IntExpr binary(String symbol, Expr left, Expr right) {
    return intExpr(new Node(new Op.BinaryOp(symbol), node(left), node(right)));
  }

  static IntExpr binary(String symbol, Expr left, Node right) {
    return intExpr(new Node(new Op.BinaryOp(symbol), node(left), right));
  }

  static Node negate(Expr expr) {
    return new Node(new Op.UnaryOp("-"), node(expr));
  }

  static IntExpr not(Expr expr) {
    return intExpr(new Node(new Op.UnaryOp("!"), node(expr)));
  }

  static Node index(Node base, Node index) {
    return new Node(Op.IndexOp.INSTANCE, base, index);
  }

  static Node member(Node base, Field<?> field) {
    return new Node(new Op.MemberOp(handle(field)), base);
  }

  static Node call(String function, List<? extends Expr> arguments) {
    if (arguments.isEmpty()) {
      throw new IllegalArgumentException("`" + function + "` takes at least one argument");
    }
    return new Node(
        new Op.CallOp(function, arguments.size()), arguments.stream().map(Exprs::node).toList());
  }

  /** The expression that a domain stands for in a comprehension or a call. */
  static Node domain(Domain<?> domain) {
    return switch (domain) {
      case Expr e -> node(e);
      case RecordName r -> new Node(new Op.RecordListOp(r));
      case RecordList l -> l.node;
      case Over<?> o -> domain(o.domain);
    };
  }

  /** What a comprehension over {@code domain} gives its body. */
  @SuppressWarnings("unchecked")
  static <E> E element(Domain<E> domain, Node node) {
    Object element =
        switch (domain) {
          case IntExpr e -> new IntImpl(node, true);
          case FloatExpr e -> new FloatImpl(node, true);
          case NumExpr e -> new NumImpl(node, true);
          case StringExpr e -> new StringImpl(node, true);
          case DynExpr e -> new DynImpl(node);
          case RecordName r -> new RecordExpr(node);
          case RecordList l -> new RecordExpr(node);
          case Over<?> o -> element(o.domain, node);
        };
    return (E) element;
  }

  static <E> IntExpr comprehension(
      String function, Domain<E> domain, Function<? super E, ? extends Expr> body) {
    Node over = domain(domain);
    String name = domain instanceof Over<?> o ? o.name : null;
    Op.Binding binding = new Op.Binding();
    Expr result = body.apply(element(domain, new Node(new Op.BindingOp(binding))));
    return intExpr(new Node(new Op.ComprehensionOp(function, binding, name), over, node(result)));
  }

  /** The node of a range bound, which is an integer or a {@code num}. */
  static Node bound(NumericExpr bound) {
    if (bound instanceof FloatExpr) {
      throw new IllegalArgumentException("a range bound is an int or num, not a float");
    }
    return node(bound);
  }
}
