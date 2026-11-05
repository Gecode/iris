package dev.gecode.iris.internal.read;

import static org.junit.jupiter.api.Assertions.assertEquals;

import java.math.BigDecimal;
import java.math.MathContext;
import java.math.RoundingMode;
import java.util.SplittableRandom;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.CsvSource;

class DznFloatTest {
  private static final long SEED = 0x1715_0002L;
  private static final BigDecimal ONE_E_17 = new BigDecimal("1e17");

  /**
   * Spells {@code value} by the letter of §B.3: the search tries every precision from p0, and reads
   * each decimal back through its text.
   */
  private static String literalSpelling(double value) {
    if (value == 0.0) {
      return Double.doubleToRawLongBits(value) < 0 ? "-0.0" : "0.0";
    }
    BigDecimal exact = new BigDecimal(Math.abs(value));
    boolean hasIntegerDigits =
        exact.compareTo(BigDecimal.ONE) >= 0 && exact.compareTo(ONE_E_17) < 0;
    int first = hasIntegerDigits ? exact.toBigInteger().toString().length() : 1;
    int precision = first;
    BigDecimal rounded = exact.round(new MathContext(precision, RoundingMode.HALF_EVEN));
    while (precision < 17 && Double.parseDouble(rounded.toString()) != Math.abs(value)) {
      precision++;
      rounded = exact.round(new MathContext(precision, RoundingMode.HALF_EVEN));
    }
    int exponent = rounded.precision() - rounded.scale() - 1;
    String digits = rounded.stripTrailingZeros().unscaledValue().toString();
    String text;
    if (exponent < -4 || exponent >= precision) {
      String fraction = digits.length() > 1 ? "." + digits.substring(1) : "";
      text = digits.charAt(0) + fraction + String.format("e%+03d", exponent);
    } else {
      text = rounded.stripTrailingZeros().toPlainString();
      text = text.indexOf('.') < 0 ? text + ".0" : text;
    }
    return value < 0 ? "-" + text : text;
  }

  private static void requireLiteral(double value) {
    assertEquals(literalSpelling(value), DznFloat.of(value), () -> Double.toString(value));
    assertEquals(literalSpelling(-value), DznFloat.of(-value), () -> Double.toString(-value));
  }

  @ParameterizedTest
  @CsvSource({
    "0.0, 0.0",
    "-0.0, -0.0",
    "20, 20.0",
    "1000, 1000.0",
    "0.1, 0.1",
    "1.2345678901234567, 1.2345678901234567",
    "0.0001, 0.0001",
    "0.00001, 1e-05",
    "1e16, 10000000000000000.0",
    "1e17, 1e+17",
    "1e20, 1e+20",
    "1e-7, 1e-07",
    "1e100, 1e+100",
    "1e-100, 1e-100",
    "1.7976931348623157e308, 1.7976931348623157e+308",
    "2.2250738585072014e-308, 2.2250738585072014e-308",
    "4.9e-324, 5e-324",
    "9007199254740992, 9007199254740992.0",
    "9999999999999998, 9999999999999998.0",
    "99999999999999584, 99999999999999584.0",
    "99999999999999600, 99999999999999600.0",
    "-99999999999999984, -99999999999999984.0",
  })
  void spellsTheFloatsOfTheSpecification(double value, String spelling) {
    assertEquals(spelling, DznFloat.of(value));
  }

  @Test
  void matchesTheLiteralSearchOnAnyBits() {
    var random = new SplittableRandom(SEED);
    for (int k = 0; k < 200_000; k++) {
      double value = Double.longBitsToDouble(random.nextLong());
      if (Double.isFinite(value)) {
        requireLiteral(value);
      }
    }
  }

  @Test
  void matchesTheLiteralSearchOnSubnormals() {
    var random = new SplittableRandom(SEED);
    for (long bits = 1; bits < 2000; bits++) {
      requireLiteral(Double.longBitsToDouble(bits));
    }
    for (int k = 0; k < 50_000; k++) {
      requireLiteral(Double.longBitsToDouble(random.nextLong(1L << 52)));
    }
  }

  @Test
  void matchesTheLiteralSearchOnPowersOfTwo() {
    for (int exponent = -1074; exponent < 1024; exponent++) {
      double power = Math.scalb(1.0, exponent);
      requireLiteral(power);
      requireLiteral(Math.nextUp(power));
      requireLiteral(Math.nextDown(power));
    }
  }

  @Test
  void matchesTheLiteralSearchNearPowersOfTen() {
    for (int exponent = -323; exponent <= 308; exponent++) {
      double power = Double.parseDouble("1e" + exponent);
      for (int ulps = -4; ulps <= 4; ulps++) {
        long bits = Double.doubleToLongBits(power) + ulps;
        if (bits > 0) {
          requireLiteral(Double.longBitsToDouble(bits));
        }
      }
    }
  }

  @Test
  void matchesTheLiteralSearchInTheGapBelow1e17() {
    double value = 99999999999999600.0;
    for (int k = 0; k < 25; k++) {
      assertEquals(
          new BigDecimal(value).toPlainString() + ".0", DznFloat.of(value), "at step " + k);
      requireLiteral(value);
      value = Math.nextUp(value);
    }
    assertEquals(1e17, value);
    requireLiteral(value);
  }

  @Test
  void matchesTheLiteralSearchOnExactTies() {
    // Each has one digit more than the integers around it can hold, so rounding meets a tie.
    for (double value : new double[] {0.5, 2.5, 0.125, 0.375, 1.5, 12.5, 1125899906842624.5}) {
      requireLiteral(value);
    }
    for (long integer = 1L << 53; integer < (1L << 53) + 2000; integer += 2) {
      requireLiteral((double) integer);
    }
    var random = new SplittableRandom(SEED);
    for (int k = 0; k < 50_000; k++) {
      long digits = random.nextLong(1L << 53);
      requireLiteral(Math.scalb((double) digits, random.nextInt(-6, 7)));
    }
  }

  @Test
  void matchesTheLiteralSearchOnIntegers() {
    var random = new SplittableRandom(SEED);
    for (int k = 0; k < 100_000; k++) {
      requireLiteral((double) random.nextLong(1, 100_000_000_000_000_000L));
    }
    for (long value = 1; value < 100_000_000_000_000_000L; value *= 10) {
      requireLiteral((double) value);
      requireLiteral((double) (value - 1));
      requireLiteral((double) (value + 1));
    }
  }
}
