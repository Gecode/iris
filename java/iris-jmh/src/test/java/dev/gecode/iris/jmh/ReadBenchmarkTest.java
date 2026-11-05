package dev.gecode.iris.jmh;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.Set;
import java.util.stream.Collectors;
import java.util.stream.Stream;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.EnumSource;

class ReadBenchmarkTest {
  private static final Path FORMATS = Path.of(System.getProperty("iris.formats"));

  @ParameterizedTest
  @EnumSource
  void readsTheInput(FormatInput input) throws IOException {
    var benchmark = new ReadBenchmark();
    benchmark.input = input;
    benchmark.load();
    assertFalse(benchmark.read().entries().isEmpty());
  }

  @Test
  void coversEveryCaseThatReads() throws IOException {
    try (Stream<Path> files = Files.walk(FORMATS)) {
      Set<Path> readable =
          files
              .filter(file -> file.endsWith("expected.json"))
              .map(file -> file.resolveSibling("input.txt"))
              .collect(Collectors.toSet());
      Set<Path> measured =
          Arrays.stream(FormatInput.values())
              .map(input -> input.file(FORMATS))
              .collect(Collectors.toSet());
      assertEquals(readable, measured);
    }
  }
}
