package dev.gecode.iris.internal.read;

import dev.gecode.iris.ErrorCode;
import java.io.Serial;

/** An error about the shape or type of a value, which the catcher completes and locates. */
final class ShapeError extends RuntimeException {
  @Serial private static final long serialVersionUID = 1L;

  private final ErrorCode code;
  private final String phrase;

  ShapeError(ErrorCode code, String phrase) {
    super(phrase, null, false, false);
    this.code = code;
    this.phrase = phrase;
  }

  ErrorCode code() {
    return code;
  }

  /** Returns the description of the error, which the catcher completes with a name. */
  String phrase() {
    return phrase;
  }
}
