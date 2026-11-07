package dev.gecode.iris.build;

import java.math.BigDecimal;
import org.jspecify.annotations.Nullable;

/**
 * Float literals as the Rust builder writes them: the shortest digits that read back, in decimal
 * from 1e-4 up to 1e16 and in exponent form outside, as Rust's {@code {:?}} does.
 */
final class FloatText {
  private FloatText() {}

  /** The text of the finite float {@code value}. */
  static String of(double value) {
    if (value == 0.0) {
      return Double.doubleToRawLongBits(value) < 0 ? "-0.0" : "0.0";
    }
    String sign = value < 0 ? "-" : "";
    String shortest = Double.toString(Math.abs(value));
    int marker = shortest.indexOf('E');
    String mantissa = marker < 0 ? shortest : shortest.substring(0, marker);
    int exponent = marker < 0 ? 0 : Integer.parseInt(shortest.substring(marker + 1));
    int point = mantissa.indexOf('.');
    String all = mantissa.substring(0, point) + mantissa.substring(point + 1);
    int leading = 0;
    while (all.charAt(leading) == '0') {
      leading++;
    }
    int end = all.length();
    while (all.charAt(end - 1) == '0') {
      end--;
    }
    String digits = all.substring(leading, end);
    // The position of the decimal point after the first significant digit, as a power of ten.
    int scientific = point + exponent - leading - 1;
    if (digits.length() == 2) {
      // Java prints at least two digits, so it may miss a single digit that reads back.
      String single = singleDigit(Math.abs(value), digits, scientific);
      if (single != null) {
        if (single.length() == 2) {
          scientific++;
        }
        digits = single.substring(0, 1);
      }
    }
    if (scientific < -4 || scientific >= 16) {
      String significand =
          digits.length() == 1 ? digits : digits.charAt(0) + "." + digits.substring(1);
      return sign + significand + "e" + scientific;
    }
    int decimalPoint = scientific + 1;
    if (decimalPoint <= 0) {
      return sign + "0." + "0".repeat(-decimalPoint) + digits;
    }
    if (decimalPoint >= digits.length()) {
      return sign + digits + "0".repeat(decimalPoint - digits.length()) + ".0";
    }
    return sign + digits.substring(0, decimalPoint) + "." + digits.substring(decimalPoint);
  }

  /**
   * The single digit nearest {@code value} that reads back as it, or null; "10" stands for a 1 at
   * the next power of ten.
   */
  private static @Nullable String singleDigit(double value, String digits, int scientific) {
    int low = digits.charAt(0) - '0';
    BigDecimal exact = new BigDecimal(value);
    String best = null;
    BigDecimal bestDistance = null;
    for (int candidate = low; candidate <= low + 1; candidate++) {
      BigDecimal text = BigDecimal.valueOf(candidate).scaleByPowerOfTen(scientific);
      if (candidate == 0 || text.doubleValue() != value) {
        continue;
      }
      BigDecimal distance = text.subtract(exact).abs();
      if (bestDistance == null || distance.compareTo(bestDistance) < 0) {
        best = Integer.toString(candidate);
        bestDistance = distance;
      }
    }
    return best;
  }

  /** The name Rust gives a float that is not finite. */
  static String nonFinite(double value) {
    if (Double.isNaN(value)) {
      return "NaN";
    }
    return value > 0 ? "inf" : "-inf";
  }
}
