package dev.gecode.iris.internal.syntax;

import java.util.ArrayList;
import java.util.List;
import org.jspecify.annotations.Nullable;

/**
 * The syntax tree of a reader spec.
 *
 * <p>Statements carry their reader-spec line, which is the line that every later diagnostic
 * reports. Expressions, types and patterns need none of their own.
 */
public final class Ast {
  private Ast() {}

  /**
   * A whole reader spec.
   *
   * @param format the format name, or null if the spec has none
   * @param body the statements
   * @param bindings the comprehension bindings, in the order the comprehensions were completed
   * @param types the named types, in definition order
   * @param sites how many record-bearing expressions there are (see {@link Expr})
   * @param checks how many check statements there are
   * @param repeats how many repeat statements there are
   */
  public record Program(
      @Nullable String format,
      List<Stmt> body,
      List<Binding> bindings,
      List<Stmt.TypeDefinition> types,
      int sites,
      int checks,
      int repeats) {}

  /**
   * A name bound by a comprehension.
   *
   * @param name the name
   * @param line the line of the name
   */
  public record Binding(String name, int line) {}

  /**
   * An expression.
   *
   * <p>The left operands of a tree can be far deeper than any stack. §5.12 releases the entries of
   * a parenthesized left operand, so {@code ((1+1)+1)+1} can continue for tens of thousands of
   * operators. Every other operand holds an entry, so at most 256 of them lie on any path through
   * the tree. A walk therefore loops over {@link #leftSpine} and recurses into the other operands.
   * The generated {@code equals}, {@code hashCode} and {@code toString} recurse into both, so they
   * are not for the trees of arbitrary reader specs.
   *
   * <p>Names, indices, fields and comprehensions can denote or range over records. They carry a
   * {@code site}, a number unique in the reader spec, by which {@link Resolution} says what each of
   * them resolved to.
   */
  public sealed interface Expr {
    /** Returns this expression and its chain of left operands, the leftmost first. */
    default List<Expr> leftSpine() {
      List<Expr> spine = new ArrayList<>();
      for (Expr expression = this; expression != null; expression = leftOperand(expression)) {
        spine.add(expression);
      }
      return spine.reversed();
    }

    private static @Nullable Expr leftOperand(Expr expression) {
      return switch (expression) {
        case Binary e -> e.left();
        case Index e -> e.base();
        case Field e -> e.base();
        case IntLiteral _,
            FloatLiteral _,
            StringLiteral _,
            Name _,
            Unary _,
            Call _,
            Comprehension _ ->
            null;
      };
    }

    /** An integer literal, or a host constant. */
    record IntLiteral(long value) implements Expr {}

    /** A float literal. */
    record FloatLiteral(double value) implements Expr {}

    /** A string literal. */
    record StringLiteral(String value) implements Expr {}

    /** A name. */
    record Name(String name, int site) implements Expr {}

    /** A unary operator applied to its operand. */
    record Unary(UnaryOperator operator, Expr operand) implements Expr {}

    /** A binary operator applied to its operands. */
    record Binary(BinaryOperator operator, Expr left, Expr right) implements Expr {}

    /** An ordinary function call. */
    record Call(Function function, List<Expr> arguments) implements Expr {}

    /** An index expression {@code base[index]}. */
    record Index(Expr base, Expr index, int site) implements Expr {}

    /** A field selection {@code base.name}. */
    record Field(Expr base, String name, int site) implements Expr {}

    /** A comprehension {@code function(binding in domain: body)}. */
    record Comprehension(
        ComprehensionFunction function, String binding, Expr domain, Expr body, int site)
        implements Expr {}
  }

  /** The unary operators. */
  public enum UnaryOperator {
    NEGATE,
    NOT
  }

  /** The binary operators. */
  public enum BinaryOperator {
    OR,
    AND,
    EQUAL,
    NOT_EQUAL,
    LESS,
    GREATER,
    LESS_EQUAL,
    GREATER_EQUAL,
    ADD,
    SUBTRACT,
    MULTIPLY,
    DIVIDE,
    REMAINDER,
    POWER
  }

