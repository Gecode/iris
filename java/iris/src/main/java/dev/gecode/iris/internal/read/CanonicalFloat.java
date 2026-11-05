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

  /** Returns the spelling of the finite float {@code value}. */
  static String of(double value) {
    if (value == 0.0) {
      return Double.doubleToRawLongBits(value) < 0 ? "-0.0" : "0";
    }
    String shortest = Double.toString(Math.abs(value));
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
    String digits = allDigits.substring(leading, end);
    int decimalPoint = point + exponent - leading;
    String fixed = fixed(digits, decimalPoint);
    String scientific = scientific(digits, decimalPoint);
    String spelling = fixed.length() <= scientific.length() ? fixed : scientific;
    return value < 0 ? "-" + spelling : spelling;
  }

  private static String fixed(String digits, int decimalPoint) {
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
