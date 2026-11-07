package dev.gecode.iris.build;

/** An expression that comparisons take: an integer, a float, a {@code num} or a dynamic value. */
public sealed interface NumericExpr extends Expr permits IntegralExpr, FloatExpr, NumExpr {
  /** {@return {@code -this}} */
  NumericExpr neg();

  /**
   * {@return {@code this < other}}
   *
   * @param other the right operand
   */
  default IntExpr lt(NumericExpr other) {
    return Exprs.binary("<", this, other);
  }

  /**
   * {@return {@code this < other}}
   *
   * @param other the right operand
   */
  default IntExpr lt(long other) {
    return Exprs.binary("<", this, Exprs.literal(other));
  }

  /**
   * {@return {@code this < other}}
   *
   * @param other the right operand
   */
  default IntExpr lt(double other) {
    return Exprs.binary("<", this, Exprs.literal(other));
  }

  /**
   * {@return {@code this <= other}}
   *
   * @param other the right operand
   */
  default IntExpr le(NumericExpr other) {
    return Exprs.binary("<=", this, other);
  }

  /**
   * {@return {@code this <= other}}
   *
   * @param other the right operand
   */
  default IntExpr le(long other) {
    return Exprs.binary("<=", this, Exprs.literal(other));
  }

  /**
   * {@return {@code this <= other}}
   *
   * @param other the right operand
   */
  default IntExpr le(double other) {
    return Exprs.binary("<=", this, Exprs.literal(other));
  }

  /**
   * {@return {@code this > other}}
   *
   * @param other the right operand
   */
  default IntExpr gt(NumericExpr other) {
    return Exprs.binary(">", this, other);
  }

  /**
   * {@return {@code this > other}}
   *
   * @param other the right operand
   */
  default IntExpr gt(long other) {
    return Exprs.binary(">", this, Exprs.literal(other));
  }

  /**
   * {@return {@code this > other}}
   *
   * @param other the right operand
   */
  default IntExpr gt(double other) {
    return Exprs.binary(">", this, Exprs.literal(other));
  }

  /**
   * {@return {@code this >= other}}
   *
   * @param other the right operand
   */
  default IntExpr ge(NumericExpr other) {
    return Exprs.binary(">=", this, other);
  }

  /**
   * {@return {@code this >= other}}
   *
   * @param other the right operand
   */
  default IntExpr ge(long other) {
    return Exprs.binary(">=", this, Exprs.literal(other));
  }

  /**
   * {@return {@code this >= other}}
   *
   * @param other the right operand
   */
  default IntExpr ge(double other) {
    return Exprs.binary(">=", this, Exprs.literal(other));
  }

  /**
   * {@return {@code this == other}}
   *
   * @param other the right operand
   */
  default IntExpr eq(NumericExpr other) {
    return Exprs.binary("==", this, other);
  }

  /**
   * {@return {@code this == other}}
   *
   * @param other the right operand
   */
  default IntExpr eq(long other) {
    return Exprs.binary("==", this, Exprs.literal(other));
  }

  /**
   * {@return {@code this == other}}
   *
   * @param other the right operand
   */
  default IntExpr eq(double other) {
    return Exprs.binary("==", this, Exprs.literal(other));
  }

  /**
   * {@return {@code this != other}}
   *
   * @param other the right operand
   */
  default IntExpr ne(NumericExpr other) {
    return Exprs.binary("!=", this, other);
  }

  /**
   * {@return {@code this != other}}
   *
   * @param other the right operand
   */
  default IntExpr ne(long other) {
    return Exprs.binary("!=", this, Exprs.literal(other));
  }

  /**
   * {@return {@code this != other}}
   *
   * @param other the right operand
   */
  default IntExpr ne(double other) {
    return Exprs.binary("!=", this, Exprs.literal(other));
  }
}
