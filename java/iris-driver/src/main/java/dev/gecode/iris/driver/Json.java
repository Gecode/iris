package dev.gecode.iris.driver;

import java.util.LinkedHashMap;
import java.util.SequencedMap;

/** The JSON of the driver protocol: the document of a parse and the decoder of parameters. */
final class Json {
  /** The document of a successful parse. */
  static final String PARSED = "{\"iris\":\"0.1\",\"parsed\":true}\n";

  private final String text;
  private int position = 0;

  private Json(String text) {
    this.text = text;
  }

  /**
   * Decodes {@code text}, a JSON object of strings, keeping its order.
   *
   * @throws IllegalArgumentException for anything else, such as a repeated key, a value that is not
   *     a string, or an unpaired surrogate
   */
  static SequencedMap<String, String> params(String text) {
    Json json = new Json(text);
    SequencedMap<String, String> params = json.object();
    json.skipSpace();
    if (json.position != text.length()) {
      throw json.invalid("content after the object");
    }
    return params;
  }

  private IllegalArgumentException invalid(String problem) {
    return new IllegalArgumentException("invalid params: " + problem + " at index " + position);
  }

  private void skipSpace() {
    while (position < text.length() && " \t\r\n".indexOf(text.charAt(position)) >= 0) {
      position++;
    }
  }

  private boolean accept(char wanted) {
    skipSpace();
    if (position < text.length() && text.charAt(position) == wanted) {
      position++;
      return true;
    }
    return false;
  }

  private void expect(char wanted) {
    if (!accept(wanted)) {
      throw invalid("expected '" + wanted + "'");
    }
  }

  private SequencedMap<String, String> object() {
    SequencedMap<String, String> entries = new LinkedHashMap<>();
    expect('{');
    if (accept('}')) {
      return entries;
    }
    do {
      skipSpace();
      String name = string();
      expect(':');
      skipSpace();
      if (entries.putIfAbsent(name, string()) != null) {
        throw invalid("repeated key");
      }
    } while (accept(','));
    expect('}');
    return entries;
  }

  private String string() {
    expect('"');
    StringBuilder decoded = new StringBuilder();
    while (true) {
      if (position == text.length()) {
        throw invalid("unterminated string");
      }
      char c = text.charAt(position++);
      if (c == '"') {
        requireText(decoded);
        return decoded.toString();
      }
      if (c < 0x20) {
        throw invalid("control character in a string");
      }
      decoded.append(c == '\\' ? escape() : c);
    }
  }

  private char escape() {
    if (position == text.length()) {
      throw invalid("unterminated escape");
    }
    char c = text.charAt(position++);
    return switch (c) {
      case '"', '\\', '/' -> c;
      case 'b' -> '\b';
      case 'f' -> '\f';
      case 'n' -> '\n';
      case 'r' -> '\r';
      case 't' -> '\t';
      case 'u' -> codeUnit();
      default -> throw invalid("unknown escape");
    };
  }

  private char codeUnit() {
    if (position + 4 > text.length()) {
      throw invalid("short unicode escape");
    }
    int unit = 0;
    for (int k = 0; k < 4; k++) {
      char digit = text.charAt(position++);
      // Character.digit alone also accepts digits outside ASCII, which JSON does not.
      if (digit >= 0x80 || Character.digit(digit, 16) < 0) {
        throw invalid("bad unicode escape");
      }
      unit = unit * 16 + Character.digit(digit, 16);
    }
    return (char) unit;
  }

  private void requireText(CharSequence decoded) {
    int index = 0;
    while (index < decoded.length()) {
      int codePoint = Character.codePointAt(decoded, index);
      if (codePoint >= Character.MIN_SURROGATE && codePoint <= Character.MAX_SURROGATE) {
        throw invalid("unpaired surrogate in a string");
      }
      index += Character.charCount(codePoint);
    }
  }
}
