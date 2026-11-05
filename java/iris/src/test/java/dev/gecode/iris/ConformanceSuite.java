package dev.gecode.iris;

import static java.nio.charset.StandardCharsets.UTF_8;

import java.io.IOException;
import java.io.InputStream;
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

/** Selects conformance cases and runs them through the Rust runner, with the Java driver. */
final class ConformanceSuite {
  private static final int BATCH_SIZE = 25;
  private static final int MAX_PARALLEL_BATCHES = 8;
  private static final String DEFERRED_TIMEOUT_SECONDS = "10";
  private static final String DRIVER_JVM_OPTIONS = "-Xmx1g";
  private static final List<String> ROOTS = List.of("conformance/cases/0.1", "formats");
  private static final Set<String> TOP_LEVEL_KEYS =
      Set.of("operation", "output", "title", "section", "source", "tags");
  private static final Pattern BARE_KEY = Pattern.compile("([A-Za-z0-9_-]+)\\s*=.*");
  private static final Pattern OPERATION =
      Pattern.compile("operation\\s*=\\s*\"(parse|read|catalog|dzn)\"\\s*(#.*)?");
  private static final Pattern DEFERRED_LINE = Pattern.compile("(\\S+) (§10\\.[2-5])");
  private static final Pattern OUTCOME = Pattern.compile("(PASS|FAIL) (.+)");

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
        System.getProperty("iris.driver"));
  }

  /** Runs every case of {@code operation}, and throws unless all of them pass. */
  void requireAllPass(String operation)
      throws IOException, InterruptedException, ExecutionException {
    requireOutcomes(operation, List.of());
  }

  /**
   * Runs every case of {@code operation}, and throws unless exactly the cases listed in the
   * resource {@code deferredResource} fail.
   */
  void requireOutcomes(String operation, String deferredResource)
      throws IOException, InterruptedException, ExecutionException {
    try (InputStream stream = ConformanceSuite.class.getResourceAsStream(deferredResource)) {
      if (stream == null) {
        throw new AssertionError("no resource " + deferredResource);
      }
      requireOutcomes(operation, new String(stream.readAllBytes(), UTF_8).lines().toList());
    }
  }

  private void requireOutcomes(String operation, List<String> deferredLines)
      throws IOException, InterruptedException, ExecutionException {
    Map<Path, String> operations = new LinkedHashMap<>();
    for (String root : ROOTS) {
      for (Path directory : discover(repository.resolve(root))) {
        operations.put(directory, operation(directory));
      }
    }
    Map<Path, String> deferred = deferred(deferredLines, repository, operations, operation);
    List<Path> selected =
        operations.keySet().stream().filter(c -> operations.get(c).equals(operation)).toList();
    System.out.printf(
        "Selected %d %s cases from %d discovered in %s%n",
        selected.size(), operation, operations.size(), String.join(" and ", ROOTS));
    if (selected.isEmpty()) {
      throw new AssertionError("no " + operation + " cases selected");
    }
    Map<Path, Boolean> passed = run(selected, deferred.keySet());
    List<String> problems = problems(passed, deferred.keySet());
    if (!problems.isEmpty()) {
      throw new AssertionError(String.join("\n", problems));
    }
    int passes = selected.size() - deferred.size();
    System.out.printf(
        "Summary: total=%d pass=%d deferred=%d fail=0%n", selected.size(), passes, deferred.size());
    if (!deferred.isEmpty()) {
      Map<String, Integer> perRule = new TreeMap<>();
      deferred.values().forEach(rule -> perRule.merge(rule, 1, Integer::sum));
      System.out.println("Deferred per rule: " + perRule);
    }
  }

  /**
   * Returns the rule of each case in the lines of a deferred list, one {@code <case path> §10.x}
   * each. The paths are relative to {@code repository}.
   *
   * @param operations the operation of every discovered case
   * @param operation the operation that the list is for
   */
  static Map<Path, String> deferred(
      List<String> lines, Path repository, Map<Path, String> operations, String operation) {
    Map<Path, String> deferred = new LinkedHashMap<>();
    for (String line : lines) {
      Matcher entry = DEFERRED_LINE.matcher(line);
      if (!entry.matches()) {
        throw new IllegalArgumentException("malformed deferred line: " + line);
      }
      Path directory = repository.resolve(entry.group(1));
      if (!operation.equals(operations.get(directory))) {
        throw new IllegalArgumentException("not a " + operation + " case: " + line);
      }
      if (deferred.put(directory, entry.group(2)) != null) {
        throw new IllegalArgumentException("repeated deferred case: " + line);
      }
    }
    return deferred;
  }

  /** Returns what is wrong with {@code passed}, given the cases that should still fail. */
  static List<String> problems(Map<Path, Boolean> passed, Set<Path> deferred) {
    List<String> problems = new ArrayList<>();
    passed.forEach((directory, pass) -> {
      if (pass && deferred.contains(directory)) {
        problems.add("passes but is on the deferred list: " + directory);
      } else if (!pass && !deferred.contains(directory)) {
        problems.add("fails but is not on the deferred list: " + directory);
      }
    });
    return problems;
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
        .forEach(line -> outcomes.put(Path.of(line.group(2)), line.group(1).equals("PASS")));
    if (!outcomes.keySet().equals(Set.copyOf(cases))) {
      throw new AssertionError("the runner did not report each case once:\n" + report);
    }
    return outcomes;
  }

  /** Runs {@code selected} in batches, keeping the cases of {@code deferred} in their own. */
  private Map<Path, Boolean> run(List<Path> selected, Set<Path> deferred)
      throws InterruptedException, ExecutionException {
    List<Callable<Map<Path, Boolean>>> batches = new ArrayList<>();
    addBatches(batches, selected.stream().filter(c -> !deferred.contains(c)).toList(), List.of());
    addBatches(
        batches,
        selected.stream().filter(deferred::contains).toList(),
        List.of("--timeout", DEFERRED_TIMEOUT_SECONDS));
    int threads = Math.min(Runtime.getRuntime().availableProcessors(), MAX_PARALLEL_BATCHES);
    Map<Path, Boolean> passed = new LinkedHashMap<>();
    try (ExecutorService pool = Executors.newFixedThreadPool(threads)) {
      for (Future<Map<Path, Boolean>> batch : pool.invokeAll(batches)) {
        passed.putAll(batch.get());
      }
    }
    return passed;
  }

  private void addBatches(
      List<Callable<Map<Path, Boolean>>> batches, List<Path> cases, List<String> options) {
    for (int start = 0; start < cases.size(); start += BATCH_SIZE) {
      List<Path> batch = cases.subList(start, Math.min(start + BATCH_SIZE, cases.size()));
      batches.add(() -> runBatch(batch, options));
    }
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

  private Map<Path, Boolean> runBatch(List<Path> cases, List<String> options)
      throws IOException, InterruptedException {
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
    command.addAll(options);
    cases.forEach(directory -> command.add(directory.toString()));
    var builder = new ProcessBuilder(command);
    builder.directory(repository.resolve("rust").toFile());
    builder.environment().put("JAVA_HOME", System.getProperty("java.home"));
    builder.environment().put("JAVA_OPTS", DRIVER_JVM_OPTIONS);
    builder.redirectErrorStream(true);
    Process process = builder.start();
    String report = new String(process.getInputStream().readAllBytes(), UTF_8);
    process.waitFor();
    return outcomes(report, cases);
  }
}