  /** The functions of ordinary calls. */
  public enum Function {
    DEFINED,
    COUNT,
    SIZE,
    MIN,
    MAX,
    SUM,
    DISTINCT
  }

  /** The functions of comprehensions. */
  public enum ComprehensionFunction {
    ALL,
    ANY,
    SUM,
    COUNT
  }

  /** What a declaration or a capture reads. */
  public enum ReadingKind {
    INT,
    FLOAT,
    NUM,
    WORD,
    REST,
    SYMBOL;

    /** Tests whether this kind reads numbers. */
    public boolean isNumeric() {
      return this == INT || this == FLOAT || this == NUM;
    }
  }

  /** A description of how to read and validate a value. */
  public sealed interface ReadingType {
    /** Returns what the type reads. */
    ReadingKind kind();

    /** A base type. */
    record Base(ReadingKind kind) implements ReadingType {}

    /**
     * An integer range.
     *
     * @param lower the lower bound, or null if it is open
     * @param upper the upper bound, or null if it is open
     * @param exclusive whether the upper bound is excluded
     */
    record Range(@Nullable Expr lower, @Nullable Expr upper, boolean exclusive)
        implements ReadingType {
      @Override
      public ReadingKind kind() {
        return ReadingKind.INT;
      }
    }

    /**
     * An alphabet.
     *
     * @param symbols the symbols
     * @param blank the symbols that stand for 0
     * @param strip the symbols to skip
     * @param base the value of the first symbol
     */
    record Alphabet(String symbols, String blank, String strip, long base) implements ReadingType {
      @Override
      public ReadingKind kind() {
        return ReadingKind.SYMBOL;
      }
    }

    /**
     * A use of a named type. It refers to the definition by number and keeps the kind, as aliases
     * can form chains of any length, which no operation on a type may have to follow.
     *
     * @param definition the {@link Stmt.TypeDefinition#id} of the definition
     * @param kind what the definition reads
     */
    record Named(int definition, ReadingKind kind) implements ReadingType {}
  }

  /**
   * A line pattern such as {@code "DIMENSION : {n:int}"}.
   *
   * @param source the decoded text of the pattern
   * @param items the items
   */
  public record Pattern(String source, List<PatternItem> items) {}

  /** An item of a line pattern. */
  public sealed interface PatternItem {
    /** Literal text. */
    record Literal(String text) implements PatternItem {}

    /**
     * A run of blanks.
     *
     * @param required whether the input needs at least one blank here
     */
    record Space(boolean required) implements PatternItem {}

    /**
     * A capture.
     *
     * @param name the name it defines, or null if it is unnamed or discarded
     * @param type what it reads
     */
    record Capture(@Nullable String name, ReadingType type) implements PatternItem {}

    /** A reference {@code {$name}} to the current value of a name. */
    record Reference(String name) implements PatternItem {}

    /** A terminal {@code ...}, which accepts any remaining text. */
    record Rest() implements PatternItem {}
  }

  /**
   * The tokens that end a repeat or sections statement.
   *
   * @param tokens the terminating tokens
   * @param store the name for the token found, or null if it is not kept
   */
  public record Terminators(List<String> tokens, @Nullable String store) {}

  /** How often a repeat runs. */
  public sealed interface RepeatCount {
    /** A count of iterations. */
    record Times(Expr count) implements RepeatCount {}

    /** Until the end of input. */
    record UntilEof() implements RepeatCount {}

    /** Until a terminating token. */
    record Until(Terminators terminators) implements RepeatCount {}
  }

  /**
   * An arm of a sections statement.
   *
   * @param line the line of its pattern in the reader spec, which owns the errors of the arm
   * @param pattern the line pattern
   * @param optional whether the input may lack a line matching the arm
   * @param last whether the sections statement ends after this arm
   * @param body the statements run after the line matched
   */
  public record Arm(int line, Pattern pattern, boolean optional, boolean last, List<Stmt> body) {}

  /** A statement. */
  public sealed interface Stmt {
    /** Returns the line of the statement in the reader spec. */
    int line();

