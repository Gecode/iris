package dev.gecode.iris;

import java.io.Serializable;
import org.jspecify.annotations.Nullable;

/**
 * A position in a reader spec or an input.
 *
 * <p>Lines and columns count from 1. A part that an error does not report is null: reader-spec
 * errors usually have a line and no column, and some errors name only the origin.
 *
 * @param origin the name of the source, or null if it has none
 * @param line the line, or null if not reported
 * @param column the column in bytes, or null if not reported
 */
public record Location(@Nullable String origin, @Nullable Integer line, @Nullable Integer column)
    implements Serializable {}
