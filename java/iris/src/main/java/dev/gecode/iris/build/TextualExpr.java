package dev.gecode.iris.build;

/** An expression that string equality takes: a string or a dynamic value. */
public sealed interface TextualExpr extends Expr permits StringExpr, DynExpr {
  /**
   * {@return {@code this == other}}
   *
   * @param other the right operand
   */
  default IntExpr eq(TextualExpr other) {
    return Exprs.binary("==", this, other);
  }

  /**
   * {@return {@code this == other}}
   *
   * @param other the right operand
   */
  default IntExpr eq(String other) {
    return Exprs.binary("==", this, Exprs.literal(other));
  }

  /**
   * {@return {@code this != other}}
   *
   * @param other the right operand
   */
  default IntExpr ne(TextualExpr other) {
    return Exprs.binary("!=", this, other);
  }

  /**
   * {@return {@code this != other}}
   *
   * @param other the right operand
   */
  default IntExpr ne(String other) {
    return Exprs.binary("!=", this, Exprs.literal(other));
  }
}
