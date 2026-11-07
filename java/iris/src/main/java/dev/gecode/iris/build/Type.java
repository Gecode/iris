package dev.gecode.iris.build;

import java.util.ArrayList;
import java.util.List;

/**
 * A reading type, such as {@code int}, {@code 0..<n} or {@code word}. {@link #dim} adds a
 * dimension, and {@link #layout} and {@link #offset} add the options of a declaration.
 *
 * @param <T> the type that adding a dimension or an option gives
 */
public abstract sealed class Type<T extends Type<T>>
    permits IntType, FloatType, NumType, StringType {
  final Declared declared;

  Type(Declared declared) {
    this.declared = declared;
  }

  abstract Tag tag();

  abstract T with(Declared value);

  /**
   * {@return this type with another dimension: {@code x[size]: type}}
   *
   * @param size the number of elements
   */
  public final T dim(Expr size) {
    return with(declared.withDimensions(List.of(Exprs.node(size).postfix())));
  }

  /**
   * {@return this type with another dimension: {@code x[size]: type}}
   *
   * @param size the number of elements
   */
  public final T dim(long size) {
    return with(declared.withDimensions(List.of(Exprs.literal(size).postfix())));
  }

  /**
   * {@return this type with several more dimensions, as many as known only at run time}
   *
   * @param sizes the numbers of elements, outermost first
   */
  public final T dims(long... sizes) {
    List<List<Op>> added = new ArrayList<>(sizes.length);
    for (long size : sizes) {
      added.add(Exprs.literal(size).postfix());
    }
    return with(declared.withDimensions(added));
  }

  /**
   * {@return this type with several more dimensions, as many as known only at run time}
   *
   * @param sizes the numbers of elements, outermost first
   */
  public final T dims(List<? extends Expr> sizes) {
    return with(
        declared.withDimensions(sizes.stream().map(size -> Exprs.node(size).postfix()).toList()));
  }

  /**
   * {@return this type with {@code layout expression}}
   *
   * @param layout the layout
   */
  public final T layout(Expr layout) {
    return with(declared.withLayout(Exprs.node(layout).postfix()));
  }

  /**
   * {@return this type with {@code layout expression}}
   *
   * @param layout the layout
   */
  public final T layout(long layout) {
    return with(declared.withLayout(Exprs.literal(layout).postfix()));
  }

  /**
   * {@return this type with {@code offset expression}}
   *
   * @param offset the offset
   */
  public final T offset(Expr offset) {
    return with(declared.withOffset(Exprs.node(offset).postfix()));
  }

  /**
   * {@return this type with {@code offset expression}}
   *
   * @param offset the offset
   */
  public final T offset(long offset) {
    return with(declared.withOffset(Exprs.literal(offset).postfix()));
  }

  /**
   * {@return a list of this type that ends at the sentinel {@code end}}
   *
   * @param end the sentinel
   * @throws IllegalArgumentException if this type has dimensions
   */
  public final ListReading until(Expr end) {
    return until(Exprs.node(end));
  }

  /**
   * {@return a list of this type that ends at the sentinel {@code end}}
   *
   * @param end the sentinel
   * @throws IllegalArgumentException if this type has dimensions
   */
  public final ListReading until(long end) {
    return until(Exprs.literal(end));
  }

  /**
   * {@return a list of this type that ends at the sentinel {@code end}}
   *
   * @param end the sentinel
   * @throws IllegalArgumentException if this type has dimensions
   */
  public final ListReading until(String end) {
    return until(Exprs.literal(end));
  }

  /**
   * {@return a list of this type that ends at the end of the line: {@code until eol}}
   *
   * @throws IllegalArgumentException if this type has dimensions
   */
  public final ListReading untilEol() {
    return new ListReading(tag(), scalar("a list's type"), null);
  }

  private ListReading until(Node end) {
    return new ListReading(tag(), scalar("a list's type"), end.postfix());
  }

  /** The declaration of this type, which must have no dimensions. */
  final Declared scalar(String role) {
    if (!declared.dimensions().isEmpty()) {
      throw new IllegalArgumentException(role + " has no dimensions");
    }
    return declared;
  }
}
