package dev.gecode.iris;

import java.io.Serial;
import java.util.List;
import java.util.Objects;
import java.util.stream.Collectors;
import java.util.stream.Stream;
import org.jspecify.annotations.Nullable;

/**
 * Thrown for an error in a reader spec, or in an input that a reader spec reads.
 *
 * <p>{@link #origin}, {@link #line} and {@link #column} give the location that {@link #getMessage}
 * shows: the one in the input if the error has it, and otherwise the one in the reader spec. {@link
 * #error} gives both.
 */
public final class IrisException extends RuntimeException {
  @Serial private static final long serialVersionUID = 1L;

  /** The details of the error. */
  private final IrisError error;

  /**
   * Creates an exception reporting {@code error}.
   *
   * @param error the details of the error
   */
  public IrisException(IrisError error) {
    super(diagnostic(error));
    this.error = error;
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

  private static @Nullable Location location(IrisError error) {
    Location input = error.inputLocation();
    return input == null ? error.specLocation() : input;
  }

  /**
   * Renders {@code origin:line:column: message}, at the location in the input if there is one,
   * without the parts the error lacks (§11.1).
   */
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
