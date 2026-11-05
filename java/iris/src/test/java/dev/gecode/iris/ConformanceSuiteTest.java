package dev.gecode.iris;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.ValueSource;

class ConformanceSuiteTest {
  private static final Path REPOSITORY = Path.of("/repository");
  private static final Path READ_CASE = REPOSITORY.resolve("cases/a");
  private static final Map<Path, String> OPERATIONS =
      Map.of(READ_CASE, "read", REPOSITORY.resolve("cases/b"), "parse");

  private static Map<Path, String> deferred(String... lines) {
    return ConformanceSuite.deferred(List.of(lines), REPOSITORY, OPERATIONS, "read");
  }

  @Test
  void readsTheOperation() {
    assertEquals("parse", ConformanceSuite.operation("operation = \"parse\" # a comment\n"));
    assertEquals("read", ConformanceSuite.operation("# a comment\n\nsource = \"s\"\n"));
    assertEquals(
        "dzn", ConformanceSuite.operation("title = \"t\"\noperation=\"dzn\"\n[input]\nx = 1\n"));
  }

  @ParameterizedTest
  @ValueSource(
      strings = {
        "\"operation\" = \"parse\"",
        "operation.kind = \"parse\"",
        "unknown = 1",
        "operation = \"format\"",
        "operation = 'parse'",
        "operation = \"parse\"\noperation = \"read\"",
      })
  void rejectsMetadataOutsideTheSubset(String metadata) {
    assertThrows(IllegalArgumentException.class, () -> ConformanceSuite.operation(metadata));
  }

  @Test
  void discoversCasesWithoutEnteringThem(@TempDir Path root) throws IOException {
    Path outer = Files.createDirectories(root.resolve("a/outer"));
    Files.writeString(outer.resolve("case.toml"), "");
    Path inner = Files.createDirectories(outer.resolve("inner"));
    Files.writeString(inner.resolve("spec.iris"), "");
    Path binary = Files.createDirectories(root.resolve("b"));
    Files.write(binary.resolve("spec.bin"), new byte[] {(byte) 0xff});
    Files.createDirectories(root.resolve("c"));
    assertEquals(List.of(outer, binary), ConformanceSuite.discover(root));
  }

  @Test
  void readsTheDeferredList() {
    assertEquals(Map.of(READ_CASE, "§10.2"), deferred("cases/a §10.2"));
    assertEquals(Map.of(READ_CASE, "§10.5"), deferred("cases/a §10.5"));
    assertEquals(Map.of(), deferred());
  }

  @ParameterizedTest
  @ValueSource(
      strings = {
        "cases/a",
        "cases/a  §10.2",
        "cases/a §10.2 more",
        " cases/a §10.2",
        "# a comment",
        "",
        "cases/a §10.1",
        "cases/a §10.6",
        "cases/a 10.2",
        "cases/missing §10.2",
        "cases/b §10.2",
      })
  void rejectsADeferredLine(String line) {
    assertThrows(IllegalArgumentException.class, () -> deferred(line));
  }

  @Test
  void rejectsARepeatedDeferredCase() {
    assertThrows(IllegalArgumentException.class, () -> deferred("cases/a §10.2", "cases/a §10.3"));
  }

  @Test
  void acceptsTheExpectedOutcomes() {
    Path other = REPOSITORY.resolve("cases/c");
    var passed = new LinkedHashMap<Path, Boolean>();
    passed.put(READ_CASE, false);
    passed.put(other, true);
    assertEquals(List.of(), ConformanceSuite.problems(passed, Set.of(READ_CASE)));
  }

  @Test
  void reportsAFailingCaseOffTheList() {
    List<String> problems = ConformanceSuite.problems(Map.of(READ_CASE, false), Set.of());
    assertEquals(List.of("fails but is not on the deferred list: " + READ_CASE), problems);
  }

  @Test
  void reportsAPassingCaseOnTheList() {
    List<String> problems = ConformanceSuite.problems(Map.of(READ_CASE, true), Set.of(READ_CASE));
    assertEquals(List.of("passes but is on the deferred list: " + READ_CASE), problems);
  }

  @Test
  void readsTheOutcomesOfABatch() {
    Path other = REPOSITORY.resolve("cases/with space");
    String report =
        "PASS " + READ_CASE + "\nFAIL " + other + "\n  a detail\nSummary: total=2 pass=1\n";
    assertEquals(
        Map.of(READ_CASE, true, other, false),
        ConformanceSuite.outcomes(report, List.of(READ_CASE, other)));
  }

  @Test
  void rejectsAReportThatLeavesOutACase() {
    List<Path> cases = List.of(READ_CASE, REPOSITORY.resolve("cases/c"));
    assertThrows(
        AssertionError.class, () -> ConformanceSuite.outcomes("PASS " + READ_CASE + "\n", cases));
    assertThrows(AssertionError.class, () -> ConformanceSuite.outcomes("error: bad", cases));
  }
}
