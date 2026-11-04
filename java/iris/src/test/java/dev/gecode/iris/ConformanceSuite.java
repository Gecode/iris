package dev.gecode.iris;

import static java.nio.charset.StandardCharsets.UTF_8;

import java.io.IOException;
import java.nio.file.FileVisitResult;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.SimpleFileVisitor;
import java.nio.file.attribute.BasicFileAttributes;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/** Selects conformance cases and runs them through the Rust runner, with the Java driver. */
final class ConformanceSuite {
  private static final int BATCH_SIZE = 100;
  private static final List<String> ROOTS = List.of("conformance/cases/0.1", "formats");
  private static final Set<String> TOP_LEVEL_KEYS =
      Set.of("operation", "output", "title", "section", "source", "tags");
  private static final Pattern BARE_KEY = Pattern.compile("([A-Za-z0-9_-]+)\\s*=.*");
  private static final Pattern OPERATION =
      Pattern.compile("operation\\s*=\\s*\"(parse|read|catalog|dzn)\"\\s*(#.*)?");
  private static final Pattern SUMMARY = Pattern.compile("Summary: total=(\\d+) pass=(\\d+) .*");

  private final Path repository;
  private final String driver;

  ConformanceSuite(Path repository, String driver) {
    this.repository = repository;
    this.driver = driver;
  }

  /** Returns the suite for the driver and repository that Gradle passes to the tests. */
  static ConformanceSuite fromBuild() {
    return new ConformanceSuite(
        Path.of(System.getProperty("iris.repository")), System.getProperty("iris.driver"));
  }

  /** Runs every case of {@code operation}, and throws unless all of them pass. */
  void requireAllPass(String operation) throws IOException, InterruptedException {
    List<Path> cases = new ArrayList<>();
    for (String root : ROOTS) {
      cases.addAll(discover(repository.resolve(root)));
    }
    List<Path> selected = new ArrayList<>();
    for (Path directory : cases) {
      if (operation(directory).equals(operation)) {
        selected.add(directory);
      }
    }
    System.out.printf(
        "Selected %d %s cases from %d discovered in %s%n",
        selected.size(), operation, cases.size(), String.join(" and ", ROOTS));
    if (selected.isEmpty()) {
      throw new AssertionError("no " + operation + " cases selected");
    }
    for (int start = 0; start < selected.size(); start += BATCH_SIZE) {
      runBatch(selected.subList(start, Math.min(start + BATCH_SIZE, selected.size())));
    }
    System.out.printf("Summary: total=%d pass=%d fail=0%n", selected.size(), selected.size());
  }

  /** Returns the case directories under {@code root}, in order, without following links. */
  static List<Path> discover(Path root) throws IOException {
    List<Path> cases = new ArrayList<>();
    Files.walkFileTree(
        root,
        new SimpleFileVisitor<>() {
          @Override
          public FileVisitResult preVisitDirectory(Path directory, BasicFileAttributes attributes) {
            if (isCase(directory)) {
              cases.add(directory);
              return FileVisitResult.SKIP_SUBTREE;
            }
            return FileVisitResult.CONTINUE;
          }
        });
    return cases.stream().sorted().toList();
  }

  private static boolean isCase(Path directory) {
    return Files.exists(directory.resolve("spec.iris"))
        || Files.exists(directory.resolve("spec.bin"))
        || Files.exists(directory.resolve("case.toml"));
  }

  private static String operation(Path directory) throws IOException {
    Path metadata = directory.resolve("case.toml");
    return Files.exists(metadata) ? operation(Files.readString(metadata)) : "read";
  }

  /**
   * Returns the operation in the text of a {@code case.toml}, by default {@code read}.
   *
   * <p>This reads only the subset of TOML that the cases use before their first table, and rejects
   * anything else, so that no case can silently leave the selection.
   */
  static String operation(String metadata) {
    String operation = null;
    for (String text : metadata.split("\n", -1)) {
      String line = text.strip();
      if (line.startsWith("[")) {
        break;
      }
      if (line.isEmpty() || line.startsWith("#")) {
        continue;
      }
      Matcher key = BARE_KEY.matcher(line);
      if (!key.matches() || !TOP_LEVEL_KEYS.contains(key.group(1))) {
        throw new IllegalArgumentException("unexpected metadata line: " + line);
      }
      if (key.group(1).equals("operation")) {
        Matcher value = OPERATION.matcher(line);
        if (operation != null || !value.matches()) {
          throw new IllegalArgumentException("bad or repeated operation: " + line);
        }
        operation = value.group(1);
      }
    }
    return operation == null ? "read" : operation;
  }

  private void runBatch(List<Path> cases) throws IOException, InterruptedException {
    List<String> command =
        new ArrayList<>(
            List.of(
                "cargo",
                "run",
                "--quiet",
                "--locked",
                "-p",
                "iris-conformance",
                "--",
                "run",
                "--driver",
                "\"" + driver + "\"",
                "--version",
                "0.1"));
    cases.forEach(directory -> command.add(directory.toString()));
    var builder = new ProcessBuilder(command);
    builder.directory(repository.resolve("rust").toFile());
    builder.environment().put("JAVA_HOME", System.getProperty("java.home"));
    builder.redirectErrorStream(true);
    Process process = builder.start();
    String output = new String(process.getInputStream().readAllBytes(), UTF_8);
    int status = process.waitFor();
    output.lines().filter(line -> !line.startsWith("PASS")).forEach(System.out::println);
    if (status != 0 || !passedAll(output, cases.size())) {
      throw new AssertionError("the conformance runner failed with status " + status);
    }
  }

  private static boolean passedAll(String output, int expected) {
    return output
        .lines()
        .map(SUMMARY::matcher)
        .filter(Matcher::matches)
        .anyMatch(summary ->
            Integer.parseInt(summary.group(1)) == expected
                && Integer.parseInt(summary.group(2)) == expected);
  }
}
