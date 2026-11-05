package dev.gecode.iris.internal.read;

/**
 * The canonical spelling of a float (§3.3): its shortest digits, in fixed notation unless
 * scientific notation is shorter.
 *
 * <p>The digits come from {@link Double#toString}, which gives the shortest that read back as the
 * same float, and the closest of those, except that it keeps two digits for the subnormals {@code
 * 4.9E-324} and {@code 1.0E-323}. Floats read from an input are never subnormal (§4.5).
 */
final class CanonicalFloat {
  private CanonicalFloat() {}

  /**
   * A positive decimal.
   *
   * @param digits its significant digits, without leading or trailing zeros
   * @param decimalPoint how many of them stand before the decimal point, which can be any number
   */
  record Decimal(String digits, int decimalPoint) {}

  /** Returns the spelling of the finite float {@code value}. */
  static String of(double value) {
    if (value == 0.0) {
      return Double.doubleToRawLongBits(value) < 0 ? "-0.0" : "0";
    }
    Decimal decimal = shortest(Math.abs(value));
    String fixed = fixed(decimal.digits(), decimal.decimalPoint());
    String scientific = scientific(decimal.digits(), decimal.decimalPoint());
    String spelling = fixed.length() <= scientific.length() ? fixed : scientific;
    return value < 0 ? "-" + spelling : spelling;
  }

  /** Returns the digits of {@link Double#toString} for {@code magnitude}, which is positive. */
  static Decimal shortest(double magnitude) {
    String shortest = Double.toString(magnitude);
    int marker = shortest.indexOf('E');
    String mantissa = marker < 0 ? shortest : shortest.substring(0, marker);
    int exponent = marker < 0 ? 0 : Integer.parseInt(shortest.substring(marker + 1));
    int point = mantissa.indexOf('.');
    String allDigits = mantissa.substring(0, point) + mantissa.substring(point + 1);
    int leading = 0;
    while (allDigits.charAt(leading) == '0') {
      leading++;
    }
    int end = allDigits.length();
    while (allDigits.charAt(end - 1) == '0') {
      end--;
    }
    return new Decimal(allDigits.substring(leading, end), point + exponent - leading);
  }

  /** Returns {@code digits} in fixed notation, with no decimal point after a whole number. */
  static String fixed(String digits, int decimalPoint) {
    if (decimalPoint <= 0) {
      return "0." + "0".repeat(-decimalPoint) + digits;
    }
    if (decimalPoint >= digits.length()) {
      return digits + "0".repeat(decimalPoint - digits.length());
    }
    return digits.substring(0, decimalPoint) + "." + digits.substring(decimalPoint);
  }

  private static String scientific(String digits, int decimalPoint) {
    String significand =
        digits.length() == 1 ? digits : digits.charAt(0) + "." + digits.substring(1);
    return significand + "e" + (decimalPoint - 1);
  }
}
