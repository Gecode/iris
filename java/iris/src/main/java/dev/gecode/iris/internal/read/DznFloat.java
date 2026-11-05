package dev.gecode.iris.internal.read;

import java.math.BigDecimal;
import java.math.MathContext;
import java.math.RoundingMode;

/**
 * The dzn spelling of a float (§B.3): the digits of the first precision, from the digits of its
 * integer part, that read back as the same float.
 */
final class DznFloat {
  private static final int MAX_PRECISION = 17;

  /** The floats from here on start the search at one digit, like those below 1. */
  private static final double FIRST_WITHOUT_INTEGER_DIGITS = 1e17;

  private static final MathContext[] ROUNDINGS = new MathContext[MAX_PRECISION + 1];

  static {
    for (int precision = 1; precision <= MAX_PRECISION; precision++) {
      ROUNDINGS[precision] = new MathContext(precision, RoundingMode.HALF_EVEN);
    }
  }

  private DznFloat() {}

  /** Returns the spelling of the finite float {@code value}. */
  static String of(double value) {
    if (value == 0.0) {
      return Double.doubleToRawLongBits(value) < 0 ? "-0.0" : "0.0";
    }
    double magnitude = Math.abs(value);
    BigDecimal exact = new BigDecimal(magnitude);
    int precision = firstPrecision(magnitude);
    BigDecimal rounded = exact.round(ROUNDINGS[precision]);
    while (precision < MAX_PRECISION && rounded.doubleValue() != magnitude) {
      precision++;
      rounded = exact.round(ROUNDINGS[precision]);
    }
    String spelling = spell(rounded, precision);
    return value < 0 ? "-" + spelling : spelling;
  }

  /**
   * Returns where the search for {@code magnitude} can start, as trying every precision from the
   * digits of the integer part costs far more. No decimal shorter than the shortest one reads back,
   * so starting there skips no success. {@link Double#toString} keeps two digits even where one
   * would do, so that a result of two digits or fewer says nothing.
   */
  private static int firstPrecision(double magnitude) {
    int integerDigits =
        magnitude >= 1 && magnitude < FIRST_WITHOUT_INTEGER_DIGITS
            ? Long.toString((long) magnitude).length()
            : 1;
    int shortest = CanonicalFloat.shortest(magnitude).digits().length();
    return shortest > 2 ? Math.max(integerDigits, shortest) : integerDigits;
  }

  /** Spells {@code rounded}, the decimal that the search selected at {@code precision}. */
  private static String spell(BigDecimal rounded, int precision) {
    int exponent = rounded.precision() - rounded.scale() - 1;
    String digits = rounded.stripTrailingZeros().unscaledValue().toString();
    if (exponent < -4 || exponent >= precision) {
      return scientific(digits, exponent);
    }
    String fixed = CanonicalFloat.fixed(digits, exponent + 1);
    return fixed.indexOf('.') < 0 ? fixed + ".0" : fixed;
  }

  private static String scientific(String digits, int exponent) {
    String significand =
        digits.length() == 1 ? digits : digits.charAt(0) + "." + digits.substring(1);
    int size = Math.abs(exponent);
    return significand + (exponent < 0 ? "e-" : "e+") + (size < 10 ? "0" : "") + size;
  }
}
