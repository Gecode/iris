package dev.gecode.iris.internal.syntax;

import dev.gecode.iris.ErrorCode;
import dev.gecode.iris.IrisError;
import dev.gecode.iris.IrisException;
import dev.gecode.iris.Location;
import org.jspecify.annotations.Nullable;

/** Creates the exceptions of parse-time errors. */
public final class SpecErrors {
  private SpecErrors() {}

  /** Returns the error {@code code} on {@code line}, which has no column (§11.3.2). */
  static IrisException at(ErrorCode code, @Nullable String origin, int line, String message) {
    return new IrisException(
        new IrisError.SpecError(code, message, new Location(origin, line, null)));
  }

  /** Returns {@code E-SPEC-UTF8} at byte {@code offset} of {@code source} (§5.1). */
  public static IrisException invalidUtf8(byte[] source, int offset, @Nullable String origin) {
    Location location =
        new Location(origin, Utf8.line(source, offset), Utf8.column(source, offset));
    return new IrisException(
        new IrisError.SpecError(ErrorCode.SPEC_UTF8, "reader spec is not valid UTF-8", location));
  }
}
