package dev.gecode.iris;

import java.io.Serializable;
import java.util.List;
import org.jspecify.annotations.Nullable;

/** The details of an error, in the terms of the Specification's canonical error document. */
public sealed interface IrisError extends Serializable {
  /** Returns the stable code of the error. */
  ErrorCode code();

  /** Returns the main diagnostic text, without a location. */
  String message();

  /** Returns the location in the input, or null if the error has none. */
  @Nullable Location inputLocation();

  /** Returns the location in the reader spec, or null if the error has none. */
  @Nullable Location specLocation();

  /** Returns the execution context, outermost first, or null if the error has none. */
  @Nullable String context();

  /** Returns the indices of a failed check's witness, or null for any other error. */
  @Nullable List<Integer> recordPath();

  /**
   * An error found while parsing a reader spec.
   *
   * @param code the stable code of the error
   * @param message the main diagnostic text
   * @param location the location in the reader spec
   */
  record SpecError(ErrorCode code, String message, Location location) implements IrisError {
    @Override
    public @Nullable Location inputLocation() {
      return null;
    }

    @Override
    public Location specLocation() {
      return location;
    }

    @Override
    public @Nullable String context() {
      return null;
    }

    @Override
    public @Nullable List<Integer> recordPath() {
      return null;
    }
  }
}
