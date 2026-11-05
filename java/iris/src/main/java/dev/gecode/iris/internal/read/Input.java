package dev.gecode.iris.internal.read;

import static java.nio.charset.StandardCharsets.UTF_8;

import dev.gecode.iris.internal.syntax.Numbers;
import java.util.ArrayList;
import java.util.List;
import org.jspecify.annotations.Nullable;

/**
 * The bytes of an input with a position in them, and the rules that split them into lines and
 * tokens (§4).
 *
 * <p>Reading can return to a position many times, as a repeat that reads nothing or a line that
 * several patterns try does. Whatever scans ahead therefore remembers where it stopped, so that the
 * cost of a read stays linear in the input.
 */
final class Input {
  private static final byte PLAIN = 0;
  private static final byte SEPARATOR = 1;
  private static final byte SPLIT = 2;

  private final byte[] text;
  private final @Nullable String origin;

  /** What each byte is in tokens. */
  private final byte[] kinds = new byte[256];

  private final List<byte[]> comments = new ArrayList<>();
  private final List<byte[]> stops = new ArrayList<>();

  /** Which bytes start a comment or stop prefix. */
  private final boolean[] startsPrefix = new boolean[256];

  private int position = 0;
  private int line = 1;
  private int lineStart = 0;
  private long lastInt = 0;

  /** The blanks at the start of the line starting at {@link #blanksLine} end here, or later. */
  private int blanksEnd = 0;

  private int blanksLine = 0;

  /** The last call of {@link #skip} went from here to the place below. */
  private int skippedFrom = -1;

  private int skippedPosition = 0;
  private int skippedLine = 0;
  private int skippedLineStart = 0;

  /** The line of the positions from {@link #lineEndFrom} up to {@link #lineEnd} ends there. */
  private int lineEndFrom = 0;

  private int lineEnd = -1;

  Input(byte[] text, @Nullable String origin) {
    this.text = text;
    this.origin = origin;
    for (int c = 0; c < kinds.length; c++) {
      kinds[c] = isSpace(c) ? SEPARATOR : PLAIN;
    }
  }

  static boolean isBlank(int c) {
    return c == ' ' || c == '\t' || c == '\r';
  }

  private static boolean isSpace(int c) {
    return isBlank(c) || c == '\n' || c == 0x0b || c == '\f';
  }

