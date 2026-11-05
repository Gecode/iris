package dev.gecode.iris.internal.read;

import static java.nio.charset.StandardCharsets.UTF_8;

import dev.gecode.iris.ErrorCode;
import dev.gecode.iris.internal.syntax.Ast.Pattern;
import dev.gecode.iris.internal.syntax.Ast.PatternItem;
import dev.gecode.iris.internal.syntax.Ast.ReadingKind;
import dev.gecode.iris.internal.syntax.Ast.ReadingType;
import dev.gecode.iris.internal.syntax.Numbers;
import java.util.ArrayList;
import java.util.List;
import org.jspecify.annotations.Nullable;

/** Matches line patterns against the current line and defines their captures (§7). */
final class PatternMatcher {
  private final Execution execution;
  private final Input input;
  private final Domains domains;
  private final ValueReader reader;

  /** The captures of the last match. */
  private final List<Captured> captures = new ArrayList<>();

  /** Where the match has got to on the line. */
  private int cursor = 0;

  /** Where the line ends, without its trailing blanks. */
  private int end = 0;

  /**
   * A value captured from a line.
   *
   * @param item the capture
   * @param number the value, or null for a string, which is made only if the match is kept
   * @param from where its text starts
   * @param to where its text ends
   */
  private record Captured(PatternItem.Capture item, @Nullable Stored number, int from, int to) {}

  PatternMatcher(Execution execution, Domains domains, ValueReader reader) {
    this.execution = execution;
    this.input = execution.input();
    this.domains = domains;
    this.reader = reader;
  }

  /** Matches {@code pattern} against the rest of the current line, consuming it if it matches. */
  boolean match(Pattern pattern) {
    if (!matches(pattern)) {
      return false;
    }
    take();
    return true;
  }

  /** Tests whether {@code pattern} matches the rest of the current line, keeping its captures. */
  boolean matches(Pattern pattern) {
    captures.clear();
    input.skip();
    if (input.isAtEnd()) {
      return false;
    }
    cursor = input.position();
    end = input.contentEnd(cursor);
    List<PatternItem> items = pattern.items();
    for (int k = 0; k < items.size(); k++) {
      PatternItem next = k + 1 < items.size() ? items.get(k + 1) : null;
      if (!matchItem(items.get(k), next)) {
        return false;
      }
    }
    skipBlanks();
    return cursor == end;
  }

  private void skipBlanks() {
    byte[] text = input.bytes();
    while (cursor < end && Input.isBlank(text[cursor])) {
      cursor++;
    }
  }

  private boolean matchItem(PatternItem item, @Nullable PatternItem next) {
    return switch (item) {
      case PatternItem.Space space -> {
        int start = cursor;
        skipBlanks();
        yield !space.required() || cursor > start || cursor == end;
      }
      case PatternItem.Literal literal -> matchText(literal.text());
      case PatternItem.Rest _ -> {
        cursor = end;
        yield true;
      }
      case PatternItem.Reference reference -> {
        String wanted = referenced(reference.name());
        yield !wanted.isEmpty() && matchText(wanted);
      }
      case PatternItem.Capture capture -> capture(capture, next);
    };
  }

  private boolean matchText(String wanted) {
    int length = input.matchLength(wanted, cursor, end);
    if (length < 0) {
      return false;
    }
    cursor += length;
    return true;
  }

  /** Returns the value of {@code name} as text, for {@code {$name}} in a pattern. */
  private String referenced(String name) {
    return switch (execution.lookup(name)) {
      case null ->
          throw execution.failure(ErrorCode.NAME_UNDEFINED, "'" + name + "' is not defined yet");
      case Stored.Text value -> value.value();
      case Stored.Int value -> Long.toString(value.value());
      case Stored.Real _, Stored.Array _ ->
          throw execution.failure(
              ErrorCode.PATTERN_REFERENCE, "'" + name + "' must be a string or an integer");
    };
  }

  private boolean capture(PatternItem.Capture capture, @Nullable PatternItem next) {
    ReadingKind kind = capture.type().kind();
    // A word ends before a literal that follows it directly.
    byte stop = next instanceof PatternItem.Literal literal ? firstByte(literal.text()) : 0;
    int from = cursor;
    cursor = captureEnd(kind, stop);
    int to = cursor;
    byte[] text = input.bytes();
    while (to > from && Input.isBlank(text[to - 1])) {
      to--;
    }
    if (to == from) {
      return false;
    }
    Stored number = null;
    if (!ValueReader.readsStrings(kind)) {
      number = reader.number(kind, from, to);
      if (number == null) {
        return false;
      }
    }
    captures.add(new Captured(capture, number, from, to));
    return true;
  }

  private static byte firstByte(String text) {
    return text.substring(0, text.offsetByCodePoints(0, 1)).getBytes(UTF_8)[0];
  }

  private int captureEnd(ReadingKind kind, byte stop) {
    return switch (kind) {
      case REST -> end;
      case WORD -> {
        byte[] text = input.bytes();
        int index = cursor;
        while (index < end && !Input.isBlank(text[index]) && text[index] != stop) {
          index++;
        }
        yield index;
      }
      case INT, FLOAT, NUM, SYMBOL -> numberEnd(kind != ReadingKind.INT);
    };
  }

  /** Returns where a number at the cursor ends, which is where a following literal starts. */
  private int numberEnd(boolean isReal) {
    byte[] text = input.bytes();
    int index = cursor;
    for (; index < end; index++) {
      int c = text[index];
      boolean isPart =
          Numbers.isDigit(c)
              || (index == cursor && Numbers.isSign(c))
              || (isReal && (c == '.' || Numbers.isExponentMarker(c)))
              || (isReal
                  && index > cursor
                  && Numbers.isSign(c)
                  && Numbers.isExponentMarker(text[index - 1]));
      if (!isPart) {
        break;
      }
    }
    return index;
  }

  /** Defines the values captured from the current line, and consumes it. */
  void take() {
    // Every string captured must be valid, even if it is not kept.
    for (Captured capture : captures) {
      if (capture.number() == null) {
        reader.requireValid(capture.from(), capture.to());
      }
    }
    // The captures can use each other in their ranges.
    for (Captured capture : captures) {
      String name = capture.item().name();
      if (name != null) {
        execution.define(name, value(capture));
      }
    }
    for (Captured capture : captures) {
      ReadingType type = capture.item().type();
      if (type instanceof ReadingType.Range || type instanceof ReadingType.Named) {
        Domain domain = domains.of(type);
        if (capture.number() instanceof Stored.Int value && !domain.fits(value)) {
          throw reader.outside(domain, capture.from(), capture.to());
        }
      }
    }
    input.toEndOfLine();
  }

  private Stored value(Captured capture) {
    Stored number = capture.number();
    return number != null ? number : new Stored.Text(input.string(capture.from(), capture.to()));
  }
}
