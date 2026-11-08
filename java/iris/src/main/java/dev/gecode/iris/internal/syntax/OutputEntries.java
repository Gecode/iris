package dev.gecode.iris.internal.syntax;

import dev.gecode.iris.ValueType;
import dev.gecode.iris.internal.syntax.Ast.Arm;
import dev.gecode.iris.internal.syntax.Ast.Expr;
import dev.gecode.iris.internal.syntax.Ast.Pattern;
import dev.gecode.iris.internal.syntax.Ast.PatternItem;
import dev.gecode.iris.internal.syntax.Ast.Program;
import dev.gecode.iris.internal.syntax.Ast.ReadingType;
import dev.gecode.iris.internal.syntax.Ast.RepeatCount;
import dev.gecode.iris.internal.syntax.Ast.Stmt;
import dev.gecode.iris.internal.syntax.Ast.Terminators;
import dev.gecode.iris.internal.syntax.OutputEntry.Facts;
import dev.gecode.iris.internal.syntax.OutputEntry.Leaf;
import dev.gecode.iris.internal.syntax.OutputEntry.Shape;
import java.util.ArrayList;
import java.util.EnumSet;
import java.util.HashMap;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import org.jspecify.annotations.Nullable;

/**
 * Finds the entries that a successful read of a valid reader spec can leave in its data, with the
 * value types and depths the statements allow and whether an entry can be absent.
 *
 * <p>This ports the output entries of the Rust semantic model ({@code rust/iris-sema}), from which
 * {@code iris gen} prints typed handles, so that both give a spec the same handles. An entry joins
 * the facts of every definition of its name, each shifted by the repeats around it. A definition
 * inside a repeat adds the depth of the column that a repeat without iterations leaves. An entry
 * may be absent when every definition is in an {@code if}, in an optional arm, or in the store of
 * {@code sections}, outside all repeats.
 *
 * <p>Reads, captures and stores have the facts of their type. Lets and parameters take theirs from
 * expressions, whose names have the facts that statement order and branch joins leave them, as in
 * the Rust model. Sections and instances make those facts uncertain, since the input decides which
 * of their definitions run.
 */
public final class OutputEntries {
  private static final Facts INT = Facts.known(ValueType.INT, 0);
  private static final Facts FLOAT = Facts.known(ValueType.FLOAT, 0);
  private static final Facts STRING = Facts.known(ValueType.STRING, 0);
  private static final Set<ValueType> NUMERIC = EnumSet.of(ValueType.INT, ValueType.FLOAT);

  private final Resolution resolution;

  /** Every definition, in source order. */
  private final List<Definition> definitions = new ArrayList<>();

  /** The definitions of each name, except of {@code _}. */
  private final Map<String, List<Definition>> definitionsByName = new HashMap<>();

  /** The definition of each statement and capture that defines a name. */
  private final IdentityHashMap<Object, Definition> definitionOf = new IdentityHashMap<>();

  /** The definitions inside each repeat, arm and instances statement. */
  private final IdentityHashMap<Object, List<Definition>> definitionsInside =
      new IdentityHashMap<>();

  /** The repeats around the statement being collected, outermost first. */
  private final List<Stmt.Repeat> repeats = new ArrayList<>();

  /** How many bodies around that statement, outside all repeats, may not run. */
  private int skippableBodies;

  private final Environment environment = new Environment();

  private OutputEntries(Resolution resolution) {
    this.resolution = resolution;
  }

  /**
   * Returns the entries of a valid reader spec, in the order of their first definitions.
   *
   * @param program the tree of the reader spec
   * @param resolution what its names refer to
   * @return the entries
   */
  public static List<OutputEntry> of(Program program, Resolution resolution) {
    var analysis = new OutputEntries(resolution);
    analysis.collect(program.body());
    analysis.infer(program.body());
    return analysis.entries();
  }

  private enum Kind {
    READ,
    CAPTURE,
    STORE,
    LET,
    PARAM
  }

  /** A definition of a name, which can leave an entry in the data. */
  private static final class Definition {
    final String name;
    final Kind kind;

