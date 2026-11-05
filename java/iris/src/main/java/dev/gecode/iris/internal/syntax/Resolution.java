package dev.gecode.iris.internal.syntax;

import dev.gecode.iris.ValueType;
import dev.gecode.iris.internal.syntax.Ast.Arm;
import dev.gecode.iris.internal.syntax.Ast.Expr;
import dev.gecode.iris.internal.syntax.Ast.Pattern;
import dev.gecode.iris.internal.syntax.Ast.PatternItem;
import dev.gecode.iris.internal.syntax.Ast.Program;
import dev.gecode.iris.internal.syntax.Ast.ReadingKind;
import dev.gecode.iris.internal.syntax.Ast.ReadingType;
import dev.gecode.iris.internal.syntax.Ast.RepeatCount;
import dev.gecode.iris.internal.syntax.Ast.Stmt;
import dev.gecode.iris.internal.syntax.Ast.Terminators;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.BitSet;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.SequencedMap;
import java.util.Set;
import org.jspecify.annotations.Nullable;

/**
 * What a reading of a reader spec needs to know beyond its tree, all computed once when it is
 * parsed: what its expressions refer to, which names its repeats define, the parameters it declares
 * and the alphabets behind its named types.
 *
 * <p>Expressions are looked up by {@code site} rather than kept in a map, as the {@code hashCode}
 * of an expression recurses through trees of any depth.
 */
public final class Resolution {
  private static final int NO_RECORD = -1;

  private final int[] checkRecordOfSite;
  private final Stmt.@Nullable Repeat[] repeatOfSite;
  private final BitSet isListOfSite = new BitSet();
  private final Stmt.@Nullable Repeat[] elementsOfSite;
  private final List<List<CheckRecord>> checkRecords;
  private final List<List<Declared>> declaredInRepeat;
  private final Set<String> parameters = new LinkedHashSet<>();
  private final ReadingType.@Nullable Alphabet[] alphabets;

  Resolution(Program program) {
    checkRecordOfSite = new int[program.sites()];
    Arrays.fill(checkRecordOfSite, NO_RECORD);
    repeatOfSite = new Stmt.Repeat[program.sites()];
    elementsOfSite = new Stmt.Repeat[program.sites()];
    checkRecords = new ArrayList<>(Collections.nCopies(program.checks(), List.of()));
    declaredInRepeat = new ArrayList<>(Collections.nCopies(program.repeats(), List.of()));
    alphabets = new ReadingType.Alphabet[program.types().size()];
    for (Stmt.TypeDefinition definition : program.types()) {
      alphabets[definition.id()] =
          switch (definition.type()) {
            case ReadingType.Alphabet alphabet -> alphabet;
            case ReadingType.Named named -> alphabets[named.definition()];
            case ReadingType.Base _, ReadingType.Range _ -> null;
          };
    }
    declare(program.body(), new LinkedHashMap<>());
  }

  /**
   * A record name of a check: a name defined inside repeats that are not around the check, or the
   * name of each record of such a repeat.
   *
   * @param name the name of the values
   * @param label the name in messages
   * @param repeats the repeats the check ranges over, outermost first
   */
  public record CheckRecord(String name, String label, List<Stmt.Repeat> repeats) {
    /** Returns the name of the record starts of the innermost of the repeats. */
    public String starts() {
      return startsName(repeats.getLast());
    }
  }

  /**
   * A name that a block may define, with what its values are as far as the statements tell.
   *
   * @param name the name
   * @param type the type of the leaves
   * @param depth the depth
   */
  public record Declared(String name, ValueType type, int depth) {}

  /**
   * Returns the name under which {@code repeat} stores where its records start in the input. It is
   * not a name a reader spec can use, and it is not part of the data read.
   */
  public static String startsName(Stmt.Repeat repeat) {
    return "#" + repeat.id();
  }

  /** Tests whether {@code name} is a name of the implementation, not of the reader spec. */
  public static boolean isInternal(String name) {
    return name.startsWith("#");
  }

  void setCheckRecord(int site, int index) {
    checkRecordOfSite[site] = index;
  }

  void setRecords(int site, Stmt.Repeat repeat, boolean isList) {
    repeatOfSite[site] = repeat;
    isListOfSite.set(site, isList);
  }

  void setElements(int site, Stmt.@Nullable Repeat repeat) {
    elementsOfSite[site] = repeat;
  }

  void setCheckRecords(int check, List<CheckRecord> records) {
    checkRecords.set(check, records);
  }

  /**
   * Returns the repeat that {@code expression} denotes records or a list of records of, or null.
   */
  public Stmt.@Nullable Repeat denotedRepeat(Expr expression) {
    return switch (expression) {
      case Expr.Name e -> repeatOfSite[e.site()];
      case Expr.Field e -> repeatOfSite[e.site()];
      case Expr.Index e -> repeatOfSite[e.site()];
      case Expr.IntLiteral _,
          Expr.FloatLiteral _,
          Expr.StringLiteral _,
          Expr.Unary _,
          Expr.Binary _,
          Expr.Call _,
          Expr.Comprehension _ ->
          null;
    };
  }

