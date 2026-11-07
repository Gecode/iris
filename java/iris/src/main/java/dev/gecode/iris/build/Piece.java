package dev.gecode.iris.build;

import org.jspecify.annotations.Nullable;

/** A piece of a line pattern. */
sealed interface Piece {
  record Text(String text) implements Piece {}

  /** {@code {x:type}}, or {@code {x}} without a type, which reads a word. */
  record Bind(Handle handle, Tag value, @Nullable Declared capture) implements Piece {}

  /** {@code {$x}}. */
  record Reference(Handle handle) implements Piece {}

  /** {@code {:type}}. */
  record Validate(Declared capture) implements Piece {}

  /** {@code {}}. */
  record Blank() implements Piece {}

  /** {@code {_}}. */
  record DiscardPiece() implements Piece {}

  /** {@code {_:type}}. */
  record DiscardAs(Declared capture) implements Piece {}

  /** The trailing {@code ...}. */
  record RestOfLine() implements Piece {}
}
