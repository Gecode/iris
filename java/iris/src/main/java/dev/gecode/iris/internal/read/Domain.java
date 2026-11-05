package dev.gecode.iris.internal.read;

import dev.gecode.iris.internal.syntax.Ast.ReadingKind;
import org.jspecify.annotations.Nullable;

/**
 * A reading type with its range evaluated.
 *
 * @param kind what the type reads
 * @param typeName the name of the type, or null if it is not a named type
 * @param bounds the range
 */
record Domain(ReadingKind kind, @Nullable String typeName, Bounds bounds) {
  /**
   * The evaluated bounds of a range, both included.
   *
   * @param hasLower whether there is a lower bound
   * @param lower the lower bound, or the smallest integer
   * @param hasUpper whether there is an upper bound
   * @param upper the upper bound, or the largest integer
   */
  record Bounds(boolean hasLower, long lower, boolean hasUpper, long upper) {
    /** The bounds of a type without a range. */
    static final Bounds NONE = new Bounds(false, Long.MIN_VALUE, false, Long.MAX_VALUE);

    boolean contains(long value) {
      return value >= lower && value <= upper;
    }

    private boolean isBounded() {
      return hasLower || hasUpper;
    }
  }

  /** Tests whether the integer {@code value} is in the range. */
  boolean contains(long value) {
    return bounds.contains(value);
  }

  /** Tests whether {@code value} is in the range, which only concerns integers. */
  boolean fits(Stored value) {
    return !(value instanceof Stored.Int number) || contains(number.value());
  }

  /** Returns a description of the values, for messages. */
  String describe() {
    String description = bounds.isBounded() ? describeRange() : describeKind();
    return typeName == null ? description : typeName + " (" + description + ")";
  }

  private String describeRange() {
    return "an int in "
        + (bounds.hasLower() ? Long.toString(bounds.lower()) : "")
        + ".."
        + (bounds.hasUpper() ? Long.toString(bounds.upper()) : "");
  }

  private String describeKind() {
    return switch (kind) {
      case INT -> "int";
      case FLOAT -> "float";
      case NUM -> "a number";
      case WORD -> "a word";
      case REST -> "the rest of the line";
      case SYMBOL -> "a symbol";
    };
  }
}
