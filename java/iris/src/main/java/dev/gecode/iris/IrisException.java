package dev.gecode.iris;

import java.io.Serial;
import java.util.Objects;
import java.util.stream.Collectors;
import java.util.stream.Stream;
import org.jspecify.annotations.Nullable;

/** Thrown for an error in a reader spec. */
public final class IrisException extends RuntimeException {
  @Serial private static final long serialVersionUID = 1L;

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

  /** Returns the details of the error. */
  public IrisError error() {
    return error;
  }

  /** Returns the stable code of the error. */
  public ErrorCode code() {
    return error.code();
  }

  /** Renders {@code origin:line: message}, without the parts the error lacks (§11.1). */
  private static String diagnostic(IrisError error) {
    @Nullable Location location = error.specLocation();
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
