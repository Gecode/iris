package dev.gecode.iris.build;

/**
 * An Iris expression. Methods build larger expressions: {@code x.plus(1).le(y)} is {@code x + 1 <=
 * y}. The Java type of an expression is its Iris value type, so that a method takes only the
 * operands Iris accepts; {@link #dynamic} gives an expression that every method takes, for a value
 * whose type is open.
 *
 * <p>A handle is an expression of its own type. In a pattern, a statement or a comprehension body
 * it stands for the current value of its name.
 */
public sealed interface Expr permits NumericExpr, TextualExpr {
  /** {@return this expression with no static type, which every method accepts} */
  default DynExpr dynamic() {
    return new Exprs.DynImpl(Exprs.node(this));
  }

  /**
   * {@return {@code this.name}, the field of the record this selects}
   *
   * @param field the handle of the field
   */
  default IntExpr get(IntField<?> field) {
    return new Exprs.IntImpl(Exprs.member(Exprs.node(this), field), true);
  }

  /**
   * {@return {@code this.name}, the field of the record this selects}
   *
   * @param field the handle of the field
   */
  default FloatExpr get(FloatField<?> field) {
    return new Exprs.FloatImpl(Exprs.member(Exprs.node(this), field), true);
  }

  /**
   * {@return {@code this.name}, the field of the record this selects}
   *
   * @param field the handle of the field
   */
  default NumExpr get(NumField<?> field) {
    return new Exprs.NumImpl(Exprs.member(Exprs.node(this), field), true);
  }

  /**
   * {@return {@code this.name}, the field of the record this selects}
   *
   * @param field the handle of the field
   */
  default StringExpr get(StringField<?> field) {
    return new Exprs.StringImpl(Exprs.member(Exprs.node(this), field), true);
  }

  /**
   * {@return {@code this.name}, the field of the record this selects}
   *
   * @param field the handle of the field
   */
  default DynExpr get(DynField field) {
    return new Exprs.DynImpl(Exprs.member(Exprs.node(this), field));
  }
}
