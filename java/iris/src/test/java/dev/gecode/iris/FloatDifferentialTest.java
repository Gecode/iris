package dev.gecode.iris;

import static java.nio.charset.StandardCharsets.UTF_8;
import static org.junit.jupiter.api.Assertions.assertEquals;

import java.io.IOException;
import java.math.BigDecimal;
import java.math.MathContext;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.SplittableRandom;
import java.util.regex.Pattern;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/**
 * Compares how Java and the Rust reference read and print floats, byte for byte.
 *
 * <p>{@code -PirisFloatValues=N} sets how many floats to compare. {@code -PirisCli=PATH} names an
 * {@code iris} binary, such as a release build, to run instead of {@code cargo run}.
 */
class FloatDifferentialTest {
  private static final long SEED = 0x1715_0001L;
  private static final int DEFAULT_VALUES = 1_000_000;
  private static final int CHUNK = 250_000;
  private static final MathContext SEVENTEEN_DIGITS = new MathContext(17);
  private static final Pattern NUMBER_END = Pattern.compile("[,\\]]");
  private static final String SPEC = "repeat until eof {\nx: float\n}\n";

  @TempDir Path directory;

  @Test
  void javaAndRustPrintTheSameFloats() throws IOException, InterruptedException {
    int values = Integer.getInteger("iris.floatValues", DEFAULT_VALUES);
    Path specFile = Files.writeString(directory.resolve("floats.iris"), SPEC);
    Path inputFile = directory.resolve("floats.txt");
    Spec spec = Spec.parse(SPEC);
    var random = new SplittableRandom(SEED);
    int compared = 0;
    while (compared < values) {
      double[] expected = new double[Math.min(CHUNK, values - compared)];
      var input = new StringBuilder();
      for (int k = 0; k < expected.length; k++) {
        expected[k] = next(random, compared + k);
        input.append(token(expected[k])).append('\n');
      }
      byte[] bytes = input.toString().getBytes(UTF_8);
      Files.write(inputFile, bytes);
      Data data = spec.read(bytes);
      String document = data.toJson();
      assertEquals(rust(specFile, inputFile, "--json"), document, "the documents differ");
      requireRoundTrip(document, expected);
      assertEquals(rust(specFile, inputFile, "--dzn"), data.toDzn(), "the dzn texts differ");
      compared += expected.length;
    }
    System.out.printf("Compared %d floats, with no differences%n", compared);
  }

  /** Returns float number {@code index}, which is normal and not zero. */
  private static double next(SplittableRandom random, int index) {
    int ulps = random.nextInt(-3, 4);
    double value =
        switch (index % 11) {
          case 0, 1 -> anyBits(random);
          case 2 -> near(Math.scalb(1.0, random.nextInt(-1021, 1024)), ulps);
          case 3 -> near(Double.parseDouble("1e" + random.nextInt(-307, 309)), ulps);
          case 4 -> near(Double.MIN_NORMAL, Math.abs(ulps));
          case 5 -> near(Double.MAX_VALUE, -Math.abs(ulps));
          case 6 -> (double) ((1L << 53) + random.nextInt(-1024, 1025));
          case 7 -> random.nextDouble(1e21, 1e23);
          case 8 -> (double) random.nextLong(1, 100_000_000_000_000_000L);
          case 9 -> near(1e17, -random.nextInt(1, 26));
          default -> random.nextLong(1L << 50) + 0.5;
        };
    return random.nextBoolean() ? value : -value;
  }

  private static double anyBits(SplittableRandom random) {
    long exponent = random.nextLong(1, 2047);
    return Double.longBitsToDouble(exponent << 52 | random.nextLong(1L << 52));
  }

  private static double near(double value, int ulps) {
    return Double.longBitsToDouble(Double.doubleToLongBits(value) + ulps);
  }

  /** Returns seventeen significant digits of {@code value}, which identify it exactly. */
  private static String token(double value) {
    return new BigDecimal(value).round(SEVENTEEN_DIGITS).toString();
  }

  private static void requireRoundTrip(String document, double[] expected) {
    int start = document.indexOf('[', document.indexOf("\"value\"")) + 1;
    String[] printed = NUMBER_END.split(document.substring(start), -1);
    for (int k = 0; k < expected.length; k++) {
      assertEquals(expected[k], Double.parseDouble(printed[k]), printed[k]);
    }
  }

  private static String rust(Path spec, Path input, String format)
      throws IOException, InterruptedException {
    List<String> command = new ArrayList<>();
    String cli = System.getProperty("iris.cli");
    if (cli == null) {
      command.addAll(List.of("cargo", "run", "--quiet", "--locked", "-p", "iris-cli", "--bin"));
      command.addAll(List.of("iris", "--"));
    } else {
      command.add(cli);
    }
    command.addAll(List.of("read", spec.toString(), input.toString(), format));
    var builder = new ProcessBuilder(command);
    builder.directory(Path.of(System.getProperty("iris.repository"), "rust").toFile());
    builder.redirectError(ProcessBuilder.Redirect.INHERIT);
    Process process = builder.start();
    String document = new String(process.getInputStream().readAllBytes(), UTF_8);
    assertEquals(0, process.waitFor(), "the Rust reader failed");
    return document;
  }
}
