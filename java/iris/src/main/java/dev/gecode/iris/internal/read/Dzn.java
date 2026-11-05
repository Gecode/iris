package dev.gecode.iris.internal.read;

import java.util.Map;
import java.util.Set;
import org.jspecify.annotations.Nullable;

/** The dzn document of data (appendix B). */
public final class Dzn {
  private static final char[] HEX = "0123456789abcdef".toCharArray();
  private static final int MAX_RECTANGULAR_DEPTH = 6;
  private static final ArraySyntax NESTED_ARRAYS = new ArraySyntax("([", "],)", ", ");

  /** The names that need quotes, whatever the host's MiniZinc reserves (§B.2). */
  private static final Set<String> RESERVED_NAMES =
      Set.of(
          "ann",
          "annotation",
          "any",
          "array",
          "bool",
          "case",
          "constraint",
          "default",
          "diff",
          "div",
          "else",
          "elseif",
          "endif",
          "enum",
          "false",
          "float",
          "function",
          "if",
          "in",
          "include",
          "int",
          "intersect",
          "let",
          "list",
          "maximize",
          "minimize",
          "mod",
          "not",
          "of",
          "op",
          "opt",
          "output",
          "par",
          "predicate",
          "record",
          "satisfy",
          "set",
          "solve",
          "string",
          "subset",
          "superset",
          "symdiff",
          "test",
          "then",
          "true",
          "tuple",
          "type",
          "union",
          "var",
          "where",
          "xor");

  private Dzn() {}

  /** Returns the document of {@code entries}. Stored strings are valid, so it has a UTF-8 form. */
  public static String data(Map<String, Stored> entries) {
    StringBuilder out = new StringBuilder();
    entries.forEach((name, value) -> entry(out, name, value));
    return out.toString();
  }

  private static void entry(StringBuilder out, String name, Stored value) {
    if (value instanceof Stored.Array array) {
      int @Nullable [] extents = rectangularExtents(array);
      if (extents == null) {
        typeComment(out, name, array);
        assignmentStart(out, name);
        nested(out, array);
      } else {
        assignmentStart(out, name);
        rectangular(out, array, extents);
      }
    } else {
      assignmentStart(out, name);
      leaf(out, value);
    }
    out.append(";\n");
  }

  private static void assignmentStart(StringBuilder out, String name) {
    if (isPlain(name)) {
      out.append(name);
    } else {
      out.append('\'').append(name).append('\'');
    }
    out.append(" = ");
  }

  private static boolean isPlain(String name) {
    char first = name.charAt(0);
    boolean isLetter = (first >= 'a' && first <= 'z') || (first >= 'A' && first <= 'Z');
    return isLetter && !RESERVED_NAMES.contains(name);
  }

  /**
   * Returns the length of the arrays at each level of {@code array}, or null if it needs the nested
   * form: at some level, not all arrays have the same length, or it has too many levels (§B.5).
   * Only part of the table may be the array, so the walk follows its rows down.
   */
  private static int @Nullable [] rectangularExtents(Stored.Array array) {
    int depth = array.depth();
    if (depth > MAX_RECTANGULAR_DEPTH) {
      return null;
    }
    Table table = array.table();
    int[] extents = new int[depth];
    int from = array.row();
    int to = from + 1;
    for (int k = 0; k < depth; k++) {
      int[] offsets = table.offsets(array.level() + k);
      extents[k] = from < to ? offsets[from + 1] - offsets[from] : 0;
      for (int row = from; row < to; row++) {
        if (offsets[row + 1] - offsets[row] != extents[k]) {
          return null;
        }
      }
      from = offsets[from];
      to = offsets[to];
    }
    return extents;
  }

  /** Writes the array {@code array} of the shape {@code extents} (§B.5). */
  private static void rectangular(StringBuilder out, Stored.Array array, int[] extents) {
    if (extents.length == 1) {
      vector(out, array);
    } else if (extents.length == 2 && extents[0] > 0 && extents[1] > 0) {
      matrix(out, array, extents[1]);
    } else {
      out.append("array").append(extents.length).append("d(");
      for (int extent : extents) {
        out.append("1..").append(extent).append(", ");
      }
      vector(out, array);
      out.append(')');
    }
  }

  /** Writes the leaves of {@code array} as a flat list, whatever its depth. */
  private static void vector(StringBuilder out, Stored.Array array) {
    Table table = array.table();
    out.append('[');
    for (int k = array.firstLeaf(); k < array.endLeaf(); k++) {
      if (k > array.firstLeaf()) {
        out.append(", ");
      }
      leaf(out, table.leaf(k));
    }
    out.append(']');
  }

  private static void matrix(StringBuilder out, Stored.Array array, int columns) {
    Table table = array.table();
    int first = array.firstLeaf();
    out.append("[| ");
    for (int k = first; k < array.endLeaf(); k++) {
      if (k > first) {
        out.append((k - first) % columns == 0 ? "\n  | " : ", ");
      }
      leaf(out, table.leaf(k));
    }
    out.append(" |]");
  }

  /** Writes the type of the nested form of {@code array} (§B.6). */
  private static void typeComment(StringBuilder out, String name, Stored.Array array) {
    int wrappers = array.depth() - 1;
    out.append("% ").append(name).append(": ");
    out.append("array[int] of tuple(".repeat(wrappers));
    out.append("array[int] of ").append(array.type().text());
    out.append(")".repeat(wrappers)).append('\n');
  }

  private static void nested(StringBuilder out, Stored.Array array) {
    Table table = array.table();
    NESTED_ARRAYS.write(out, array, index -> leaf(out, table.leaf(index)));
  }

  private static void leaf(StringBuilder out, Stored value) {
    switch (value) {
      case Stored.Int v -> out.append(v.value());
      case Stored.Real v -> out.append(DznFloat.of(v.value()));
      case Stored.Text v -> string(out, v.value());
      case Stored.Array _ -> throw new IllegalArgumentException("not a leaf");
    }
  }

  /**
   * Writes {@code text}. Only ASCII is escaped, so going by chars is the same as by bytes (§B.4).
   */
  private static void string(StringBuilder out, String text) {
    out.append('"');
    for (int k = 0; k < text.length(); k++) {
      char c = text.charAt(k);
      switch (c) {
        case '"' -> out.append("\\\"");
        case '\\' -> out.append("\\\\");
        case '\n' -> out.append("\\n");
        case '\t' -> out.append("\\t");
        default -> {
          if (c < 0x20) {
            out.append("\\x").append(HEX[c >> 4]).append(HEX[c & 0xf]);
          } else {
            out.append(c);
          }
        }
      }
    }
    out.append('"');
  }
}
