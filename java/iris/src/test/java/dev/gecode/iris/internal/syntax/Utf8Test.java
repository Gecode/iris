package dev.gecode.iris.internal.syntax;

import static org.junit.jupiter.api.Assertions.assertEquals;

import java.util.HexFormat;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.CsvSource;
import org.junit.jupiter.params.provider.ValueSource;

class Utf8Test {
  private static byte[] bytes(String hex) {
    return HexFormat.of().parseHex(hex);
  }

  @ParameterizedTest
  @ValueSource(
      strings = {"", "7f", "c280", "dfbf", "e0a080", "ed9fbf", "ee8080", "f0908080", "f48fbfbf"})
  void acceptsTheEdgesOfEachRange(String hex) {
    assertEquals(-1, Utf8.firstInvalid(bytes(hex)));
  }

  @ParameterizedTest
  @CsvSource({
    "80, 0",
    "c0af, 0",
    "c1bf, 0",
    "e08080, 0",
    "eda080, 0",
    "edbfbf, 0",
    "f0808080, 0",
    "f4908080, 0",
    "f5808080, 0",
    "41c2, 1",
    "41e0a0, 1",
    "41f09080, 1",
    "41c241, 1",
  })
  void findsTheFirstInvalidByte(String hex, int offset) {
    assertEquals(offset, Utf8.firstInvalid(bytes(hex)));
  }

  @Test
  void countsLinesAndByteColumns() {
    byte[] text = bytes("410a" + "c3a5" + "41" + "0a" + "42");
    assertEquals(1, Utf8.line(text, 1));
    assertEquals(2, Utf8.line(text, 2));
    assertEquals(3, Utf8.line(text, 6));
    assertEquals(1, Utf8.column(text, 2));
    assertEquals(3, Utf8.column(text, 4));
    assertEquals(4, Utf8.column(text, 5));
  }
}
