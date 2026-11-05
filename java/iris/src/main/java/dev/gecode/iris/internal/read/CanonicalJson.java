package dev.gecode.iris.internal.read;

import dev.gecode.iris.IrisError;
import dev.gecode.iris.Location;
import java.util.List;
import java.util.Map;
import org.jspecify.annotations.Nullable;

/** The canonical JSON documents: data (§3.3) and errors (§3.7), each with its final newline. */
public final class CanonicalJson {
  private static final char[] HEX = "0123456789abcdef".toCharArray();

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

  /**
   * Writes {@code array} with a cursor for each level. Any number of dimensions is legal (§5.12),
   * so the walk does not recurse.
   */
  private static void array(StringBuilder out, Stored.Array array) {
    Table table = array.table();
    int depth = table.depth();
    int outermost = array.level();
    int[] start = new int[depth];
    int[] cursor = new int[depth];
    int[] end = new int[depth];
    int level = outermost;
    open(table, level, array.row(), start, cursor, end);
    out.append('[');
    while (true) {
      if (cursor[level] == end[level]) {
        out.append(']');
        if (level == outermost) {
          return;
        }
        level--;
        continue;
      }
      if (cursor[level] != start[level]) {
        out.append(',');
      }
      int child = cursor[level]++;
      if (level + 1 == depth) {
        value(out, table.leaf(child));
      } else {
        level++;
        open(table, level, child, start, cursor, end);
        out.append('[');
      }
    }
  }

  private static void open(Table table, int level, int row, int[] start, int[] cursor, int[] end) {
    int[] offsets = table.offsets(level);
    start[level] = offsets[row];
    cursor[level] = offsets[row];
    end[level] = offsets[row + 1];
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