    /** The repeats around the definition, outermost first. */
    final List<Stmt.Repeat> repeats;

    /** Whether its entry is in the data of every successful read. */
    final boolean required;

    /**
     * What the statements tell about its values: fixed for reads, captures and stores, and found in
     * statement order for lets and parameters.
     */
    Facts facts;

    Definition(String name, Kind kind, List<Stmt.Repeat> repeats, boolean required, Facts facts) {
      this.name = name;
      this.kind = kind;
      this.repeats = repeats;
      this.required = required;
      this.facts = facts;
    }

    /** Whether the definition names a value. A store can keep the token as {@code _}. */
    boolean isOrdinary() {
      return !name.equals("_");
    }

    /** Whether it can leave an entry. No column collects {@code _} inside a repeat (§7.5). */
    boolean reachesOutput() {
      return isOrdinary() || repeats.isEmpty();
    }

    /** The facts of the column of a repeat without iterations, before it is shifted. */
    Facts fallback() {
      if (kind == Kind.LET || kind == Kind.PARAM) {
        return INT;
      }
      Set<ValueType> types = facts.types();
      if (types != null && types.containsAll(NUMERIC)) {
        return new Facts(Set.of(ValueType.INT), facts.depths());
      }
      return facts;
    }

    /** The facts of its values collected over {@code levels} repeats, any of which may be empty. */
    Facts collected(int levels) {
      Facts actual = facts.shifted(levels);
      return levels == 0 ? actual : actual.join(fallback().shifted(levels));
    }

    /**
     * The facts of its values at the top level. A column of a repeat without iterations has no
     * leaves, and no leaves fit every value type, so only the fallback's depth counts.
     */
    Facts entryFacts() {
      Facts collected = collected(repeats.size());
      return new Facts(facts.types(), collected.depths());
    }
  }

  /*
   * Definitions, in source order
   */

  private void collect(List<Stmt> body) {
    for (Stmt statement : body) {
      collect(statement);
    }
  }

