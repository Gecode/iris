package dev.gecode.iris.build;

/** An expression that integer arithmetic and logic take: an integer or a dynamic value. */
public sealed interface IntegralExpr extends NumericExpr permits IntExpr, DynExpr {
  /**
   * {@return {@code this + other}}
   *
   * @param other the right operand
   */
  default IntExpr plus(IntegralExpr other) {
    return Exprs.binary("+", this, other);
  }

  /**
   * {@return {@code this + other}}
   *
   * @param other the right operand
   */
  default IntExpr plus(long other) {
    return Exprs.binary("+", this, Exprs.literal(other));
  }

  /**
   * {@return {@code this - other}}
   *
   * @param other the right operand
   */
  default IntExpr minus(IntegralExpr other) {
    return Exprs.binary("-", this, other);
  }

  /**
   * {@return {@code this - other}}
   *
   * @param other the right operand
   */
  default IntExpr minus(long other) {
    return Exprs.binary("-", this, Exprs.literal(other));
  }

  /**
   * {@return {@code this * other}}
   *
   * @param other the right operand
   */
  default IntExpr times(IntegralExpr other) {
    return Exprs.binary("*", this, other);
  }

  /**
   * {@return {@code this * other}}
   *
   * @param other the right operand
   */
  default IntExpr times(long other) {
    return Exprs.binary("*", this, Exprs.literal(other));
  }

  /**
   * {@return {@code this / other}, rounding toward zero}
   *
   * @param other the right operand
   */
  default IntExpr div(IntegralExpr other) {
    return Exprs.binary("/", this, other);
  }

  /**
   * {@return {@code this / other}, rounding toward zero}
   *
   * @param other the right operand
   */
  default IntExpr div(long other) {
    return Exprs.binary("/", this, Exprs.literal(other));
  }

  /**
   * {@return {@code this % other}, which has the sign of {@code this}}
   *
   * @param other the right operand
   */
  default IntExpr mod(IntegralExpr other) {
    return Exprs.binary("%", this, other);
  }

  /**
   * {@return {@code this % other}, which has the sign of {@code this}}
   *
   * @param other the right operand
   */
  default IntExpr mod(long other) {
    return Exprs.binary("%", this, Exprs.literal(other));
  }

  /**
   * {@return {@code this ^ other}}
   *
   * @param other the right operand
   */
  default IntExpr pow(IntegralExpr other) {
    return Exprs.binary("^", this, other);
  }

  /**
   * {@return {@code this ^ other}}
   *
   * @param other the right operand
   */
  default IntExpr pow(long other) {
    return Exprs.binary("^", this, Exprs.literal(other));
  }

  /**
   * {@return {@code this && other}}
   *
   * @param other the right operand
   */
  default IntExpr and(IntegralExpr other) {
    return Exprs.binary("&&", this, other);
  }

  /**
   * {@return {@code this && other}}
   *
   * @param other the right operand
   */
  default IntExpr and(long other) {
    return Exprs.binary("&&", this, Exprs.literal(other));
  }

  /**
   * {@return {@code this || other}}
   *
   * @param other the right operand
   */
  default IntExpr or(IntegralExpr other) {
    return Exprs.binary("||", this, other);
  }

  /**
   * {@return {@code this || other}}
   *
   * @param other the right operand
   */
  default IntExpr or(long other) {
    return Exprs.binary("||", this, Exprs.literal(other));
  }

  /** {@return {@code !this}} */
  default IntExpr not() {
    return Exprs.not(this);
  }
}
