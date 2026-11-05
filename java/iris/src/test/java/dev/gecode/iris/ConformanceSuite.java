package dev.gecode.iris;

import static java.nio.charset.StandardCharsets.UTF_8;

import java.io.File;
import java.io.IOException;
import java.nio.file.FileVisitResult;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.SimpleFileVisitor;
import java.nio.file.attribute.BasicFileAttributes;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.concurrent.Callable;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.stream.Collectors;

/** Selects conformance cases and runs them through the Rust runner, with the Java driver. */
final class ConformanceSuite {
  private static final int BATCH_SIZE = 25;
  private static final int MAX_PARALLEL_BATCHES = 8;
  private static final String DRIVER_MAIN_CLASS = "dev.gecode.iris.driver.Main";
  // The launcher's options from iris-driver/build.gradle.kts, and a heap for the largest cases.
  private static final List<String> DRIVER_JVM_OPTIONS =
      List.of("-XX:TieredStopAtLevel=1", "-XX:+UseSerialGC", "-XX:-UsePerfData", "-Xmx1g");
  private static final List<String> ROOTS = List.of("conformance/cases/0.1", "formats");
  private static final List<String> OPERATIONS = List.of("parse", "read", "catalog", "dzn");
  private static final Set<String> TOP_LEVEL_KEYS =
      Set.of("operation", "output", "title", "section", "source", "tags");
  private static final Pattern BARE_KEY = Pattern.compile("([A-Za-z0-9_-]+)\\s*=.*");
  private static final Pattern OPERATION =
      Pattern.compile("operation\\s*=\\s*\"(parse|read|catalog|dzn)\"\\s*(#.*)?");
  private static final Pattern OUTCOME = Pattern.compile("(PASS|FAIL) (.+)");
  private static final String VERBATIM_PREFIX = "//?/";

  private final Path repository;
  private final String driver;

  ConformanceSuite(Path repository, String driver) {
    this.repository = repository;
    this.driver = driver;
  }

  /** Returns the suite for the driver and repository that Gradle passes to the tests. */
  static ConformanceSuite fromBuild() throws IOException {
    // The runner reports a case by its real path.
    return new ConformanceSuite(
        Path.of(System.getProperty("iris.repository")).toRealPath(),
        driverCommand(
            Path.of(System.getProperty("java.home"), "bin", "java"),
            Path.of(System.getProperty("iris.driverHome"))));
  }

  /**
   * Returns the runner's command for the driver installed in {@code driverHome}.
   *
   * <p>It starts {@code java} rather than the installed launcher, as Rust refuses to pass the
   * newline that ends a {@code params.json} to the launcher's batch file on Windows. Each word is
   * in single quotes: on Windows {@code ProcessBuilder} would drop double quotes around the
   * command, and the runner would then read the backslashes of a path as escapes.
   */
  static String driverCommand(Path java, Path driverHome) {
    List<String> words = new ArrayList<>();
    words.add(java.toString());
    words.addAll(DRIVER_JVM_OPTIONS);
    words.addAll(List.of("-cp", driverHome.resolve("lib") + File.separator + "*"));
    words.add(DRIVER_MAIN_CLASS);
    return words.stream().map(ConformanceSuite::quoted).collect(Collectors.joining(" "));
  }

  private static String quoted(String word) {
    return "'" + word.replace("'", "'\\''") + "'";
  }

  /**
   * Runs every case of every operation, and throws unless all of them pass. An operation without
   * cases is an error, as the selection must have gone wrong.
   */
  void requireAllPass() throws IOException, InterruptedException, ExecutionException {
    Map<Path, String> operations = new LinkedHashMap<>();
    for (String root : ROOTS) {
      for (Path directory : discover(repository.resolve(root))) {
        operations.put(directory, operation(directory));
      }
    }
    Map<String, Integer> counts = new TreeMap<>();
    operations.values().forEach(operation -> counts.merge(operation, 1, Integer::sum));
    for (String operation : OPERATIONS) {
      if (!counts.containsKey(operation)) {
        throw new AssertionError("no " + operation + " cases selected");
      }
    }
    System.out.printf("Selected %s from %s%n", counts, String.join(" and ", ROOTS));
    List<Path> selected = List.copyOf(operations.keySet());
    Map<Path, Boolean> passed = run(selected);
    List<String> failures =
        selected.stream()
            .filter(c -> !passed.getOrDefault(c, false))
            .map(c -> "fails: " + c)
            .toList();
    if (!failures.isEmpty()) {
      throw new AssertionError(String.join("\n", failures));
    }
    System.out.printf("Summary: total=%d pass=%d fail=0%n", selected.size(), selected.size());
  }

  /**
   * Returns whether each case passed, from the runner's report on {@code cases}. A report that
   * leaves out a case, or that says something else about one, is an error.
   */
  static Map<Path, Boolean> outcomes(String report, List<Path> cases) {
    Map<Path, Boolean> outcomes = new LinkedHashMap<>();
    report
        .lines()
        .map(OUTCOME::matcher)
        .filter(Matcher::matches)
        .forEach(line ->
            outcomes.put(
                Path.of(withNativeSeparators(line.group(2))), line.group(1).equals("PASS")));
    if (!outcomes.keySet().equals(Set.copyOf(cases))) {
      throw new AssertionError("the runner did not report each case once:\n" + report);
    }
    return outcomes;
  }

  /**
   * Returns a reported path that {@code Path} can read. On Windows the runner reports verbatim
   * paths with forward slashes, and {@code Path} knows that prefix, for drives and UNC shares
   * alike, only with backslashes.
   */
  static String withNativeSeparators(String reported) {
    return reported.startsWith(VERBATIM_PREFIX) ? reported.replace('/', '\\') : reported;
  }

  /** Runs {@code selected} in batches. */
  private Map<Path, Boolean> run(List<Path> selected)
      throws InterruptedException, ExecutionException {
    List<Callable<Map<Path, Boolean>>> batches = new ArrayList<>();
    for (int start = 0; start < selected.size(); start += BATCH_SIZE) {
      List<Path> batch = selected.subList(start, Math.min(start + BATCH_SIZE, selected.size()));
      batches.add(() -> runBatch(batch));
    }
    int threads = Math.min(Runtime.getRuntime().availableProcessors(), MAX_PARALLEL_BATCHES);
    Map<Path, Boolean> passed = new LinkedHashMap<>();
    try (ExecutorService pool = Executors.newFixedThreadPool(threads)) {
      for (Future<Map<Path, Boolean>> batch : pool.invokeAll(batches)) {
        passed.putAll(batch.get());
      }
    }
    return passed;
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

  private Map<Path, Boolean> runBatch(List<Path> cases) throws IOException, InterruptedException {
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
                driver,
                "--version",
                "0.1"));
    cases.forEach(directory -> command.add(directory.toString()));
    var builder = new ProcessBuilder(command);
    builder.directory(repository.resolve("rust").toFile());
    builder.redirectErrorStream(true);
    Process process = builder.start();
    String report = new String(process.getInputStream().readAllBytes(), UTF_8);
    process.waitFor();
    return outcomes(report, cases);
  }
}
