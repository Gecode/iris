package dev.gecode.iris.internal.syntax;

/** Strict UTF-8 validation and the byte positions it reports (§5.1). */
public final class Utf8 {
  private Utf8() {}

  /**
   * Returns the offset of the first byte of the first ill-formed subsequence of {@code text}, or -1
   * if it is valid.
   *
   * <p>A truncated sequence is ill-formed at its initial byte. Overlong forms, surrogates and
   * values above U+10FFFF are ill-formed.
   */
  public static int firstInvalid(byte[] text) {
    int offset = 0;
    while (offset < text.length) {
      int length = sequenceLength(text, offset);
      if (length == 0) {
        return offset;
      }
      offset += length;
    }
    return -1;
  }

  /** Returns the line of {@code offset}: one plus the number of LF bytes before it. */
  public static int line(byte[] text, int offset) {
    int line = 1;
    for (int k = 0; k < offset; k++) {
      if (text[k] == '\n') {
        line++;
      }
    }
    return line;
  }

  /** Returns the column of {@code offset}: one plus the number of bytes since the last LF. */
  public static int column(byte[] text, int offset) {
    int lineStart = offset;
    while (lineStart > 0 && text[lineStart - 1] != '\n') {
      lineStart--;
    }
    return offset - lineStart + 1;
  }

  /** Returns the length of the well-formed sequence at {@code offset}, or 0 if there is none. */
  static int sequenceLength(byte[] text, int offset) {
    int first = text[offset] & 0xff;
    if (first < 0x80) {
      return 1;
    }
    if (first < 0xc2 || first > 0xf4) {
      return 0;
    }
    int length = first < 0xe0 ? 2 : first < 0xf0 ? 3 : 4;
    if (offset + length > text.length) {
      return 0;
    }
    // The second byte's range excludes overlong forms, surrogates and values above U+10FFFF.
    int lowest =
        switch (first) {
          case 0xe0 -> 0xa0;
          case 0xf0 -> 0x90;
          default -> 0x80;
        };
    int highest =
        switch (first) {
          case 0xed -> 0x9f;
          case 0xf4 -> 0x8f;
          default -> 0xbf;
        };
    int second = text[offset + 1] & 0xff;
    if (second < lowest || second > highest) {
      return 0;
    }
    for (int k = 2; k < length; k++) {
      if ((text[offset + k] & 0xc0) != 0x80) {
        return 0;
      }
    }
    return length;
  }
}