  private static boolean isNameByte(int c) {
    return Numbers.isDigit(c) || (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c == '_';
  }

  byte[] bytes() {
    return text;
  }

  @Nullable String origin() {
    return origin;
  }

  int position() {
    return position;
  }

  /** Tests whether the position is at the end of the input. */
  boolean isAtEnd() {
    return position >= text.length;
  }

  /** Returns the byte at the position, which is not the end, as an unsigned value. */
  int current() {
    return text[position] & 0xff;
  }

  /** Returns the current place. */
  Place here() {
    return new Place(position, line, lineStart);
  }

  /** Returns the place of {@code target}, which is not after the current position. */
  Place place(int target) {
    int targetLine = line;
    for (int k = target; k < position; k++) {
      if (text[k] == '\n') {
        targetLine--;
      }
    }
    int start = target;
    if (targetLine == line) {
      start = lineStart;
    } else {
      while (start > 0 && text[start - 1] != '\n') {
        start--;
      }
    }
    return new Place(target, targetLine, start);
  }

  /** Sets the position to {@code target} on the current line. */
  void moveOnLine(int target) {
    position = target;
  }

  /** Moves forward to {@code target}. */
  void advance(int target) {
    while (position < target) {
      if (text[position] == '\n') {
        line++;
        lineStart = position + 1;
      }
      position++;
    }
  }

  /** Adds comment prefixes (§4.7). */
  void addComments(List<String> prefixes) {
    addPrefixes(comments, prefixes);
  }

  /** Adds prefixes of the lines that end the data (§4.8). */
  void addStops(List<String> prefixes) {
    addPrefixes(stops, prefixes);
  }

  private void addPrefixes(List<byte[]> into, List<String> prefixes) {
    for (String prefix : prefixes) {
      byte[] encoded = prefix.getBytes(UTF_8);
      into.add(encoded);
      if (encoded.length > 0) {
        startsPrefix[encoded[0] & 0xff] = true;
      }
    }
    skippedFrom = -1;
  }

  /** Makes the bytes of {@code characters} tokens by themselves, unless they separate tokens. */
  void addSplit(String characters) {
    for (byte c : characters.getBytes(UTF_8)) {
      if (kinds[c & 0xff] == PLAIN) {
        kinds[c & 0xff] = SPLIT;
      }
    }
    skippedFrom = -1;
  }

  /** Makes the bytes of {@code characters} separate tokens like white space. */
  void addSeparators(String characters) {
    for (byte c : characters.getBytes(UTF_8)) {
      kinds[c & 0xff] = SEPARATOR;
    }
    skippedFrom = -1;
  }

  private boolean isSeparator(int index) {
    return kinds[text[index] & 0xff] == SEPARATOR;
  }

  private boolean isSplit(int index) {
    return kinds[text[index] & 0xff] == SPLIT;
  }

  private boolean isPlain(int index) {
    return kinds[text[index] & 0xff] == PLAIN;
  }

  /** Returns where the line that holds {@code from} ends: at its newline or the end of input. */
  int lineEnd(int from) {
    if (from < lineEndFrom || from > lineEnd) {
      int end = from;
      while (end < text.length && text[end] != '\n') {
        end++;
      }
      lineEndFrom = from;
      lineEnd = end;
    }
    return lineEnd;
  }

  /** Returns where the line from {@code from} ends without its trailing blanks. */
  int contentEnd(int from) {
    int end = lineEnd(from);
    while (end > from && isBlank(text[end - 1])) {
      end--;
    }
    return end;
  }

  /** Moves to the end of the current line. */
  void toEndOfLine() {
    position = lineEnd(position);
  }

  private boolean isInLine(int index) {
    return index < text.length && text[index] != '\n';
  }

  /** Tests whether only blanks precede the position on its line. */
  private boolean isAtLineStart() {
    if (blanksLine != lineStart) {
      blanksLine = lineStart;
      blanksEnd = lineStart;
    }
    while (blanksEnd < position && isBlank(text[blanksEnd])) {
      blanksEnd++;
    }
    return blanksEnd >= position;
  }

  /** Tests whether the text at the position starts with {@code prefix}, as a whole name. */
  private boolean startsWith(byte[] prefix) {
    int after = position + prefix.length;
    if (prefix.length == 0 || after > text.length) {
      return false;
    }
    for (int k = 0; k < prefix.length; k++) {
      if (text[position + k] != prefix[k]) {
        return false;
      }
    }
    return !isNameByte(prefix[prefix.length - 1] & 0xff)
        || after >= text.length
        || !isNameByte(text[after] & 0xff);
  }

  private boolean startsWithAny(List<byte[]> prefixes) {
    for (byte[] prefix : prefixes) {
      if (startsWith(prefix)) {
        return true;
      }
    }
    return false;
  }

  /** Skips white space, comment lines, and everything after a stop line. */
  void skip() {
    if (position == skippedFrom) {
      position = skippedPosition;
      line = skippedLine;
      lineStart = skippedLineStart;
      return;
    }
    int from = position;
    skipToToken();
    skippedFrom = from;
    skippedPosition = position;
    skippedLine = line;
    skippedLineStart = lineStart;
  }

  private void skipToToken() {
    while (true) {
      while (position < text.length && isSeparator(position)) {
        if (text[position] == '\n') {
          line++;
          lineStart = position + 1;
        }
        position++;
      }
      if (position >= text.length || !startsPrefix[current()] || !isAtLineStart()) {
        return;
      }
      if (startsWithAny(comments)) {
        toEndOfLine();
      } else if (startsWithAny(stops)) {
        advance(text.length);
        return;
      } else {
        return;
      }
    }
  }

  /** Skips to the next data (see {@link #skip}), and returns whether there is any. */
  boolean skipToData() {
    skip();
    return position < text.length;
  }

  /** Skips separators, and returns whether data follows on the line. */
  boolean skipToLineData() {
    while (isInLine(position) && isSeparator(position)) {
      position++;
    }
    return isInLine(position);
  }

  /** Returns where the next data starts (see {@link #skip}), without moving there. */
  int nextData() {
    int fromPosition = position;
    int fromLine = line;
    int fromLineStart = lineStart;
    skip();
    int next = position;
    position = fromPosition;
    line = fromLine;
    lineStart = fromLineStart;
    return next;
  }

  /** Returns where the token at the position ends. */
  int tokenEnd() {
    int end = position;
    if (end < text.length && isSplit(end)) {
      return end + 1;
    }
    while (end < text.length && isPlain(end)) {
      end++;
    }
    return end;
  }

  /**
   * Reads the token at the position as an integer, which {@link #lastInt} then returns, and returns
   * whether it is one. It accepts what {@link Numbers#parseInteger} accepts, without a detour
   * through the token's end.
   */
  boolean readIntToken() {
    int size = text.length;
    int index = position;
    if (index < size && isSplit(index)) {
      if (!Numbers.isDigit(text[index])) {
        return false;
      }
      lastInt = (long) text[index] - '0';
      position = index + 1;
      return true;
    }
    boolean isNegative = index < size && text[index] == '-';
    if (index < size && Numbers.isSign(text[index])) {
      index++;
    }
    int digits = index;
    // Accumulated negatively, as the negative range is the larger one.
    long limit = isNegative ? Long.MIN_VALUE : -Long.MAX_VALUE;
    long value = 0;
    for (; index < size && Numbers.isDigit(text[index]) && isPlain(index); index++) {
      int digit = text[index] - '0';
      if (value < (limit + digit) / 10) {
        return false;
      }
      value = value * 10 - digit;
    }
    // The token must end after the digits.
    if (index == digits || (index < size && isPlain(index))) {
      return false;
    }
    lastInt = isNegative ? value : -value;
    position = index;
    return true;
  }

  /** Returns the integer that {@link #readIntToken} read last. */
  long lastInt() {
    return lastInt;
  }

  /** Tests whether the token at the position is {@code token}. */
  boolean isToken(String token) {
    int end = tokenEnd();
    return matchLength(token, position, end) == end - position;
  }

  /**
   * Returns how many bytes {@code wanted} takes in UTF-8 if it is what the input holds from {@code
   * from}, before {@code end}, or -1. It stops at the first difference, so that a long string read
   * from the input costs little to compare with a line it does not match.
   */
  int matchLength(String wanted, int from, int end) {
    int index = from;
    int k = 0;
    while (k < wanted.length()) {
      int codePoint = wanted.codePointAt(k);
      k += Character.charCount(codePoint);
      if (codePoint < 0x80) {
        if (index >= end || text[index] != (byte) codePoint) {
          return -1;
        }
        index++;
      } else {
        byte[] encoded = Character.toString(codePoint).getBytes(UTF_8);
        for (byte b : encoded) {
          if (index >= end || text[index] != b) {
            return -1;
          }
          index++;
        }
      }
    }
    return index - from;
  }

  /** Returns the valid UTF-8 from {@code from} up to {@code to} as a string. */
  String string(int from, int to) {
    return new String(text, from, to - from, UTF_8);
  }
}
