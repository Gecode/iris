package dev.gecode.iris.internal.syntax;

import dev.gecode.iris.ErrorCode;
import dev.gecode.iris.IrisException;
import dev.gecode.iris.internal.syntax.Ast.Pattern;
import dev.gecode.iris.internal.syntax.Ast.PatternItem;
import dev.gecode.iris.internal.syntax.Ast.ReadingType;
import java.util.ArrayList;
import java.util.List;
import java.util.function.Function;
import org.jspecify.annotations.Nullable;

/** Compiles the decoded text of a line pattern (§7.2, appendix A.4). */
final class PatternParser {
  private static final String ANY_TEXT = "...";

  private final String source;
  private final @Nullable String origin;
  private final int line;
  private final List<Part> parts = new ArrayList<>();
  private final StringBuilder literal = new StringBuilder();
  private int position = 0;

  private PatternParser(String source, @Nullable String origin, int line) {
    this.source = source;
    this.origin = origin;
    this.line = line;
  }

  /**
   * Compiles pattern {@code source} on {@code line}.
   *
   * <p>The whole structure is compiled first, so that a structural failure precedes a failure in
   * any capture type. Then {@code captureType} parses the text of each capture's type, in capture
   * order.
   */
  static Pattern compile(
      String source, @Nullable String origin, int line, Function<String, ReadingType> captureType) {
    List<Part> parts = new PatternParser(source, origin, line).parts();
    List<PatternItem> items = new ArrayList<>();
    for (Part part : parts) {
      items.add(
          switch (part) {
            case Part.Compiled compiled -> compiled.item();
            case Part.UntypedCapture capture ->
                new PatternItem.Capture(capture.name(), captureType.apply(capture.typeText()));
          });
    }
    return new Pattern(source, withRequiredSpaces(items));
  }

  /** A part of a pattern whose structure is known. */
  private sealed interface Part {
    /** An item that needs no further parsing. */
    record Compiled(PatternItem item) implements Part {}

    /** A capture whose type is still text. */
    record UntypedCapture(@Nullable String name, String typeText) implements Part {}
  }

  private List<Part> parts() {
    while (position < source.length()) {
      char c = source.charAt(position);
      if (isBlank(c)) {
        blanks();
      } else if (c == '{') {
        capture();
      } else if (source.startsWith(ANY_TEXT, position)) {
        anyText();
      } else {
        literal.append(c);
        position++;
      }
    }
    flushLiteral();
    return parts;
  }

  private IrisException failure(String message) {
    return SpecErrors.at(ErrorCode.SPEC_PATTERN, origin, line, message);
  }

  private static boolean isBlank(char c) {
    return c == ' ' || c == '\t' || c == '\r';
  }

  private static boolean isName(String text) {
    return !text.isEmpty()
        && !Character.isDigit(text.charAt(0))
        && text.chars().allMatch(Lexer::isNameCharacter);
  }

  private void add(PatternItem item) {
    parts.add(new Part.Compiled(item));
  }

  private void flushLiteral() {
    if (!literal.isEmpty()) {
      add(new PatternItem.Literal(literal.toString()));
      literal.setLength(0);
    }
  }

  private void blanks() {
    flushLiteral();
    while (position < source.length() && isBlank(source.charAt(position))) {
      position++;
    }
    add(new PatternItem.Space(false));
  }

  /** Capture text ends at the first closing brace, even inside quoted type text. */
  private void capture() {
    flushLiteral();
    int end = source.indexOf('}', position);
    if (end < 0) {
      throw failure("unterminated capture in pattern \"" + source + "\"");
    }
    String text = source.substring(position + 1, end);
    position = end + 1;
    if (text.startsWith("$")) {
      reference(text.substring(1));
      return;
    }
    int colon = text.indexOf(':');
    String name = colon < 0 ? text : text.substring(0, colon);
    if (!name.isEmpty() && !isName(name)) {
      throw failure("capture name '" + name + "' in pattern \"" + source + "\" is not a name");
    }
    String definedName = name.isEmpty() || name.equals("_") ? null : name;
    parts.add(new Part.UntypedCapture(definedName, colon < 0 ? "word" : text.substring(colon + 1)));
  }

  private void reference(String name) {
    if (!isName(name)) {
      throw failure("missing or bad name in pattern \"" + source + "\"");
    }
    add(new PatternItem.Reference(name));
  }

  private void anyText() {
    flushLiteral();
    int end = position + ANY_TEXT.length();
    if (!source.substring(end).chars().allMatch(c -> c == ' ' || c == '\t')) {
      throw failure("'...' must end the pattern \"" + source + "\"");
    }
    add(new PatternItem.Rest());
    position = end;
  }

  /**
   * Marks the spaces between two word-like items as required, such as in {@code "p cnf"} or {@code
   * "{a:int} {b:int}"}.
   */
  private static List<PatternItem> withRequiredSpaces(List<PatternItem> items) {
    List<PatternItem> result = new ArrayList<>(items);
    for (int k = 1; k + 1 < items.size(); k++) {
      if (items.get(k) instanceof PatternItem.Space) {
        boolean required = endsWord(items.get(k - 1)) && startsWord(items.get(k + 1));
        result.set(k, new PatternItem.Space(required));
      }
    }
    return List.copyOf(result);
  }

  private static boolean endsWord(PatternItem item) {
    return switch (item) {
      case PatternItem.Literal literal ->
          Lexer.isNameCharacter(literal.text().charAt(literal.text().length() - 1));
      case PatternItem.Capture _, PatternItem.Reference _ -> true;
      case PatternItem.Space _, PatternItem.Rest _ -> false;
    };
  }

  private static boolean startsWord(PatternItem item) {
    return switch (item) {
      case PatternItem.Literal literal -> Lexer.isNameCharacter(literal.text().charAt(0));
      case PatternItem.Capture _, PatternItem.Reference _ -> true;
      case PatternItem.Space _, PatternItem.Rest _ -> false;
    };
  }
}
