package dev.gecode.iris.internal.syntax;

import static java.nio.charset.StandardCharsets.US_ASCII;
import static java.nio.charset.StandardCharsets.UTF_8;

import dev.gecode.iris.ErrorCode;
import dev.gecode.iris.IrisException;
import java.io.ByteArrayOutputStream;
import java.util.ArrayList;
import java.util.List;
import java.util.OptionalDouble;
import java.util.OptionalLong;
import org.jspecify.annotations.Nullable;

/** Splits reader-spec source, which is valid UTF-8, into tokens (§§5.2–5.5). */
public final class Lexer {
  /** The prefix of the names of host constants. */
  static final String HOST_PREFIX = "gecode.";

  private static final List<String> OPERATORS =
      List.of("..<", "..", "==", "!=", "<=", ">=", "&&", "||");

  /** NUL is punctuation without a use, as for Gecode (§5.2). */
  private static final String SINGLE_PUNCTUATION = "{}[]()=<>+-*/%^!,.:\0";

  private final byte[] source;
  private final @Nullable String origin;
  private int line;
  private int position = 0;
  private boolean afterSpace = true;

  private Lexer(byte[] source, @Nullable String origin, int line) {
    this.source = source;
    this.origin = origin;
    this.line = line;
  }

  /** Returns the tokens of a whole reader spec, ending with a token of kind END. */
  public static List<Token> lex(byte[] source, @Nullable String origin) {
    return lex(source, origin, 1);
  }

  /** Returns the tokens of {@code source}, whose first line is {@code line}. */
  static List<Token> lex(byte[] source, @Nullable String origin, int line) {
    return new Lexer(source, origin, line).tokens();
  }

