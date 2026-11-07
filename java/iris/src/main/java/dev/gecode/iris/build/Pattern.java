package dev.gecode.iris.build;

import dev.gecode.iris.Field;
import java.util.ArrayList;
import java.util.List;

/**
 * A line pattern, from {@link Iris#pattern}: text, with parts that capture, match or skip what
 * stands there.
 */
public final class Pattern {
  final List<Piece> pieces;

  private Pattern(List<Piece> pieces) {
    this.pieces = pieces;
  }

  /** The pattern of {@code parts}, with adjacent text joined. */
  static Pattern of(Object... parts) {
    List<Piece> pieces = new ArrayList<>();
    for (Object part : parts) {
      if (part instanceof Pattern pattern) {
        pattern.pieces.forEach(piece -> add(pieces, piece));
      } else {
        add(pieces, piece(part));
      }
    }
    return new Pattern(List.copyOf(pieces));
  }

  private static void add(List<Piece> pieces, Piece piece) {
    if (piece instanceof Piece.Text text) {
      if (text.text().isEmpty()) {
        return;
      }
      if (!pieces.isEmpty() && pieces.getLast() instanceof Piece.Text before) {
        pieces.set(pieces.size() - 1, new Piece.Text(before.text() + text.text()));
        return;
      }
    }
    pieces.add(piece);
  }

  private static Piece piece(Object part) {
    return switch (part) {
      case String text -> new Piece.Text(text);
      case Field<?> field -> new Piece.Bind(Exprs.handle(field), Tag.STR, null);
      case Part p -> p.piece;
      case Discard d -> new Piece.DiscardPiece();
      default ->
          throw new IllegalArgumentException(
              "a pattern part is text, a handle, a part such as capture(x, intType()), or"
                  + " discard(), not "
                  + part.getClass().getName());
    };
  }
}
