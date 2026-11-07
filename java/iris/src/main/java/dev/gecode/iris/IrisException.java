package dev.gecode.iris;

import dev.gecode.iris.internal.access.CallSites;
import java.io.Serial;
import java.util.List;
import java.util.Objects;
import java.util.Optional;
import java.util.stream.Collectors;
import java.util.stream.Stream;
import org.jspecify.annotations.Nullable;

/**
 * Thrown for an error in a reader spec, or in an input that a reader spec reads.
 *
 * <p>{@link #origin}, {@link #line} and {@link #column} give the location that {@link #getMessage}
 * shows: the one in the input if the error has it, and otherwise the one in the reader spec. {@link
 * #error} gives both.
 *
 * <p>An error from a reader spec that a spec builder made also names the line of the host program
 * that wrote the statement the error belongs to: {@link #callSite} returns it, and {@link
 * #getMessage} ends with it.
 */
public final class IrisException extends RuntimeException {
  @Serial private static final long serialVersionUID = 1L;

  /** The details of the error. */
  private final IrisError error;

  /**
   * The reader-spec line of the statement that the error belongs to, or 0 if the error records
   * none. It is recorded where the error's own line can lie past its statement: a failure at the
   * end of a block, or one noticed while reading a later statement.
   */
  private final int ownerLine;

  /** The host code that wrote the statement, for a reader spec from a spec builder. */
  private final @Nullable StackTraceElement callSite;

  /**
   * Creates an exception reporting {@code error}.
   *
   * @param error the details of the error
   */
  public IrisException(IrisError error) {
    this(error, 0, null);
  }

  private IrisException(IrisError error, int ownerLine, @Nullable StackTraceElement callSite) {
    super(diagnostic(error, callSite));
    this.error = error;
    this.ownerLine = ownerLine;
    this.callSite = callSite;
  }

  /**
   * Returns an exception reporting {@code error}, which belongs to the statement on {@code
   * ownerLine}.
   */
  static IrisException owned(IrisError error, int ownerLine) {
    return new IrisException(error, ownerLine, null);
  }

  /**
   * {@return the reader-spec line of the statement that the error belongs to, or 0 if it records
   * none}
   */
  int ownerLine() {
    return ownerLine;
  }

  /**
   * Returns this exception with the call site from {@code sites} of the statement it belongs to:
   * the one it records, or else the one on its reader-spec line. The code, the message and the
   * locations stay as they are, and so does the stack trace.
   */
  IrisException located(CallSites sites) {
    Location spec = error.specLocation();
    Integer specLine = spec == null ? null : spec.line();
    int line = ownerLine > 0 ? ownerLine : specLine == null ? 0 : specLine;
    StackTraceElement site = sites.at(line);
    if (site == null) {
      return this;
    }
    IrisException located = new IrisException(error, ownerLine, site);
    located.setStackTrace(getStackTrace());
    return located;
  }

  /** {@return the details of the error} */
  public IrisError error() {
    return error;
  }

  /** {@return the stable code of the error} */
  public ErrorCode code() {
    return error.code();
  }

  /** {@return the name of the source, or null if the error does not report one} */
  public @Nullable String origin() {
    Location location = location(error);
    return location == null ? null : location.origin();
  }

  /** {@return the line, counted from 1, or null if the error does not report one} */
  public @Nullable Integer line() {
    Location location = location(error);
    return location == null ? null : location.line();
  }

  /** {@return the column in bytes, counted from 1, or null if the error does not report one} */
  public @Nullable Integer column() {
    Location location = location(error);
    return location == null ? null : location.column();
  }

  /** {@return the indices of a failed check's witness, or null for any other error} */
  public @Nullable List<Integer> path() {
    return error.recordPath();
  }

  /**
   * {@return the line of the host program that wrote the statement the error belongs to, if the
   * reader spec comes from a spec builder}
   *
   * <p>It is the statement's {@link dev.gecode.iris.build.BuildException#callSite}. A reader spec
   * parsed from text has no call sites, and neither has an error that belongs to no statement, such
   * as unexpected input after the end of the spec.
   */
  public Optional<StackTraceElement> callSite() {
    return Optional.ofNullable(callSite);
  }

  private static @Nullable Location location(IrisError error) {
    Location input = error.inputLocation();
    return input == null ? error.specLocation() : input;
  }

  /**
   * Renders {@code origin:line:column: message}, at the location in the input if there is one,
   * without the parts the error lacks (§11.1), and then {@code call site: } and the call site on a
   * line of its own if there is one.
   */
  private static String diagnostic(IrisError error, @Nullable StackTraceElement callSite) {
    String diagnostic = diagnostic(error);
    return callSite == null
        ? diagnostic
        : diagnostic + "\ncall site: " + CallSites.escapeControls(callSite.toString());
  }

  private static String diagnostic(IrisError error) {
    Location location = location(error);
    if (location == null) {
      return error.message();
    }
    String position =
        Stream.of(location.origin(), location.line(), location.column())
            .filter(Objects::nonNull)
            .map(String::valueOf)
            .collect(Collectors.joining(":"));
    return position.isEmpty() ? error.message() : position + ": " + error.message();
  }
}