  static boolean isNameCharacter(int c) {
    return isDigit(c) || (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c == '_';
  }

  private static boolean isDigit(int c) {
    return c >= '0' && c <= '9';
  }

  private List<Token> tokens() {
    List<Token> tokens = new ArrayList<>();
    while (skipSpace()) {
      tokens.add(token());
    }
    tokens.add(Token.of(Token.Kind.END, "", line, false));
    return tokens;
  }

  private IrisException failure(ErrorCode code, String message) {
    return SpecErrors.at(code, origin, line, message);
  }

  /** Skips white space and comments, and returns whether a token follows. */
  private boolean skipSpace() {
    while (position < source.length) {
      byte c = source[position];
      if (c == '#') {
        skipComment();
      } else if (c == '\n') {
        line++;
        position++;
      } else if (c == ' ' || c == '\t' || c == '\r' || c == 0x0b || c == '\f') {
        position++;
      } else {
        return true;
      }
      afterSpace = true;
    }
    return false;
  }

  private void skipComment() {
    while (position < source.length && source[position] != '\n') {
      position++;
    }
  }

  private Token token() {
    boolean glued = !afterSpace;
    afterSpace = false;
    byte c = source[position];
    if (c == '"') {
      return string(glued);
    }
    if (isDigit(c)) {
      return number(glued);
    }
    if (isNameCharacter(c)) {
      return name(glued);
    }
    return punctuation(glued);
  }

  /** A string ends on its line: an LF, even after a backslash, leaves it unterminated. */
  private Token string(boolean glued) {
    position++;
    var characters = new ByteArrayOutputStream();
    while (position < source.length && source[position] != '"') {
      byte c = source[position];
      if (c == '\n' || (c == '\\' && position + 1 == source.length)) {
        throw failure(ErrorCode.SPEC_STRING, "unterminated string");
      }
      characters.write(c == '\\' ? unescape(position + 1) : c);
      position += c == '\\' ? 2 : 1;
    }
    if (position == source.length) {
      throw failure(ErrorCode.SPEC_STRING, "unterminated string");
    }
    position++;
    return Token.of(Token.Kind.STRING, characters.toString(UTF_8), line, glued);
  }

  /** Returns the character that the escape with the character at {@code offset} stands for. */
  private char unescape(int offset) {
    return switch (source[offset]) {
      case 'n' -> '\n';
      case 't' -> '\t';
      case '\\' -> '\\';
      case '"' -> '"';
      case '\n' -> throw failure(ErrorCode.SPEC_STRING, "unterminated string");
      default ->
          throw failure(
              ErrorCode.SPEC_STRING,
              "unknown escape '\\"
                  + characterAt(offset)
                  + "' in string (use \\n, \\t, \\\\, or \\\")");
    };
  }

  private String characterAt(int offset) {
    return new String(source, offset, Utf8.sequenceLength(source, offset), UTF_8);
  }

  /** A minus sign before a number is an operator, and a range such as 0..9 is two numbers. */
  private Token number(boolean glued) {
    String candidate = new String(source, position, numberEnd() - position, US_ASCII);
    int range = candidate.indexOf("..");
    String text = range < 0 ? candidate : candidate.substring(0, range);
    int start = position;
    position += text.length();
    if (text.chars().allMatch(Lexer::isDigit)) {
      OptionalLong value = Numbers.parseInteger(source, start, position);
      if (value.isEmpty()) {
        throw failure(ErrorCode.SPEC_NUMBER, "bad number " + text);
      }
      return new Token(Token.Kind.INTEGER, text, value.getAsLong(), 0.0, line, glued);
    }
    OptionalDouble value = Numbers.parseFloat(source, start, position);
    if (value.isEmpty()) {
      throw failure(ErrorCode.SPEC_NUMBER, "bad number " + text);
    }
    return new Token(Token.Kind.FLOAT, text, 0, value.getAsDouble(), line, glued);
  }

  /** Returns where the digits, dots, exponent markers and exponent signs from here end. */
  private int numberEnd() {
    int end = position;
    while (end < source.length && isNumberCharacter(end)) {
      end++;
    }
    return end;
  }

  private boolean isNumberCharacter(int offset) {
    byte c = source[offset];
    if (c == '+' || c == '-') {
      return offset > position && isExponentMarker(source[offset - 1]);
    }
    return isDigit(c) || c == '.' || isExponentMarker(c);
  }

  private static boolean isExponentMarker(byte c) {
    return c == 'e' || c == 'E';
  }

  private Token name(boolean glued) {
    int end = nameEnd(position);
    if (atHostName(end)) {
      end = nameEnd(end + 1);
    }
    String text = new String(source, position, end - position, US_ASCII);
    position = end;
    return Token.of(Token.Kind.IDENTIFIER, text, line, glued);
  }

  private int nameEnd(int start) {
    int end = start;
    while (end < source.length && isNameCharacter(source[end])) {
      end++;
    }
    return end;
  }

  /** Tests whether the name from here to {@code nameEnd} and what follows form a host constant. */
  private boolean atHostName(int nameEnd) {
    return nameEnd + 1 < source.length
        && source[nameEnd] == '.'
        && isNameCharacter(source[nameEnd + 1])
        && HOST_PREFIX.equals(new String(source, position, nameEnd + 1 - position, US_ASCII));
  }

  private Token punctuation(boolean glued) {
    for (String operator : OPERATORS) {
      if (startsWith(operator)) {
        position += operator.length();
        return Token.of(Token.Kind.PUNCTUATION, operator, line, glued);
      }
    }
    int c = source[position];
    if (c < 0 || SINGLE_PUNCTUATION.indexOf(c) < 0) {
      throw failure(
          ErrorCode.SPEC_CHARACTER, "unexpected character '" + characterAt(position) + "'");
    }
    position++;
    return Token.of(Token.Kind.PUNCTUATION, String.valueOf((char) c), line, glued);
  }

  private boolean startsWith(String operator) {
    if (position + operator.length() > source.length) {
      return false;
    }
    for (int k = 0; k < operator.length(); k++) {
      if (source[position + k] != operator.charAt(k)) {
        return false;
      }
    }
    return true;
  }
}