  private void collect(Stmt statement) {
    switch (statement) {
      case Stmt.Declaration s ->
          define(s, s.name(), Kind.READ, reading(s.type(), s.dimensions().size()));
      case Stmt.ListRead s -> define(s, s.name(), Kind.READ, reading(s.type(), 1));
      case Stmt.Let s -> define(s, s.name(), Kind.LET, Facts.UNKNOWN);
      case Stmt.Param s -> define(s, s.name(), Kind.PARAM, Facts.UNKNOWN);
      case Stmt.Line s -> collect(s.pattern());
      case Stmt.Find s -> collect(s.pattern());
      case Stmt.Repeat s -> {
        if (s.count() instanceof RepeatCount.Until until) {
          defineStore(s, until.terminators(), skippableBodies == 0);
        }
        int first = definitions.size();
        repeats.add(s);
        collect(s.body());
        repeats.removeLast();
        definitionsInside.put(s, List.copyOf(definitions.subList(first, definitions.size())));
      }
      case Stmt.Sections s -> {
        // Sections can end at the end of input or after a last arm, without a token to store.
        defineStore(s, s.terminators(), !repeats.isEmpty() && skippableBodies == 0);
        for (Arm arm : s.arms()) {
          int first = definitions.size();
          inBody(
              arm.optional(),
              () -> {
                collect(arm.pattern());
                collect(arm.body());
              });
          definitionsInside.put(arm, List.copyOf(definitions.subList(first, definitions.size())));
        }
      }
      case Stmt.If s ->
          inBody(
              true,
              () -> {
                collect(s.body());
                collect(s.orElse());
              });
      case Stmt.Instances s -> {
        // The instance that the read keeps runs its whole body.
        int first = definitions.size();
        collect(s.body());
        definitionsInside.put(s, List.copyOf(definitions.subList(first, definitions.size())));
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

  /**
   * Collects in a body that may not run if {@code skippable}. Inside a repeat that does not count:
   * the repeat publishes a column of every name it may define, whatever runs.
   */
  private void inBody(boolean skippable, Runnable work) {
    boolean counted = skippable && repeats.isEmpty();
    if (counted) {
      skippableBodies++;
    }
    work.run();
    if (counted) {
      skippableBodies--;
    }
  }

  private void collect(Pattern pattern) {
    for (PatternItem item : pattern.items()) {
      if (item instanceof PatternItem.Capture capture && capture.name() != null) {
        define(capture, capture.name(), Kind.CAPTURE, reading(capture.type(), 0));
      }
    }
  }

  /** Defines {@code name}, unless it is {@code _}, which a read or capture uses to keep nothing. */
  private void define(Object owner, String name, Kind kind, Facts facts) {
    if (!name.equals("_")) {
      add(owner, new Definition(name, kind, List.copyOf(repeats), skippableBodies == 0, facts));
    }
  }

  private void defineStore(Object owner, @Nullable Terminators terminators, boolean required) {
    String name = terminators == null ? null : terminators.store();
    if (name != null) {
      add(owner, new Definition(name, Kind.STORE, List.copyOf(repeats), required, STRING));
    }
  }

  private void add(Object owner, Definition definition) {
    definitions.add(definition);
    definitionOf.put(owner, definition);
    if (definition.isOrdinary()) {
      definitionsByName.computeIfAbsent(definition.name, _ -> new ArrayList<>()).add(definition);
    }
  }

  private static Facts reading(ReadingType type, int depth) {
    return switch (type.kind()) {
      case FLOAT -> FLOAT.shifted(depth);
      case WORD, REST -> STRING.shifted(depth);
      case NUM -> new Facts(NUMERIC, Set.of(depth));
      case INT, SYMBOL -> INT.shifted(depth);
    };
  }

  /*
   * Facts of names, in statement order
   */

  private void infer(List<Stmt> body) {
    for (Stmt statement : body) {
      infer(statement);
    }
  }

  private void infer(Stmt statement) {
    switch (statement) {
      case Stmt.Declaration s -> publish(s);
      case Stmt.ListRead s -> publish(s);
      case Stmt.Let s -> publish(s, facts(s.value()));
      case Stmt.Param s -> publish(s, parameter(s.defaultValue()));
      case Stmt.Line s -> publishCaptures(s.pattern());
      case Stmt.Find s -> publishCaptures(s.pattern());
      case Stmt.Repeat s -> inferRepeat(s);
      case Stmt.Sections s -> inferSections(s);
      case Stmt.If s -> join(List.of(branch(s.body()), branch(s.orElse())));
      case Stmt.Instances s -> inferInstances(s);
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

  /** Publishes the facts of the definition of {@code owner}, if it defines a name. */
  private void publish(Object owner) {
    Definition definition = definitionOf.get(owner);
    if (definition != null) {
      environment.put(definition.name, definition.facts);
    }
  }

  private void publish(Object owner, Facts facts) {
    Definition definition = definitionOf.get(owner);
    if (definition != null) {
      definition.facts = facts;
      environment.put(definition.name, facts);
    }
  }

  private void publishCaptures(Pattern pattern) {
    for (PatternItem item : pattern.items()) {
      publish(item);
    }
  }

  /** A parameter is a single value, whether it is set from outside or takes its default. */
  private Facts parameter(@Nullable Expr defaultValue) {
    if (defaultValue == null) {
      return new Facts(EnumSet.allOf(ValueType.class), Set.of(0));
    }
    Facts value = facts(defaultValue);
    return value.join(new Facts(value.types(), Set.of(0)));
  }

  /**
   * The body's own facts end with the repeat. After it, each name defined inside is a column of the
   * values of all its definitions there.
   */
  private void inferRepeat(Stmt.Repeat repeat) {
    int checkpoint = environment.checkpoint();
    infer(repeat.body());
    environment.rollback(checkpoint);
    publish(repeat);
    Map<String, Facts> fields = new LinkedHashMap<>();
    for (Definition definition : inside(repeat)) {
      if (definition.isOrdinary()) {
        int levels = definition.repeats.size() - definition.repeats.indexOf(repeat);
        fields.merge(definition.name, definition.collected(levels), Facts::join);
      }
    }
    fields.forEach(environment::put);
  }

  /**
   * The input decides the order and occurrence of the arms, so every name an arm defines is
   * uncertain throughout.
   */
  private void inferSections(Stmt.Sections sections) {
    for (Arm arm : sections.arms()) {
      admit(inside(arm));
    }
    for (Arm arm : sections.arms()) {
      int checkpoint = environment.checkpoint();
      publishCaptures(arm.pattern());
      infer(arm.body());
      join(List.of(Map.of(), environment.rollback(checkpoint)));
    }
    Definition store = definitionOf.get(sections);
    if (store != null) {
      Facts before = environment.get(store.name);
      environment.put(store.name, before == null ? store.facts : before.join(store.facts));
    }
  }

  /**
   * An instance that the read keeps can publish its names before this one runs, and instances can
   * make every name uncertain (§9.3). There is at most one instances statement.
   */
  private void inferInstances(Stmt.Instances instances) {
    int checkpoint = environment.checkpoint();
    admit(inside(instances));
    infer(instances.body());
    join(List.of(Map.of(), environment.rollback(checkpoint)));
    environment.forgetFacts();
  }

  private List<Definition> inside(Object owner) {
    return Objects.requireNonNull(definitionsInside.get(owner));
  }

  /** Gives the names of {@code possible} values about which nothing is known. */
  private void admit(List<Definition> possible) {
    for (Definition definition : possible) {
      if (definition.isOrdinary()) {
        environment.put(definition.name, Facts.UNKNOWN);
      }
    }
  }

  /** Returns the names that {@code body} changed, with their facts at its end. */
  private Map<String, Facts> branch(List<Stmt> body) {
    int checkpoint = environment.checkpoint();
    infer(body);
    return environment.rollback(checkpoint);
  }

  /** Gives each name that a branch changed the facts of every branch joined. */
  private void join(List<Map<String, Facts>> branches) {
    Set<String> names = new LinkedHashSet<>();
    for (Map<String, Facts> branch : branches) {
      names.addAll(branch.keySet());
    }
    for (String name : names) {
      Facts before = environment.get(name);
      Facts joined = null;
      for (Map<String, Facts> branch : branches) {
        Facts facts = branch.get(name);
        joined = either(joined, facts == null ? before : facts);
      }
      if (joined != null) {
        environment.put(name, joined);
      }
    }
  }

  /** The facts of a name with {@code first} or {@code second}, where null means no value. */
  private static @Nullable Facts either(@Nullable Facts first, @Nullable Facts second) {
    if (first == null) {
      return second;
    }
    return second == null ? first : first.join(second);
  }

  private Facts facts(Expr expression) {
    Facts facts = Facts.UNKNOWN;
    for (Expr node : expression.leftSpine()) {
      facts = factsOver(facts, node);
    }
    return facts;
  }

  /**
   * Returns the facts of {@code node}, whose left operand, if it has one, has the facts {@code
   * left}. Operators and functions other than negation give integers. A comprehension's body gives
   * none of its facts to its result, so it needs no visit.
   */
  private Facts factsOver(Facts left, Expr node) {
    return switch (node) {
      case Expr.IntLiteral _ -> INT;
      case Expr.FloatLiteral _ -> FLOAT;
      case Expr.StringLiteral _ -> STRING;
      case Expr.Name e -> name(e);
      case Expr.Unary e ->
          e.operator() == Ast.UnaryOperator.NEGATE
              ? new Facts(facts(e.operand()).types(), Set.of(0))
              : INT;
      case Expr.Index _ -> left.shifted(-1);
      case Expr.Field e -> field(e);
      case Expr.Binary _, Expr.Call _, Expr.Comprehension _ -> INT;
    };
  }

  /** Records and lists of records are not values, so nothing is known of their facts. */
  private Facts name(Expr.Name name) {
    Facts facts = environment.get(name.name());
    return facts == null || resolution.denotedRepeat(name) != null ? Facts.UNKNOWN : facts;
  }

  /** A field of a record has the values of every definition of its name, collected per record. */
  private Facts field(Expr.Field field) {
    Stmt.Repeat records = resolution.denotedRepeat(field.base());
    if (records == null || resolution.denotedRepeat(field) != null) {
      return Facts.UNKNOWN;
    }
    Facts joined = null;
    for (Definition definition : definitionsByName.getOrDefault(field.name(), List.of())) {
      int levels = definition.repeats.size() - definition.repeats.indexOf(records) - 1;
      joined = either(joined, definition.collected(levels));
    }
    return joined == null ? Facts.UNKNOWN : joined;
  }

  /*
   * Entries
   */

  private List<OutputEntry> entries() {
    Map<String, List<Definition>> byEntry = new LinkedHashMap<>();
    for (Definition definition : definitions) {
      if (definition.reachesOutput()) {
        byEntry.computeIfAbsent(definition.name, _ -> new ArrayList<>()).add(definition);
      }
    }
    List<OutputEntry> entries = new ArrayList<>(byEntry.size());
    byEntry.forEach((name, defined) -> entries.add(entry(name, defined)));
    return List.copyOf(entries);
  }

  private static OutputEntry entry(String name, List<Definition> defined) {
    Facts facts = Facts.UNKNOWN;
    for (int k = 0; k < defined.size(); k++) {
      Facts own = defined.get(k).entryFacts();
      facts = k == 0 ? own : facts.join(own);
    }
    boolean possiblyAbsent = defined.stream().noneMatch(definition -> definition.required);
    return new OutputEntry(name, facts, shape(facts, defined), possiblyAbsent);
  }

  /**
   * A typed handle promises that reading through it cannot fail on the data of a read, so an entry
   * is typed only with one value type and one depth.
   */
  private static Shape shape(Facts facts, List<Definition> defined) {
    Set<Integer> depths = facts.depths();
    Leaf leaf = leaf(facts.types(), defined);
    if (depths == null || depths.size() != 1 || leaf == null) {
      return new Shape.Open();
    }
    return new Shape.Typed(leaf, depths.iterator().next());
  }

  /**
   * Returns the one value type of an entry, or null. Int and float together are {@code num} only
   * when every definition leaves both open: a name redefined from int to float has no one type.
   */
  private static @Nullable Leaf leaf(@Nullable Set<ValueType> types, List<Definition> defined) {
    if (types == null) {
      return null;
    }
    if (types.equals(NUMERIC)) {
      return defined.stream().allMatch(definition -> NUMERIC.equals(definition.facts.types()))
          ? Leaf.NUM
          : null;
    }
    if (types.size() != 1) {
      return null;
    }
    return switch (types.iterator().next()) {
      case INT -> Leaf.INT;
      case FLOAT -> Leaf.FLOAT;
      case STRING -> Leaf.STRING;
    };
  }

  /** The facts of names, with checkpoints that save and restore only the names that change. */
  private static final class Environment {
    private final Map<String, Facts> bindings = new HashMap<>();
    private final List<Change> undo = new ArrayList<>();

    private record Change(String name, @Nullable Facts previous) {}

    @Nullable Facts get(String name) {
      return bindings.get(name);
    }

    void put(String name, Facts facts) {
      undo.add(new Change(name, bindings.put(name, facts)));
    }

    int checkpoint() {
      return undo.size();
    }

    /**
     * Restores the names as they were at {@code checkpoint}, and returns those that changed with
     * their facts before the restore.
     */
    Map<String, Facts> rollback(int checkpoint) {
      Map<String, Facts> changed = new HashMap<>();
      while (undo.size() > checkpoint) {
        Change change = undo.removeLast();
        // The last write is the result; earlier writes only restore what came before.
        changed.putIfAbsent(change.name(), Objects.requireNonNull(bindings.remove(change.name())));
        if (change.previous() != null) {
          bindings.put(change.name(), change.previous());
        }
      }
      return changed;
    }

    void forgetFacts() {
      for (String name : List.copyOf(bindings.keySet())) {
        put(name, Facts.UNKNOWN);
      }
    }
  }
}
