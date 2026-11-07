package dev.gecode.iris.internal.access;

import java.util.List;
import org.jspecify.annotations.Nullable;

/**
 * Where the statements of a reader spec from a spec builder came from in the host program: the call
 * site of each line of its text.
 *
 * <p>A spec builder prints one statement per line, so a line names the statement that starts on it,
 * or for a line of {@code }} alone the call that wrote the block. A reader spec parsed from text
 * has none. The table is immutable, so a spec can share it between threads.
 */
public final class CallSites {
  /** No call sites, as for a reader spec parsed from text. */
  public static final CallSites NONE = new CallSites(List.of());

  /** The call site of each line, the first line first. */
  private final List<StackTraceElement> lines;

  /**
   * Creates the table.
   *
   * @param lines the call site of each line of the text, the first line first
   */
  public CallSites(List<StackTraceElement> lines) {
    this.lines = List.copyOf(lines);
  }

  /** {@return the call site of {@code line}, counted from 1, or null if it has none} */
  public @Nullable StackTraceElement at(int line) {
    return line >= 1 && line <= lines.size() ? lines.get(line - 1) : null;
  }

  /**
   * Escapes the control characters of {@code text} as the Rust reader does, so that a terminal
   * never acts on them: emitted strings keep some control characters raw, and a file name in a call
   * site can hold them too.
   */
  public static String escapeControls(String text) {
    StringBuilder out = new StringBuilder(text.length());
    text.codePoints()
        .forEach(c -> {
          if (Character.getType(c) != Character.CONTROL) {
            out.appendCodePoint(c);
          } else if (c == '\t') {
            out.append("\\t");
          } else if (c == '\r') {
            out.append("\\r");
          } else if (c == '\n') {
            out.append("\\n");
          } else {
            out.append("\\u{").append(Integer.toHexString(c)).append('}');
          }
        });
    return out.toString();
  }
}