  /** Tests whether the name {@code name} denotes a list of records, rather than a record. */
  public boolean isRecordList(Expr.Name name) {
    return isListOfSite.get(name.site());
  }

  /** Returns the index of {@code name} among the record names of its check, or -1. */
  public int checkRecord(Expr.Name name) {
    return checkRecordOfSite[name.site()];
  }

  /** Returns the repeat whose records are the elements of the domain of {@code e}, or null. */
  public Stmt.@Nullable Repeat elements(Expr.Comprehension e) {
    return elementsOfSite[e.site()];
  }

  /** Returns the record names of {@code check}. */
  public List<CheckRecord> checkRecords(Stmt.Check check) {
    return checkRecords.get(check.id());
  }

  /** Returns the names that the body of {@code repeat} may define. */
  public List<Declared> declared(Stmt.Repeat repeat) {
    return declaredInRepeat.get(repeat.id());
  }

  /** Returns the names of the parameters of the reader spec. */
  public Set<String> parameters() {
    return parameters;
  }

  /**
   * Returns the alphabet that the named type {@code named} stands for. This takes no walk along a
   * chain of aliases, which can be of any length.
   */
  public ReadingType.Alphabet alphabet(ReadingType.Named named) {
    ReadingType.Alphabet alphabet = alphabets[named.definition()];
    if (alphabet == null) {
      throw new IllegalArgumentException("the type is not an alphabet");
    }
    return alphabet;
  }

  /*
   * Declared names, each repeat computed once from the repeats inside it
   */

  private void declare(List<Stmt> body, SequencedMap<String, Declared> out) {
    for (Stmt statement : body) {
      declare(statement, out);
    }
  }

  private void declare(Stmt statement, SequencedMap<String, Declared> out) {
    switch (statement) {
      case Stmt.Declaration s -> add(out, s.name(), valueType(s.type()), s.dimensions().size());
      case Stmt.ListRead s -> add(out, s.name(), valueType(s.type()), 1);
      // Usually a single value; the type is only known when running.
      case Stmt.Let s -> add(out, s.name(), ValueType.INT, 0);
      case Stmt.Param s -> {
        parameters.add(s.name());
        add(out, s.name(), ValueType.INT, 0);
      }
      case Stmt.Line s -> declare(s.pattern(), out);
      case Stmt.Find s -> declare(s.pattern(), out);
      case Stmt.Repeat s -> declareRepeat(s, out);
      case Stmt.Sections s -> {
        declare(s.terminators(), out);
        for (Arm arm : s.arms()) {
          declare(arm.pattern(), out);
          declare(arm.body(), out);
        }
      }
      case Stmt.If s -> {
        declare(s.body(), out);
        declare(s.orElse(), out);
      }
      case Stmt.Instances s -> {
        parameters.add("instance");
        declare(s.body(), out);
      }
      case Stmt.Comment _,
          Stmt.Stop _,
          Stmt.Tokens _,
          Stmt.Separators _,
          Stmt.IgnoreTrailing _,
          Stmt.Expect _,
          Stmt.Skip _,
          Stmt.IndexBase _,
          Stmt.Each _,
          Stmt.Check _,
          Stmt.TypeDefinition _ -> {}
    }
  }

  private void declareRepeat(Stmt.Repeat repeat, SequencedMap<String, Declared> out) {
    if (repeat.count() instanceof RepeatCount.Until until) {
      declare(until.terminators(), out);
    }
    add(out, startsName(repeat), ValueType.INT, 1);
    SequencedMap<String, Declared> inner = new LinkedHashMap<>();
    declare(repeat.body(), inner);
    List<Declared> declared = List.copyOf(inner.values());
    declaredInRepeat.set(repeat.id(), declared);
    // The values of a nested repeat are one level deeper.
    for (Declared name : declared) {
      add(out, name.name(), name.type(), name.depth() + 1);
    }
  }

  private static void declare(@Nullable Terminators terminators, Map<String, Declared> out) {
    if (terminators != null) {
      add(out, terminators.store(), ValueType.STRING, 0);
    }
  }

  private static void declare(Pattern pattern, Map<String, Declared> out) {
    for (PatternItem item : pattern.items()) {
      if (item instanceof PatternItem.Capture capture) {
        add(out, capture.name(), valueType(capture.type()), 0);
      }
    }
  }

  private static void add(
      Map<String, Declared> out, @Nullable String name, ValueType type, int depth) {
    if (name != null && !name.equals("_")) {
      out.putIfAbsent(name, new Declared(name, type, depth));
    }
  }

  private static ValueType valueType(ReadingType type) {
    ReadingKind kind = type.kind();
    return switch (kind) {
      case FLOAT -> ValueType.FLOAT;
      case WORD, REST -> ValueType.STRING;
      case INT, NUM, SYMBOL -> ValueType.INT;
    };
  }
}
