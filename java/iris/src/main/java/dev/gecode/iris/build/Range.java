package dev.gecode.iris.build;

/**
 * The integers from a lower bound up: {@code lo..}. {@link #to} and {@link #below} give it an upper
 * bound.
 */
public final class Range extends IntType {
  private final Node lower;

  Range(Node lower) {
    super(new Declared(new Declared.RangeSpec(lower.postfix(), null, true)));
    this.lower = lower;
  }

  /**
   * {@return {@code lo..hi}, with both bounds included}
   *
   * @param hi the upper bound
   * @throws IllegalArgumentException if the bound is a float
   */
  public IntType to(NumericExpr hi) {
    return bounded(Exprs.bound(hi), true);
  }

  /**
   * {@return {@code lo..hi}, with both bounds included}
   *
   * @param hi the upper bound
   */
  public IntType to(long hi) {
    return bounded(Exprs.literal(hi), true);
  }

  /**
   * {@return {@code lo..<hi}, without the upper bound}
   *
   * @param hi the upper bound
   * @throws IllegalArgumentException if the bound is a float
   */
  public IntType below(NumericExpr hi) {
    return bounded(Exprs.bound(hi), false);
  }

  /**
   * {@return {@code lo..<hi}, without the upper bound}
   *
   * @param hi the upper bound
   */
  public IntType below(long hi) {
    return bounded(Exprs.literal(hi), false);
  }

  private IntType bounded(Node upper, boolean inclusive) {
    return new IntType(
        new Declared(new Declared.RangeSpec(lower.postfix(), upper.postfix(), inclusive)));
  }
}
