package dev.gecode.iris.processor;

import java.util.ArrayList;
import java.util.List;

/** Java string literals in ASCII, as {@code iris gen --lang java} writes them. */
final class Literals {
  /**
   * The most characters of the spec's text in one literal. Each takes at most three bytes in a
   * class file's constant, which holds 65535.
   */
  private static final int WIDEST_LITERAL = 8192;

  private Literals() {}

  /**
   * Returns {@code text} as a literal: everything outside printable ASCII is an escape, and
   * characters outside the BMP are two {@code \}{@code u} escapes of their UTF-16 units.
   */
  static String quoted(String text) {
    var literal = new StringBuilder(text.length() + 2).append('"');
    for (int index = 0; index < text.length(); index++) {
      char character = text.charAt(index);
      switch (character) {
        case '\\' -> literal.append("\\\\");
        case '"' -> literal.append("\\\"");
        case '\n' -> literal.append("\\n");
        case '\r' -> literal.append("\\r");
        case '\t' -> literal.append("\\t");
        default -> {
          if (character >= ' ' && character <= '~') {
            literal.append(character);
          } else {
            literal.append(String.format("\\u%04x", (int) character));
          }
        }
      }
    }
    return literal.append('"').toString();
  }

  /**
   * Returns the literals of {@code text}: one for each line, with its line break, and a line wider
   * than a literal can hold in pieces that keep surrogate pairs whole.
   */
  static List<String> lines(String text) {
    List<String> literals = new ArrayList<>();
    int start = 0;
    while (start < text.length()) {
      int lineEnd = text.indexOf('\n', start);
      int end = lineEnd < 0 ? text.length() : lineEnd + 1;
      end = Math.min(end, start + WIDEST_LITERAL);
      if (end < text.length() && Character.isHighSurrogate(text.charAt(end - 1))) {
        end--;
      }
      literals.add(quoted(text.substring(start, end)));
      start = end;
    }
    return literals;
  }
}
