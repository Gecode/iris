package dev.gecode.iris;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.ValueSource;

class ConformanceSuiteTest {
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
}
