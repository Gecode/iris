package dev.gecode.iris.build;

/**
 * A float expression.
 *
 * <p>As a {@link Domain}, it gives a comprehension its elements, which are floats too.
 */
public sealed interface FloatExpr extends NumericExpr, Domain<FloatExpr>
    permits FloatField, Exprs.FloatImpl {
  /** {@return {@code -this}} */
  @Override
  default FloatExpr neg() {
    return new Exprs.FloatImpl(Exprs.negate(this), false);
  }

  /**
   * {@return {@code this[index]}}
   *
   * @param index the index, counted from 0
   */
  default FloatExpr at(IntegralExpr index) {
    return new Exprs.FloatImpl(Exprs.index(Exprs.node(this), Exprs.node(index)), true);
  }

  /**
   * {@return {@code this[index]}}
   *
   * @param index the index, counted from 0
   */
  default FloatExpr at(long index) {
    return new Exprs.FloatImpl(Exprs.index(Exprs.node(this), Exprs.literal(index)), true);
  }
}