    /** {@code comment "prefix"...}. */
    record Comment(int line, List<String> prefixes) implements Stmt {}

    /** {@code stop "prefix"...}. */
    record Stop(int line, List<String> prefixes) implements Stmt {}

    /** {@code tokens "characters"}. */
    record Tokens(int line, String characters) implements Stmt {}

    /** {@code separators "characters"}. */
    record Separators(int line, String characters) implements Stmt {}

    /** {@code ignore trailing}. */
    record IgnoreTrailing(int line) implements Stmt {}

    /** {@code expect "token"...}. */
    record Expect(int line, List<String> tokens) implements Stmt {}

    /** {@code param name [= default]}. */
    record Param(int line, String name, @Nullable Expr defaultValue) implements Stmt {}

    /** A declaration {@code name[dimension]...: type [layout e] [offset e]}. */
    record Declaration(
        int line,
        String name,
        List<Expr> dimensions,
        ReadingType type,
        @Nullable Expr layout,
        @Nullable Expr offset)
        implements Stmt {}

    /**
     * {@code list name: type until (sentinel | eol) [offset e]}.
     *
     * @param sentinel the value that ends the list, or null if the end of line does
     */
    record ListRead(
        int line, String name, ReadingType type, @Nullable Expr sentinel, @Nullable Expr offset)
        implements Stmt {}

    /** {@code line "pattern"}. */
    record Line(int line, Pattern pattern) implements Stmt {}

    /** {@code find "pattern"}. */
    record Find(int line, Pattern pattern) implements Stmt {}

    /** {@code skip line}, with a count of 1, or {@code skip count lines}. */
    record Skip(int line, Expr count) implements Stmt {}

    /** {@code index base}. */
    record IndexBase(int line, Expr base) implements Stmt {}

    /**
     * {@code repeat count [as list] { ... }}.
     *
     * @param id the number of the repeat in source order. It makes two repeats equal only if they
     *     are the same statement, as repeat identity decides name resolution (§8.12).
     * @param line the line of the statement
     * @param count how often the repeat runs
     * @param list the name of the list of records, or null if it has none
     * @param each the name of each record, or null if it has none
     * @param body the statements, starting with any {@link Each}
     */
    record Repeat(
        int id,
        int line,
        RepeatCount count,
        @Nullable String list,
        @Nullable String each,
        List<Stmt> body)
        implements Stmt {
      /** Compares ids alone: a count's tree may be too deep for generated record equality. */
      @Override
      public boolean equals(@Nullable Object other) {
        return other instanceof Repeat repeat && repeat.id == id;
      }

      @Override
      public int hashCode() {
        return id;
      }
    }

    /** {@code each name}, first in the body of a repeat. */
    record Each(int line, String name) implements Stmt {}

    /**
     * {@code sections [until ...] [skip other] { arms }}.
     *
     * @param line the line of the statement
     * @param terminators the tokens that end the statement, or null if only the end of input does
     * @param skipOther whether unknown lines are skipped
     * @param arms the arms
     */
    record Sections(int line, @Nullable Terminators terminators, boolean skipOther, List<Arm> arms)
        implements Stmt {}

    /** {@code if condition { ... } [else ...]}. */
    record If(int line, Expr condition, List<Stmt> body, List<Stmt> orElse) implements Stmt {}

    /** {@code let name = value}. */
    record Let(int line, String name, Expr value) implements Stmt {}

    /**
     * {@code check condition ["message"]}.
     *
     * @param id the number of the check in source order
     * @param line the line of the statement
     * @param condition what must hold
     * @param message the text for a failure, or null if it has none
     */
    record Check(int id, int line, Expr condition, @Nullable String message) implements Stmt {}

    /**
     * {@code type name = type}.
     *
     * @param id the index of the definition in {@link Program#types}
     * @param line the line of the statement
     * @param name the name
     * @param type what the name stands for
     */
    record TypeDefinition(int id, int line, String name, ReadingType type) implements Stmt {}

    /** {@code instances { ... }}. */
    record Instances(int line, List<Stmt> body) implements Stmt {}
  }
}
