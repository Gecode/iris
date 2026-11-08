package examples;

import static java.nio.charset.StandardCharsets.UTF_8;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.stream.Collectors;
import java.util.stream.Stream;
import org.junit.jupiter.api.DynamicTest;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestFactory;

/**
 * Runs every case of every format through the conformance runner with {@link FormatBuilderDriver},
 * once for the Java builders and once for the Kotlin ones, so that each builder reads what the
 * format's {@code .iris} file reads, errors included.
 */
class FormatBuildersTest {
  private static final Path REPOSITORY = Path.of(System.getProperty("iris.repository", ""));
  private static final Path SOURCES = Path.of(System.getProperty("iris.examples", ""));
  private static final String CLASS_PATH = System.getProperty("iris.testClasspath", "");

  private static final Pattern SUMMARY =
      Pattern.compile("^Summary: total=(\\d+) pass=(\\d+)", Pattern.MULTILINE);

  @TestFactory
  Stream<DynamicTest> everyCaseReadsWithEachBuilder() {
    return FormatBuilderDriver.LANGUAGES.stream()
        .flatMap(language ->
            FormatBuilderDriver.FORMATS.stream()
                .map(format ->
                    DynamicTest.dynamicTest(
                        language + " " + format, () -> check(language, format))));
  }

  @Test
  void everyFormatHasBothBuilders() throws IOException {
    Set<String> formats;
    try (Stream<Path> entries = Files.list(REPOSITORY.resolve("formats"))) {
      formats =
          entries
              .filter(Files::isDirectory)
              .map(directory -> String.valueOf(directory.getFileName()))
              .collect(Collectors.toSet());
    }
    assertEquals(Set.copyOf(FormatBuilderDriver.FORMATS), formats);
    for (String format : FormatBuilderDriver.FORMATS) {
      String name = "Build" + className(format);
      assertTrue(Files.isRegularFile(SOURCES.resolve("java/examples/" + name + ".java")), name);
      assertTrue(Files.isRegularFile(SOURCES.resolve("kotlin/examples/" + name + ".kt")), name);
    }
  }

  private static void check(String language, String format)
      throws IOException, InterruptedException {
    List<String> command =
        new ArrayList<>(
            List.of(
                "cargo",
                "run",
                "--quiet",
                "--locked",
                "-p",
                "gecode-iris-conformance",
                "--",
                "run",
                "--driver",
                driverCommand(language, format),
                "--version",
                "0.1",
                REPOSITORY.resolve("formats").resolve(format).toString()));
    Process process =
        new ProcessBuilder(command)
            .directory(REPOSITORY.resolve("rust").toFile())
            .redirectErrorStream(true)
            .start();
    String report = new String(process.getInputStream().readAllBytes(), UTF_8);
    int status = process.waitFor();
    Matcher summary = SUMMARY.matcher(report);
    assertTrue(summary.find(), () -> "no summary in the report:\n" + report);
    int total = Integer.parseInt(summary.group(1));
    int passed = Integer.parseInt(summary.group(2));
    System.out.printf("%s %s: %d of %d cases pass%n", language, format, passed, total);
    assertTrue(total >= 2, () -> format + " has too few cases:\n" + report);
    assertEquals(total, passed, report);
    assertEquals(0, status, report);
  }

  /**
   * Returns the runner's command for the driver, with each word in single quotes, as {@code
   * ConformanceSuite} quotes it: on Windows {@code ProcessBuilder} would drop double quotes, and
   * the runner would then read the backslashes of a path as escapes.
   */
  private static String driverCommand(String language, String format) {
    String java = Path.of(System.getProperty("java.home"), "bin", "java").toString();
    return Stream.of(java, "-cp", CLASS_PATH, "examples.FormatBuilderDriver", language, format)
        .map(word -> "'" + word.replace("'", "'\\''") + "'")
        .collect(Collectors.joining(" "));
  }

  /** {@return the class name part for {@code format}, such as {@code DimacsCnf}} */
  private static String className(String format) {
    StringBuilder name = new StringBuilder();
    for (String word : format.split("-", -1)) {
      name.append(Character.toUpperCase(word.charAt(0))).append(word.substring(1));
    }
    return name.toString();
  }
}
