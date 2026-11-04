package dev.gecode.iris.driver;

import dev.gecode.iris.IrisError;
import dev.gecode.iris.Location;
import java.util.List;
import java.util.stream.Collectors;
import org.jspecify.annotations.Nullable;

/** Encodes the documents of the driver protocol in canonical JSON (§3.7). */
final class Json {
  /** The document of a successful parse. */
  static final String PARSED = "{\"iris\":\"0.1\",\"parsed\":true}\n";

  private Json() {}

  /** Returns the canonical error document of {@code error}. */
  static String error(IrisError error) {
    return "{\"code\":"
        + string(error.code().code())
        + ",\"message\":"
        + string(error.message())
        + ",\"input_location\":"
        + location(error.inputLocation())
        + ",\"spec_location\":"
        + location(error.specLocation())
        + ",\"context\":"
        + nullableString(error.context())
        + ",\"record_path\":"
        + path(error.recordPath())
        + "}\n";
  }

  private static String location(@Nullable Location location) {
    if (location == null) {
      return "null";
    }
    return "{\"origin\":"
        + nullableString(location.origin())
        + ",\"line\":"
        + location.line()
        + ",\"column\":"
        + location.column()
        + "}";
  }

  private static String path(@Nullable List<Integer> path) {
    if (path == null) {
      return "null";
    }
    return path.stream().map(String::valueOf).collect(Collectors.joining(",", "[", "]"));
  }

  private static String nullableString(@Nullable String text) {
    return text == null ? "null" : string(text);
  }

  /** Encodes {@code text}, leaving non-ASCII characters as they are (§3.6). */
  private static String string(String text) {
    var encoded = new StringBuilder("\"");
    text.chars().forEach(c -> appendCharacter(encoded, c));
    return encoded.append('"').toString();
  }

  private static void appendCharacter(StringBuilder encoded, int c) {
    switch (c) {
      case '"' -> encoded.append("\\\"");
      case '\\' -> encoded.append("\\\\");
      case '\n' -> encoded.append("\\n");
      case '\r' -> encoded.append("\\r");
      case '\t' -> encoded.append("\\t");
      default -> {
        if (c < 0x20) {
          encoded.append(String.format("\\u%04x", c));
        } else {
          encoded.append((char) c);
        }
      }
    }
  }
}
