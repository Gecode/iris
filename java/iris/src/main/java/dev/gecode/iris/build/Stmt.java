package dev.gecode.iris.build;

import java.util.ArrayList;
import java.util.List;
import org.jspecify.annotations.Nullable;

/**
 * A statement. The statements form one flat list in text order, with blocks marked by openers and
 * {@link Close}, so that every pass is a loop over it.
 */
sealed interface Stmt {
  /**
   * What a definition defines.
   *
   * @param name the name, or null for the discard {@code _}
   * @param value the value type
   * @param levels the definition's own array levels, or null when they are open
   */
  record Definition(@Nullable String name, Tag value, @Nullable Integer levels) {}

  /** How a repeat counts. */
  sealed interface RepeatCount {}

  record Times(List<Op> count) implements RepeatCount {}

  record UntilEof() implements RepeatCount {}

  /** A clause of {@code sections}. */
  sealed interface Clause {}

  /** {@code until "text"... store x}, of a repeat or of sections. */
  record Terminated(List<String> terminators, @Nullable Definition store)
      implements RepeatCount, Clause {}

  record SkipOtherClause() implements Clause {}

  record Format(String name) implements Stmt {}

  /** {@code comment}, {@code stop}, {@code tokens}, {@code separators} or {@code expect}. */
  record Strings(String keyword, List<String> strings) implements Stmt {}

  record IgnoreTrailing() implements Stmt {}

  record Param(Definition definition, @Nullable List<Op> defaultValue) implements Stmt {}

  record Declaration(Definition definition, Declared declared) implements Stmt {}

  /** {@code list x: type until end}, with a null end for {@code until eol}. */
  record ListStmt(Definition definition, Declared declared, @Nullable List<Op> end)
      implements Stmt {}

  record TypeDefinition(String name, Declared declared) implements Stmt {}

  /** {@code line} or {@code find}. */
  record Line(String keyword, List<Piece> pieces) implements Stmt {}

  record SkipLine() implements Stmt {}

  record SkipLines(List<Op> count) implements Stmt {}

  record Index(List<Op> position) implements Stmt {}

  record Let(Definition definition, List<Op> value) implements Stmt {}

  record Check(List<Op> condition, @Nullable String message) implements Stmt {}

  record Repeat(RepeatCount count, int record) implements Stmt {}

  record Sections(List<Clause> clauses) implements Stmt {}

  record Arm(List<Piece> pieces, boolean optional, boolean last, boolean body) implements Stmt {}

  record If(List<Op> condition) implements Stmt {}

  record ElseIf(List<Op> condition) implements Stmt {}

  record Else() implements Stmt {}

  /** {@code else if:type}, a declaration named {@code if} as the else branch. */
  record ElseDeclaration(Definition definition, Declared declared) implements Stmt {}

  record Instances() implements Stmt {}

  record Close() implements Stmt {}

  /** Whether a statement starts a block that {@link Close} ends. */
  static boolean opensBlock(Stmt stmt) {
    return switch (stmt) {
      case Arm arm -> arm.body;
      case Repeat r -> true;
      case Sections s -> true;
      case If i -> true;
      case ElseIf e -> true;
      case Else e -> true;
      case Instances i -> true;
      default -> false;
    };
  }

  /** Whether a statement continues the line of the preceding {@code }}. */
  static boolean continuesIf(Stmt stmt) {
    return stmt instanceof ElseIf || stmt instanceof Else || stmt instanceof ElseDeclaration;
  }

  /** Every expression, handle and named type a statement mentions. */
  static List<Mention> mentions(Stmt stmt) {
    List<Mention> out = new ArrayList<>();
    switch (stmt) {
      case Param p -> {
        if (p.defaultValue != null) {
          out.add(new Mention.Expression(p.defaultValue));
        }
      }
      case Declaration d -> d.declared.mentions(out);
      case TypeDefinition t -> t.declared.mentions(out);
      case ElseDeclaration e -> e.declared.mentions(out);
      case ListStmt l -> {
        l.declared.mentions(out);
        if (l.end != null) {
          out.add(new Mention.Expression(l.end));
        }
      }
      case Line l -> patternMentions(l.pieces, out);
      case Arm a -> patternMentions(a.pieces, out);
      case If i -> out.add(new Mention.Expression(i.condition));
      case ElseIf e -> out.add(new Mention.Expression(e.condition));
      case Check c -> out.add(new Mention.Expression(c.condition));
      case Let l -> out.add(new Mention.Expression(l.value));
      case SkipLines s -> out.add(new Mention.Expression(s.count));
      case Index i -> out.add(new Mention.Expression(i.position));
      case Repeat r -> {
        if (r.count instanceof Times times) {
          out.add(new Mention.Expression(times.count));
        }
      }
      default -> {}
    }
    return out;
  }

  private static void patternMentions(List<Piece> pieces, List<Mention> out) {
    for (Piece piece : pieces) {
      switch (piece) {
        case Piece.Bind bind -> {
          out.add(new Mention.HandleUse(bind.handle()));
          if (bind.capture() != null) {
            bind.capture().mentions(out);
          }
        }
        case Piece.Reference reference -> out.add(new Mention.HandleUse(reference.handle()));
        case Piece.Validate validate -> validate.capture().mentions(out);
        case Piece.DiscardAs discard -> discard.capture().mentions(out);
        default -> {}
      }
    }
  }
}
