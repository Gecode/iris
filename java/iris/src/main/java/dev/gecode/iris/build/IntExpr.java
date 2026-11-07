package dev.gecode.iris.build;

/**
 * An integer expression.
 *
 * <p>As a {@link Domain}, it gives a comprehension its elements, which are integers too.
 */
public sealed interface IntExpr extends IntegralExpr, Domain<IntExpr>
    permits IntField, Exprs.IntImpl {
  /** {@return {@code -this}} */
  @Override
  default IntExpr neg() {
    return new Exprs.IntImpl(Exprs.negate(this), false);
  }

  /**
   * {@return {@code this[index]}}
   *
   * @param index the index, counted from 0
   */
  default IntExpr at(IntegralExpr index) {
    return new Exprs.IntImpl(Exprs.index(Exprs.node(this), Exprs.node(index)), true);
  }

  /**
   * {@return {@code this[index]}}
   *
   * @param index the index, counted from 0
   */
  default IntExpr at(long index) {
    return new Exprs.IntImpl(Exprs.index(Exprs.node(this), Exprs.literal(index)), true);
  }
}
