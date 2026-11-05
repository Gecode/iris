package dev.gecode.iris.internal.syntax;

import static java.nio.charset.StandardCharsets.US_ASCII;

import java.util.OptionalDouble;
import java.util.OptionalLong;

/** The numeric tokens of §4.4 and §4.5, shared by reader specs and input. */
public final class Numbers {
  private Numbers() {}

  /** Tests whether {@code c} is an ASCII digit. */
  public static boolean isDigit(int c) {
    return c >= '0' && c <= '9';
  }

  /** Tests whether {@code c} is a sign. */
  public static boolean isSign(int c) {
    return c == '+' || c == '-';
  }

  /** Tests whether {@code c} starts the exponent of a float. */
  public static boolean isExponentMarker(int c) {
    return c == 'e' || c == 'E';
  }

  /**
   * Returns the integer in the bytes of {@code text} from {@code from} up to {@code to}, or nothing
   * if they are not one or it is out of range.
   */
  public static OptionalLong parseInteger(byte[] text, int from, int to) {
    int position = from;
    boolean isNegative = position < to && text[position] == '-';
    if (position < to && isSign(text[position])) {
      position++;
    }
    if (position == to) {
      return OptionalLong.empty();
    }
    // Accumulated negatively, as the negative range is the larger one.
    long limit = isNegative ? Long.MIN_VALUE : -Long.MAX_VALUE;
    long value = 0;
    for (; position < to; position++) {
      int digit = text[position] - '0';
      if (digit < 0 || digit > 9 || value < (limit + digit) / 10) {
        return OptionalLong.empty();
      }
      value = value * 10 - digit;
    }
    return OptionalLong.of(isNegative ? value : -value);
  }

  /**
   * Returns the float in the bytes of {@code text} from {@code from} up to {@code to}, or nothing
   * if they are not one, if it overflows, or if a nonzero value rounds to a subnormal or to zero
   * (§4.5).
   */
  public static OptionalDouble parseFloat(byte[] text, int from, int to) {
    // Double.parseDouble takes more than §4.5: whitespace, hexadecimal, suffixes, NaN and Infinity.
    if (!isFloat(text, from, to)) {
      return OptionalDouble.empty();
    }
    double value = Double.parseDouble(new String(text, from, to - from, US_ASCII));
    boolean underflows = Math.abs(value) < Double.MIN_NORMAL && !hasZeroSignificand(text, from, to);
    return Double.isInfinite(value) || underflows
        ? OptionalDouble.empty()
        : OptionalDouble.of(value);
  }

  private static boolean isFloat(byte[] text, int from, int to) {
    int position = from < to && isSign(text[from]) ? from + 1 : from;
    int digits = digitsEnd(text, position, to);
    int significandDigits = digits - position;
    position = digits;
    if (position < to && text[position] == '.') {
      digits = digitsEnd(text, position + 1, to);
      significandDigits += digits - (position + 1);
      position = digits;
    }
    if (significandDigits == 0) {
      return false;
    }
    if (position == to) {
      return true;
    }
    if (!isExponentMarker(text[position])) {
      return false;
    }
    position++;
    if (position < to && isSign(text[position])) {
      position++;
    }
    return position < to && digitsEnd(text, position, to) == to;
  }

  private static int digitsEnd(byte[] text, int from, int to) {
    int position = from;
    while (position < to && isDigit(text[position])) {
      position++;
    }
    return position;
  }

  private static boolean hasZeroSignificand(byte[] text, int from, int to) {
    for (int position = from; position < to && !isExponentMarker(text[position]); position++) {
      if (text[position] >= '1' && text[position] <= '9') {
        return false;
      }
    }
    return true;
  }

  /**
   * Returns {@code base} to the power of {@code exponent}, which is not negative.
   *
   * @throws ArithmeticException if the result is out of range
   */
  public static long power(long base, long exponent) {
    long result = 1;
    long square = base;
    // The square overflows only if bits of the exponent remain, and then the result does too.
    for (long remaining = exponent; remaining > 0; ) {
      if ((remaining & 1) != 0) {
        result = Math.multiplyExact(result, square);
      }
      remaining >>= 1;
      if (remaining > 0) {
        square = Math.multiplyExact(square, square);
      }
    }
    return result;
  }
}
