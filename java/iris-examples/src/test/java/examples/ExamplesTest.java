package examples;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.File;
import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;
import java.util.concurrent.TimeUnit;
import java.util.regex.MatchResult;
import java.util.regex.Pattern;
import java.util.stream.Collectors;
import java.util.stream.Stream;
import org.junit.jupiter.api.DynamicTest;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestFactory;
import org.junit.jupiter.api.io.TempDir;

/**
 * Runs each example of the guide from the repository root, as the guide says to, in a JVM of its
 * own. It must exit with status 0, print nothing on standard error, and print its recording. The
 * guide must show exactly these examples, and no code that comes from anywhere else.
 */
class ExamplesTest {
  private static final Path REPOSITORY = Path.of(System.getProperty("iris.repository", ""));
  private static final Path SOURCES = Path.of(System.getProperty("iris.examples", ""));
  private static final String CLASS_PATH = System.getProperty("iris.examplesClasspath", "");
  private static final Path GUIDE = REPOSITORY.resolve("site/src/content/docs/using/java.mdx");

  /** An example from a source file; the guide's setup snippets have lowercase names. */
  private static final Pattern SHOWN_EXAMPLE =
      Pattern.compile("<CheckedExample id=\"((?:java|kotlin)-[A-Z]\\w*)\"");

  private static final Pattern CODE_FENCE = Pattern.compile("^\\s*(?:```|~~~)", Pattern.MULTILINE);

  @TempDir Path scratch;

  private record Example(String language, Path source, String mainClass) {
    Path recording() {
      String file = source.getFileName().toString();
      return source.resolveSibling(stem(file) + ".out");
    }

    /** Returns the name that the guide gives this example, such as {@code java-Read}. */
    String guideId() {
      return language + "-" + stem(source.getFileName().toString());
    }

    private static String stem(String file) {
      return file.substring(0, file.lastIndexOf('.'));
    }
  }

  @TestFactory
  Stream<DynamicTest> everyExamplePrintsItsRecording() throws IOException {
    return examples().stream()
        .map(example -> DynamicTest.dynamicTest(example.mainClass(), () -> check(example)));
  }

  @Test
  void theGuideShowsEveryExample() throws IOException {
    Set<String> shown =
        SHOWN_EXAMPLE
            .matcher(read(GUIDE))
            .results()
            .map(match -> match.group(1))
            .collect(Collectors.toSet());
    assertEquals(examples().stream().map(Example::guideId).collect(Collectors.toSet()), shown);
  }

  @Test
  void theGuideHasNoCodeThatNothingChecks() {
    List<String> fences =
        CODE_FENCE.matcher(read(GUIDE)).results().map(MatchResult::group).toList();
    assertEquals(List.of(), fences);
  }

  private static List<Example> examples() throws IOException {
    List<Example> examples = new ArrayList<>();
    examples.addAll(examplesIn("java", ".java", ""));
    examples.addAll(examplesIn("kotlin", ".kt", "Kt"));
    return examples;
  }

  private static List<Example> examplesIn(String language, String extension, String classSuffix)
      throws IOException {
    Path directory = SOURCES.resolve(language).resolve("examples");
    try (Stream<Path> files = Files.list(directory)) {
      List<Example> examples =
          files
              .filter(file -> file.toString().endsWith(extension))
              .filter(file -> !file.endsWith("package-info.java"))
              .sorted()
              .map(file -> new Example(language, file, mainClass(file, extension, classSuffix)))
              .toList();
      assertFalse(examples.isEmpty(), "no examples in " + directory);
      return examples;
    }
  }

  private static String mainClass(Path source, String extension, String classSuffix) {
    String file = source.getFileName().toString();
    return "examples." + file.substring(0, file.length() - extension.length()) + classSuffix;
  }

  private void check(Example example) throws IOException, InterruptedException {
    Path output = scratch.resolve(example.mainClass() + ".out");
    Path errors = scratch.resolve(example.mainClass() + ".err");
    String java = Path.of(System.getProperty("java.home"), "bin", "java").toString();
    Process process =
        new ProcessBuilder(java, "-cp", CLASS_PATH, example.mainClass())
            .directory(REPOSITORY.toFile())
            .redirectInput(new File(devNull()))
            .redirectOutput(output.toFile())
            .redirectError(errors.toFile())
            .start();
    assertTrue(process.waitFor(1, TimeUnit.MINUTES), example.mainClass() + " did not finish");
    assertEquals(0, process.exitValue(), () -> example.mainClass() + ": " + read(errors));
    assertEquals("", read(errors));
    assertEquals(read(example.recording()), read(output));
  }

  private static String devNull() {
    return System.getProperty("os.name").startsWith("Windows") ? "NUL" : "/dev/null";
  }

  /** Reads a file. Windows ends the lines of standard output with CRLF. */
  private static String read(Path file) {
    try {
      return Files.readString(file).replace("\r\n", "\n");
    } catch (IOException e) {
      throw new UncheckedIOException(e);
    }
  }
}
