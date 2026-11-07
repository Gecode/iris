package dev.gecode.iris.build;

/**
 * An expression with no static type, which every method accepts.
 *
 * <p>As a {@link Domain}, it gives a comprehension its elements, which are dynamic too.
 */
public sealed interface DynExpr extends IntegralExpr, TextualExpr, Domain<DynExpr>
    permits DynField, Exprs.DynImpl {
  /** {@return {@code -this}} */
  @Override
  default DynExpr neg() {
    return new Exprs.DynImpl(Exprs.negate(this));
  }

  /**
   * {@return {@code this == other}}
   *
   * @param other the right operand
   */
  default IntExpr eq(DynExpr other) {
    return Exprs.binary("==", this, other);
  }

  /**
   * {@return {@code this != other}}
   *
   * @param other the right operand
   */
  default IntExpr ne(DynExpr other) {
    return Exprs.binary("!=", this, other);
  }

  /**
   * {@return {@code this[index]}}
   *
   * @param index the index, counted from 0
   */
  default DynExpr at(IntegralExpr index) {
    return new Exprs.DynImpl(Exprs.index(Exprs.node(this), Exprs.node(index)));
  }

  /**
   * {@return {@code this[index]}}
   *
   * @param index the index, counted from 0
   */
  default DynExpr at(long index) {
    return new Exprs.DynImpl(Exprs.index(Exprs.node(this), Exprs.literal(index)));
  }
}
