package dev.gecode.iris;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.ValueSource;

class ConformanceSuiteTest {
  private static final Path REPOSITORY = Path.of("/repository");
  private static final Path READ_CASE = REPOSITORY.resolve("cases/a");

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
  void readsTheOutcomesOfABatch() {
    Path other = REPOSITORY.resolve("cases/with space");
    String report =
        "PASS " + READ_CASE + "\nFAIL " + other + "\n  a detail\nSummary: total=2 pass=1\n";
    assertEquals(
        Map.of(READ_CASE, true, other, false),
        ConformanceSuite.outcomes(report, List.of(READ_CASE, other)));
  }

  @Test
  void readsAVerbatimPathFromTheRunner() {
    assertEquals("\\\\?\\D:\\a\\case", ConformanceSuite.withNativeSeparators("//?/D:/a/case"));
    assertEquals(
        "\\\\?\\UNC\\host\\share\\case",
        ConformanceSuite.withNativeSeparators("//?/UNC/host/share/case"));
    assertEquals("/a/case", ConformanceSuite.withNativeSeparators("/a/case"));
  }

  @Test
  void quotesTheDriverCommandForTheRunner() {
    String command = ConformanceSuite.driverCommand(Path.of("it's", "java"), Path.of("home"));
    assertTrue(command.startsWith("'it'\\''s" + File.separator + "java' '-XX:"), command);
    assertTrue(
        command.endsWith(
            " '-cp' 'home"
                + File.separator
                + "lib"
                + File.separator
                + "*' 'dev.gecode.iris.driver.Main'"),
        command);
  }

  @Test
  void rejectsAReportThatLeavesOutACase() {
    List<Path> cases = List.of(READ_CASE, REPOSITORY.resolve("cases/c"));
    assertThrows(
        AssertionError.class, () -> ConformanceSuite.outcomes("PASS " + READ_CASE + "\n", cases));
    assertThrows(AssertionError.class, () -> ConformanceSuite.outcomes("error: bad", cases));
  }
}
