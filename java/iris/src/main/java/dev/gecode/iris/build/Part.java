package dev.gecode.iris.build;

/**
 * A part of a line pattern other than text, a handle or {@link Iris#discard()}: {@link
 * Iris#capture}, {@link Iris#ref}, {@link Iris#validate}, {@link Iris#blank}, {@link
 * Iris#discard(Type)} or {@link Iris#restOfLine}.
 */
public final class Part {
  final Piece piece;

  Part(Piece piece) {
    this.piece = piece;
  }
}
