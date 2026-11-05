package dev.gecode.iris.internal.read;

import dev.gecode.iris.IrisError;
import dev.gecode.iris.Location;
import java.util.List;
import java.util.Map;
import org.jspecify.annotations.Nullable;

/** The canonical JSON documents: data (§3.3) and errors (§3.7), each with its final newline. */
public final class CanonicalJson {
  private static final char[] HEX = "0123456789abcdef".toCharArray();
  private static final ArraySyntax ARRAYS = new ArraySyntax("[", "]", ",");

  private CanonicalJson() {}

  /** Returns the data document of {@code entries}. */
  public static String data(Map<String, Stored> entries) {
    StringBuilder out = new StringBuilder("{\"iris\":\"0.1\",\"entries\":[");
    boolean isFirst = true;
    for (Map.Entry<String, Stored> entry : entries.entrySet()) {
      out.append(isFirst ? "{\"name\":" : ",{\"name\":");
      isFirst = false;
      Stored value = entry.getValue();
      string(out, entry.getKey());
      out.append(",\"type\":\"").append(value.type().text());
      out.append("\",\"depth\":").append(value.depth()).append(",\"value\":");
      value(out, value);
      out.append('}');
    }
    return out.append("]}\n").toString();
  }

  /** Returns the error document of {@code error}. */
  public static String error(IrisError error) {
    StringBuilder out = new StringBuilder("{\"code\":");
    string(out, error.code().code());
    out.append(",\"message\":");
    string(out, error.message());
    out.append(",\"input_location\":");
    location(out, error.inputLocation());
    out.append(",\"spec_location\":");
    location(out, error.specLocation());
    out.append(",\"context\":");
    nullableString(out, error.context());
    out.append(",\"record_path\":");
    List<Integer> path = error.recordPath();
    if (path == null) {
      out.append("null");
    } else {
      out.append('[');
      for (int k = 0; k < path.size(); k++) {
        out.append(k == 0 ? "" : ",").append(path.get(k));
      }
      out.append(']');
    }
    return out.append("}\n").toString();
  }

  /** Returns {@code array} in JSON, without a final newline. */
  public static String value(Stored.Array array) {
    StringBuilder out = new StringBuilder();
    array(out, array);
    return out.toString();
  }

  private static void value(StringBuilder out, Stored value) {
    switch (value) {
      case Stored.Int v -> out.append(v.value());
      case Stored.Real v -> out.append(CanonicalFloat.of(v.value()));
      case Stored.Text v -> string(out, v.value());
      case Stored.Array v -> array(out, v);
    }
  }

  private static void array(StringBuilder out, Stored.Array array) {
    Table table = array.table();
    ARRAYS.write(out, array, index -> value(out, table.leaf(index)));
  }

  private static void location(StringBuilder out, @Nullable Location location) {
    if (location == null) {
      out.append("null");
      return;
    }
    out.append("{\"origin\":");
    nullableString(out, location.origin());
    out.append(",\"line\":").append(location.line());
    out.append(",\"column\":").append(location.column()).append('}');
  }

  private static void nullableString(StringBuilder out, @Nullable String text) {
    if (text == null) {
      out.append("null");
    } else {
      string(out, text);
    }
  }

  private static void string(StringBuilder out, String text) {
    out.append('"');
    for (int k = 0; k < text.length(); k++) {
      char c = text.charAt(k);
      switch (c) {
        case '"' -> out.append("\\\"");
        case '\\' -> out.append("\\\\");
        case '\b' -> out.append("\\b");
        case '\f' -> out.append("\\f");
        case '\n' -> out.append("\\n");
        case '\r' -> out.append("\\r");
        case '\t' -> out.append("\\t");
        default -> {
          if (c < 0x20) {
            out.append("\\u00").append(HEX[c >> 4]).append(HEX[c & 0xf]);
          } else {
            out.append(c);
          }
        }
      }
    }
    out.append('"');
  }
}
