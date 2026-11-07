package dev.gecode.iris.build;

/**
 * A {@code num} expression: an integer or a float, decided by the data.
 *
 * <p>As a {@link Domain}, it gives a comprehension its elements, which are numbers too.
 */
public sealed interface NumExpr extends NumericExpr, Domain<NumExpr>
    permits NumField, Exprs.NumImpl {
  /** {@return {@code -this}} */
  @Override
  default NumExpr neg() {
    return new Exprs.NumImpl(Exprs.negate(this), false);
  }

  /**
   * {@return {@code this[index]}}
   *
   * @param index the index, counted from 0
   */
  default NumExpr at(IntegralExpr index) {
    return new Exprs.NumImpl(Exprs.index(Exprs.node(this), Exprs.node(index)), true);
  }

  /**
   * {@return {@code this[index]}}
   *
   * @param index the index, counted from 0
   */
  default NumExpr at(long index) {
    return new Exprs.NumImpl(Exprs.index(Exprs.node(this), Exprs.literal(index)), true);
  }
}
