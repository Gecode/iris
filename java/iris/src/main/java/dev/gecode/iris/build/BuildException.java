package dev.gecode.iris.build;

import dev.gecode.iris.IrisException;
import java.io.Serial;
import org.jspecify.annotations.Nullable;

/**
 * Thrown when a reader spec cannot be built: a builder check failed, or the language rejected the
 * emitted text.
 *
 * <p>{@link #kind} names the check. A check that one statement can fail throws at that statement's
 * call; the checks of the whole spec throw from {@link Builder#source} and {@link Builder#build}.
 * Either way {@link #callSite} names the line of the caller's code that made the statement, and
 * {@link #getMessage} ends with it.
 */
public final class BuildException extends RuntimeException {
  @Serial private static final long serialVersionUID = 1L;

  /** The check that failed. */
  public enum Kind {
    /** A statement went to a block that does not take it, such as an arm outside sections. */
    SCOPE,
    /** A handle or type from another builder. */
    PROVENANCE,
    /** Two handles declare one name. */
    REDEFINITION,
    /** Something that has no spelling in Iris text, such as a name that is not an Iris name. */
    UNPRINTABLE,
    /** A definition that does not have its handle's type, depth or presence, or a missing one. */
    BINDING,
    /** The language rejected the emitted text; {@link #failure} gives its error. */
    SPEC
  }

  /** The check that failed. */
  private final Kind kind;

  /** What went wrong, without the call site. */
  private final String reason;

  /** The line of the caller's code that made the statement, if one was found. */
  private final @Nullable StackTraceElement callSite;

  /** The emitted text, for a {@link Kind#SPEC} failure. */
  private final @Nullable String text;

  private BuildException(
      Kind kind,
      String reason,
      @Nullable StackTraceElement callSite,
      @Nullable String text,
      @Nullable IrisException failure) {
    super(message(kind, reason, callSite, text, failure), failure);
    this.kind = kind;
    this.reason = reason;
    this.callSite = callSite;
    this.text = text;
  }

  static BuildException check(Kind kind, String reason, StackTraceElement callSite) {
    return new BuildException(kind, reason, callSite, null, null);
  }

  static BuildException spec(
      IrisException failure, String text, @Nullable StackTraceElement callSite) {
    return new BuildException(Kind.SPEC, failure.error().message(), callSite, text, failure);
  }

  /**
   * {@return the message: the error code for a {@link Kind#SPEC} failure, the reason, the emitted
   * line if there is one, and the call site}
   */
  @Override
  public String getMessage() {
    return java.util.Objects.requireNonNull(super.getMessage());
  }

  /** {@return the check that failed} */
  public Kind kind() {
    return kind;
  }

  /**
   * {@return the reason, without the error code, the emitted line and the call site that {@link
   * #getMessage} adds}
   */
  public String reason() {
    return reason;
  }

  /**
   * {@return the line of the caller's code that made the failing statement, or null if no caller is
   * known}
   */
  public @Nullable StackTraceElement callSite() {
    return callSite;
  }

  /** {@return the emitted text, when the language rejected it, and otherwise null} */
  public @Nullable String text() {
    return text;
  }

  /** {@return the language error, when the language rejected the emitted text, or null} */
  public @Nullable IrisException failure() {
    return getCause() instanceof IrisException failure ? failure : null;
  }

  /** {@return the line of emitted text that the language error points at, or null} */
  public @Nullable String emittedLine() {
    return emittedLine(text, failure());
  }

  private static @Nullable String emittedLine(
      @Nullable String text, @Nullable IrisException failure) {
    Integer line = failure == null ? null : failure.line();
    if (text == null || line == null) {
      return null;
    }
    String[] lines = text.split("\n", -1);
    int count = text.endsWith("\n") ? lines.length - 1 : lines.length;
    if (line < 1 || line > count) {
      return null;
    }
    String found = lines[line - 1];
    return found.endsWith("\r") ? found.substring(0, found.length() - 1) : found;
  }

  private static String message(
      Kind kind,
      String reason,
      @Nullable StackTraceElement callSite,
      @Nullable String text,
      @Nullable IrisException failure) {
    StringBuilder out = new StringBuilder();
    if (kind == Kind.SPEC && failure != null) {
      out.append(failure.code().code()).append(": ");
    }
    out.append(kind == Kind.SPEC ? reason : escapeControls(reason));
    String line = emittedLine(text, failure);
    Integer number = failure == null ? null : failure.line();
    if (line != null && number != null) {
      out.append('\n').append(String.format("%6d", number)).append(" | ");
      out.append(escapeControls(line));
    }
    if (callSite != null) {
      out.append("\ncall site: ").append(escapeControls(callSite.toString()));
    }
    return out.toString();
  }

  /** Escape control characters as the Rust reader does, so that a terminal never acts on them. */
  static String escapeControls(String text) {
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
