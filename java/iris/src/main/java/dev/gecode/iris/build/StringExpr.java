package dev.gecode.iris.build;

/**
 * A string expression.
 *
 * <p>As a {@link Domain}, it gives a comprehension its elements, which are strings too.
 */
public sealed interface StringExpr extends TextualExpr, Domain<StringExpr>
    permits StringField, Exprs.StringImpl {
  /**
   * {@return {@code this[index]}}
   *
   * @param index the index, counted from 0
   */
  default StringExpr at(IntegralExpr index) {
    return new Exprs.StringImpl(Exprs.index(Exprs.node(this), Exprs.node(index)), true);
  }

  /**
   * {@return {@code this[index]}}
   *
   * @param index the index, counted from 0
   */
  default StringExpr at(long index) {
    return new Exprs.StringImpl(Exprs.index(Exprs.node(this), Exprs.literal(index)), true);
  }
}
