package dev.gecode.iris.internal.syntax;

import dev.gecode.iris.ErrorCode;
import dev.gecode.iris.IrisException;
import dev.gecode.iris.internal.syntax.Ast.Arm;
import dev.gecode.iris.internal.syntax.Ast.Binding;
import dev.gecode.iris.internal.syntax.Ast.Expr;
import dev.gecode.iris.internal.syntax.Ast.Function;
import dev.gecode.iris.internal.syntax.Ast.Pattern;
import dev.gecode.iris.internal.syntax.Ast.PatternItem;
import dev.gecode.iris.internal.syntax.Ast.Program;
import dev.gecode.iris.internal.syntax.Ast.ReadingType;
import dev.gecode.iris.internal.syntax.Ast.RepeatCount;
import dev.gecode.iris.internal.syntax.Ast.Stmt;
import dev.gecode.iris.internal.syntax.Ast.Terminators;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import org.jspecify.annotations.Nullable;

/**
 * Applies the parse-time checks that need the whole spec, in the order of §11.2: comprehension
 * bindings against values, named types against values, the names of records, and then the
 * expressions of the statements (§8.12).
 */
public final class Resolver {
  private final @Nullable String origin;

  /** All definitions, in source order, but never of {@code _}. */
  private final List<Definition> definitions = new ArrayList<>();

  /** The same definitions by name. */
  private final Map<String, List<Definition>> definitionsByName = new HashMap<>();

  /** All repeats that name their records, outer before inner. */
  private final List<NamedRepeat> namedRepeats = new ArrayList<>();

  private final Map<String, NamedRepeat> lists = new HashMap<>();
  private final Map<String, NamedRepeat> eachNames = new HashMap<>();

  /** The repeats around the statement visited, outermost first. */
  private final List<Stmt.Repeat> repeats = new ArrayList<>();

  /** The names bound by the comprehensions around the expression, innermost last. */
  private final List<Bound> bound = new ArrayList<>();

  /** The record names of the check visited. */
  private final List<CheckRecord> checkRecords = new ArrayList<>();

  private int line = 0;
  private boolean inCheck = false;

  private Resolver(@Nullable String origin) {
    this.origin = origin;
  }

  /** Checks {@code program}, the parsed reader spec from {@code origin}. */
  public static void check(Program program, @Nullable String origin) {
    new Resolver(origin).run(program);
  }

  /** A definition of a name, with the repeats around it, outermost first. */
  private record Definition(String name, List<Stmt.Repeat> repeats) {}

  /** A repeat that names its records, with the repeats around it, outermost first. */
  private record NamedRepeat(Stmt.Repeat repeat, List<Stmt.Repeat> repeats) {}

  /** A name bound by a comprehension, with the repeat whose records it stands for, if any. */
  private record Bound(String name, Stmt.@Nullable Repeat records) {}

  /**
   * What an expression denotes, if not a value.
   *
   * @param repeat the repeat of the records
   * @param isList whether it is a list of records, rather than a record
   */
  private record Records(Stmt.Repeat repeat, boolean isList) {}

  /**
   * A record name of a check.
   *
   * @param name identifies the values of the record name
   * @param label the name in messages
   * @param repeats the repeats the check ranges over, outermost first
   */
  private record CheckRecord(String name, String label, List<Stmt.Repeat> repeats) {}

  private void run(Program program) {
    define(program.body());
    checkBindings(program.bindings());
    checkTypes(program.types());
    checkRecordNames();
    visit(program.body());
  }

  private IrisException failure(ErrorCode code, String message) {
    return failureAt(code, line, message);
  }

  private IrisException failureAt(ErrorCode code, int at, String message) {
    return SpecErrors.at(code, origin, at, message);
  }

  /*
   * Definitions
   */

  private void define(@Nullable String name) {
    if (name != null && !name.equals("_")) {
      var definition = new Definition(name, List.copyOf(repeats));
      definitions.add(definition);
      definitionsByName.computeIfAbsent(name, n -> new ArrayList<>()).add(definition);
    }
  }

  private void define(Pattern pattern) {
    for (PatternItem item : pattern.items()) {
      if (item instanceof PatternItem.Capture capture) {
        define(capture.name());
      }
    }
  }

  private void define(List<Stmt> body) {
    for (Stmt statement : body) {
      define(statement);
    }
  }

  private void define(Stmt statement) {
    switch (statement) {
      case Stmt.Declaration s -> define(s.name());
      case Stmt.ListRead s -> define(s.name());
      case Stmt.Let s -> define(s.name());
      case Stmt.Param s -> define(s.name());
      case Stmt.Line s -> define(s.pattern());
      case Stmt.Find s -> define(s.pattern());
      case Stmt.Repeat s -> defineRepeat(s);
      case Stmt.Sections s -> defineSections(s);
      case Stmt.If s -> {
        define(s.body());
        define(s.orElse());
      }
      case Stmt.Instances s -> define(s.body());
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

  private void defineRepeat(Stmt.Repeat repeat) {
    if (repeat.count() instanceof RepeatCount.Until until) {
      define(until.terminators().store());
    }
    if (repeat.list() != null || repeat.each() != null) {
      namedRepeats.add(new NamedRepeat(repeat, List.copyOf(repeats)));
    }
    repeats.add(repeat);
    define(repeat.body());
    repeats.removeLast();
  }

  private void defineSections(Stmt.Sections sections) {
    Terminators terminators = sections.terminators();
    if (terminators != null) {
      define(terminators.store());
    }
    for (Arm arm : sections.arms()) {
      define(arm.pattern());
      define(arm.body());
    }
  }

  private boolean isDefined(String name) {
    return definitionsByName.containsKey(name);
  }

  /*
   * Conflicts between names
   */

  private void checkBindings(List<Binding> bindings) {
    for (Binding binding : bindings) {
      if (isDefined(binding.name())) {
        throw failureAt(
            ErrorCode.SPEC_NAME_CONFLICT,
            binding.line(),
            "'" + binding.name() + "' is bound by a comprehension but is also a name in the spec");
      }
    }
  }

  private void checkTypes(List<Stmt.TypeDefinition> types) {
    for (Stmt.TypeDefinition type : types) {
      if (isDefined(type.name())) {
        throw failureAt(
            ErrorCode.SPEC_NAME_CONFLICT,
            type.line(),
            "'" + type.name() + "' names a type, so it cannot also name a value");
      }
    }
  }

  private void checkRecordNames() {
    Set<String> preceding = new HashSet<>();
    for (NamedRepeat named : namedRepeats) {
      Stmt.Repeat repeat = named.repeat();
      checkRecordName(repeat.list(), repeat, preceding);
      if (Objects.equals(repeat.each(), repeat.list())) {
        throw failureAt(
            ErrorCode.SPEC_NAME_CONFLICT,
            repeat.line(),
            "'" + repeat.each() + "' cannot name both the list and each record");
      }
      checkRecordName(repeat.each(), repeat, preceding);
      if (repeat.list() != null) {
        preceding.add(repeat.list());
        lists.put(repeat.list(), named);
      }
      if (repeat.each() != null) {
        preceding.add(repeat.each());
        eachNames.put(repeat.each(), named);
      }
    }
  }

  private void checkRecordName(@Nullable String name, Stmt.Repeat repeat, Set<String> preceding) {
    if (name == null) {
      return;
    }
    if (preceding.contains(name)) {
      throw failureAt(
          ErrorCode.SPEC_NAME_CONFLICT, repeat.line(), "'" + name + "' already names records");
    }
    if (isDefined(name)) {
      throw failureAt(
          ErrorCode.SPEC_NAME_CONFLICT,
          repeat.line(),
          "'" + name + "' names records, so it cannot also be defined");
    }
  }

  /*
   * Statements
   */

  private void visit(List<Stmt> body) {
    for (Stmt statement : body) {
      visit(statement);
    }
  }

  /** Resolves the components of {@code statement} before its bodies, in the order of §11.2. */
  private void visit(Stmt statement) {
    line = statement.line();
    inCheck = statement instanceof Stmt.Check;
    checkRecords.clear();
    switch (statement) {
      case Stmt.Declaration s -> {
        bounds(s.type());
        s.dimensions().forEach(this::value);
        optionalValue(s.layout());
        optionalValue(s.offset());
      }
      case Stmt.ListRead s -> {
        bounds(s.type());
        optionalValue(s.sentinel());
        optionalValue(s.offset());
      }
      case Stmt.TypeDefinition s -> bounds(s.type());
      case Stmt.Param s -> optionalValue(s.defaultValue());
      case Stmt.Line s -> captureBounds(s.pattern());
      case Stmt.Find s -> captureBounds(s.pattern());
      case Stmt.Skip s -> value(s.count());
      case Stmt.IndexBase s -> value(s.base());
      case Stmt.Let s -> value(s.value());
      case Stmt.Check s -> {
        value(s.condition());
        checkChain();
      }
      case Stmt.Repeat s -> visitRepeat(s);
      case Stmt.Sections s -> {
        s.arms().forEach(arm -> captureBounds(arm.pattern()));
        s.arms().forEach(arm -> visit(arm.body()));
      }
      case Stmt.If s -> {
        value(s.condition());
        visit(s.body());
        visit(s.orElse());
      }
      case Stmt.Instances s -> visit(s.body());
      case Stmt.Comment _,
          Stmt.Stop _,
          Stmt.Tokens _,
          Stmt.Separators _,
          Stmt.IgnoreTrailing _,
          Stmt.Expect _,
          Stmt.Each _ -> {}
    }
  }

  private void visitRepeat(Stmt.Repeat repeat) {
    if (repeat.count() instanceof RepeatCount.Times times) {
      value(times.count());
    }
    repeats.add(repeat);
    visit(repeat.body());
    repeats.removeLast();
  }

  private void bounds(ReadingType type) {
    if (type instanceof ReadingType.Range range) {
      optionalValue(range.lower());
      optionalValue(range.upper());
    }
  }

  private void captureBounds(Pattern pattern) {
    for (PatternItem item : pattern.items()) {
      if (item instanceof PatternItem.Capture capture) {
        bounds(capture.type());
      }
    }
  }

  /** Checks that the record names of the check range over one repeat and those inside it. */
  private void checkChain() {
    int deepest = 0;
    for (int k = 1; k < checkRecords.size(); k++) {
      if (checkRecords.get(k).repeats().size() > checkRecords.get(deepest).repeats().size()) {
        deepest = k;
      }
    }
    for (int k = 0; k < checkRecords.size(); k++) {
      if (!isPrefix(checkRecords.get(k).repeats(), checkRecords.get(deepest).repeats())) {
        throw failure(
            ErrorCode.SPEC_CHECK_SCOPE,
            "a check can only range over the records of one repeat, but '"
                + checkRecords.get(Math.min(k, deepest)).label()
                + "' and '"
                + checkRecords.get(Math.max(k, deepest)).label()
                + "' are defined in different repeats");
      }
    }
  }

  private static boolean isPrefix(List<Stmt.Repeat> prefix, List<Stmt.Repeat> whole) {
    return prefix.size() <= whole.size() && whole.subList(0, prefix.size()).equals(prefix);
  }

  /*
   * Expressions
   */

  private void optionalValue(@Nullable Expr expression) {
    if (expression != null) {
      value(expression);
    }
  }

  /** Resolves {@code expression}, which must be a value. */
  private void value(Expr expression) {
    requireValue(resolve(expression, false));
  }

  private void requireValue(@Nullable Records records) {
    if (records == null) {
      return;
    }
    Stmt.Repeat repeat = records.repeat();
    if (!records.isList()) {
      throw failure(
          ErrorCode.SPEC_RECORD_VALUE,
          "a record of '"
              + recordsName(repeat)
              + "' is not a value; use one of its fields, as in '"
              + example(repeat, false)
              + "'");
    }
    throw failure(
        ErrorCode.SPEC_RECORD_VALUE,
        "'"
            + repeat.list()
            + "' is a list of records, not a value; use size("
            + repeat.list()
            + "), an index, or a field of a record, as in '"
            + example(repeat, true)
            + "'");
  }

  /** Resolves {@code expression}, a whole array if {@code whole}, and returns its records. */
  private @Nullable Records resolve(Expr expression, boolean whole) {
    if (expression instanceof Expr.Name name) {
      return name(name.name(), whole);
    }
    Records records = null;
    for (Expr node : expression.leftSpine()) {
      records = resolveOver(records, node);
    }
    return records;
  }

  /** Resolves {@code node}, whose left operand, if it has one, has the records {@code left}. */
  private @Nullable Records resolveOver(@Nullable Records left, Expr node) {
    return switch (node) {
      case Expr.Name e -> name(e.name(), false);
      case Expr.Field e -> field(e, left);
      case Expr.Index e -> index(e, left);
      case Expr.Comprehension e -> comprehension(e);
      case Expr.Call e -> call(e);
      case Expr.Unary e -> {
        value(e.operand());
        yield null;
      }
      case Expr.Binary e -> {
        requireValue(left);
        value(e.right());
        yield null;
      }
      case Expr.IntLiteral _, Expr.FloatLiteral _, Expr.StringLiteral _ -> null;
    };
  }

  private @Nullable Records name(String name, boolean whole) {
    for (Bound binding : bound.reversed()) {
      if (binding.name().equals(name)) {
        Stmt.Repeat repeat = binding.records();
        return repeat == null ? null : new Records(repeat, false);
      }
    }
    NamedRepeat list = lists.get(name);
    if (list != null) {
      return listName(name, list);
    }
    NamedRepeat each = eachNames.get(name);
    if (each != null) {
      return eachName(name, each);
    }
    if (inCheck && !whole) {
      liftDefinitions(name);
    }
    return null;
  }

  /** Returns the repeats of {@code enclosing} that are not around the statement visited. */
  private List<Stmt.Repeat> beyond(List<Stmt.Repeat> enclosing) {
    return enclosing.subList(commonPrefix(enclosing, repeats), enclosing.size());
  }

  private static int commonPrefix(List<Stmt.Repeat> first, List<Stmt.Repeat> second) {
    int common = 0;
    while (common < first.size()
        && common < second.size()
        && first.get(common).equals(second.get(common))) {
      common++;
    }
    return common;
  }

  private Records listName(String name, NamedRepeat list) {
    if (repeats.contains(list.repeat())) {
      throw failure(
          ErrorCode.SPEC_RECORD_SCOPE, "'" + name + "' cannot be used inside its own repeat");
    }
    if (beyond(list.repeats()).isEmpty()) {
      return new Records(list.repeat(), true);
    }
    // One list for each record of the repeat around it.
    String outer = list.repeats().getLast().each();
    throw failure(
        ErrorCode.SPEC_RECORD_SCOPE,
        "'"
            + name
            + "' is a list in each record of the repeat around it; "
            + (outer == null
                ? "name each of these records with 'each' and use its field '" + name + "'"
                : "use '" + outer + "." + name + "'"));
  }

  /**
   * Resolves a name of each record. Inside the repeat, it is the current record. Elsewhere, a check
   * ranges over the records.
   */
  private Records eachName(String name, NamedRepeat named) {
    Stmt.Repeat repeat = named.repeat();
    if (!repeats.contains(repeat)) {
      if (!inCheck) {
        throw failure(
            ErrorCode.SPEC_RECORD_SCOPE,
            "'"
                + name
                + "' is each record of a repeat, so only checks can use it outside that"
                + " repeat");
      }
      List<Stmt.Repeat> ranged = new ArrayList<>(beyond(named.repeats()));
      ranged.add(repeat);
      lift("#" + repeat.id(), name, ranged);
    }
    return new Records(repeat, false);
  }

  /** Makes {@code name} a record name of the check, if it is defined inside other repeats. */
  private void liftDefinitions(String name) {
    List<Definition> nameDefinitions = definitionsByName.getOrDefault(name, List.of());
    if (nameDefinitions.isEmpty()) {
      return;
    }
    List<Stmt.Repeat> outside = beyond(nameDefinitions.getFirst().repeats());
    for (Definition definition : nameDefinitions) {
      if (!beyond(definition.repeats()).equals(outside)) {
        throw failure(
            ErrorCode.SPEC_CHECK_SCOPE,
            "'"
                + name
                + "' is defined in several repeats, or inside and outside a repeat, so a check"
                + " here cannot range over its records");
      }
    }
    if (!outside.isEmpty()) {
      lift(name, name, outside);
    }
  }

  private void lift(String name, String label, List<Stmt.Repeat> ranged) {
    for (CheckRecord record : checkRecords) {
      if (record.name().equals(name) && record.repeats().size() == ranged.size()) {
        return;
      }
    }
    checkRecords.add(new CheckRecord(name, label, List.copyOf(ranged)));
  }

  private @Nullable Records field(Expr.Field field, @Nullable Records base) {
    String name = field.name();
    if (base == null) {
      throw failure(
          ErrorCode.SPEC_FIELD,
          "only records have fields such as '."
              + name
              + "'; to name each record of a repeat, write 'repeat n as nodes { each node ... }'"
              + " and use 'node."
              + name
              + "' or 'nodes[0]."
              + name
              + "'");
    }
    Stmt.Repeat repeat = base.repeat();
    if (base.isList()) {
      throw failure(
          ErrorCode.SPEC_FIELD,
          "'"
              + repeat.list()
              + "' is a list of records, so '."
              + name
              + "' needs one of them: use '"
              + repeat.list()
              + "[i]."
              + name
              + "', or "
              + (repeat.each() == null
                  ? "name each record with 'each' in the repeat and use its field"
                  : "'" + repeat.each() + "." + name + "' for each record"));
    }
    NamedRepeat nested = nestedList(repeat, name);
    if (nested != null) {
      return new Records(nested.repeat(), true);
    }
    requireField(repeat, name);
    return null;
  }

  /** Returns the list of records called {@code name} directly inside {@code repeat}, or null. */
  private @Nullable NamedRepeat nestedList(Stmt.Repeat repeat, String name) {
    NamedRepeat nested = lists.get(name);
    if (nested == null || !nested.repeats().contains(repeat)) {
      return null;
    }
    if (!nested.repeats().getLast().equals(repeat)) {
      throw failure(
          ErrorCode.SPEC_RECORD_SCOPE,
          "'"
              + name
              + "' is inside a repeat without a list name in the records of '"
              + recordsName(repeat)
              + "'; name that list with 'as' to reach it");
    }
    if (repeats.contains(nested.repeat())) {
      throw failure(
          ErrorCode.SPEC_RECORD_SCOPE, "'" + name + "' cannot be used inside its own repeat");
    }
    return nested;
  }

  /** Checks that {@code name} is a field of the records of {@code repeat}. */
  private void requireField(Stmt.Repeat repeat, String name) {
    List<Definition> nameDefinitions = definitionsByName.getOrDefault(name, List.of());
    boolean inside = false;
    boolean outside = false;
    for (Definition definition : nameDefinitions) {
      inside = inside || definition.repeats().contains(repeat);
      outside = outside || !definition.repeats().contains(repeat);
    }
    if (!inside) {
      String known = fields(repeat);
      throw failure(
          ErrorCode.SPEC_FIELD,
          "the records of '"
              + recordsName(repeat)
              + "' have no field '"
              + name
              + "'"
              + (known.isEmpty() ? " (they have none)" : " (they have " + known + ")"));
    }
    if (outside) {
      throw failure(
          ErrorCode.SPEC_FIELD,
          "'"
              + name
              + "' is defined inside and outside the repeat of '"
              + recordsName(repeat)
              + "', so it cannot be a field of its records");
    }
    for (Definition definition : nameDefinitions) {
      if (isIncomplete(definition, repeat)) {
        throw failure(
            ErrorCode.SPEC_RECORD_SCOPE,
            "the field '" + name + "' cannot be used inside the repeat that reads it");
      }
    }
  }

  /**
   * Tests whether a definition of a field of the records of {@code repeat} is in a repeat around
   * the statement inside {@code repeat}, so that its values for the record are not complete.
   */
  private boolean isIncomplete(Definition definition, Stmt.Repeat repeat) {
    return commonPrefix(definition.repeats(), repeats) > definition.repeats().indexOf(repeat) + 1;
  }

  private @Nullable Records index(Expr.Index index, @Nullable Records base) {
    value(index.index());
    if (base == null) {
      return null;
    }
    if (!base.isList()) {
      throw failure(
          ErrorCode.SPEC_RECORD_VALUE,
          "a record of '"
              + recordsName(base.repeat())
              + "' cannot be indexed; use one of its fields, as in '"
              + example(base.repeat(), false)
              + "'");
    }
    return new Records(base.repeat(), false);
  }

  private @Nullable Records comprehension(Expr.Comprehension comprehension) {
    String binding = comprehension.binding();
    if (lists.containsKey(binding) || eachNames.containsKey(binding)) {
      throw failure(
          ErrorCode.SPEC_NAME_CONFLICT,
          "'" + binding + "' is bound by a comprehension but names records");
    }
    Records domain = resolve(comprehension.domain(), true);
    if (domain != null && !domain.isList()) {
      throw failure(
          ErrorCode.SPEC_RECORD_VALUE,
          "a record of '"
              + recordsName(domain.repeat())
              + "' is not a list; use one of its fields");
    }
    bound.add(new Bound(binding, domain == null ? null : domain.repeat()));
    value(comprehension.body());
    bound.removeLast();
    return null;
  }

  /**
   * Resolves a call. The argument of {@code defined} is a whole array, and a list of records is a
   * value for {@code size}, {@code count} and {@code defined}.
   */
  private @Nullable Records call(Expr.Call call) {
    Function function = call.function();
    boolean isCounted =
        function == Function.SIZE || function == Function.COUNT || function == Function.DEFINED;
    for (Expr argument : call.arguments()) {
      Records records = resolve(argument, function == Function.DEFINED);
      if (!isCounted || records == null || !records.isList()) {
        requireValue(records);
      }
    }
    return null;
  }

  /*
   * Messages
   */

  private static @Nullable String recordsName(Stmt.Repeat repeat) {
    return repeat.list() == null ? repeat.each() : repeat.list();
  }

  /** Returns the names that the records of {@code repeat} have. */
  private String fields(Stmt.Repeat repeat) {
    Set<String> names = new LinkedHashSet<>();
    for (Definition definition : definitions) {
      if (definition.repeats().contains(repeat)) {
        names.add(definition.name());
      }
    }
    for (NamedRepeat named : namedRepeats) {
      if (!named.repeats().isEmpty()
          && named.repeats().getLast().equals(repeat)
          && named.repeat().list() != null) {
        names.add(named.repeat().list());
      }
    }
    return String.join(", ", names);
  }

  /** Returns an example of a field of a record of {@code repeat}, through its list if asked. */
  private String example(Stmt.Repeat repeat, boolean throughList) {
    String fields = fields(repeat);
    int comma = fields.indexOf(',');
    String record = throughList || repeat.each() == null ? repeat.list() + "[0]" : repeat.each();
    if (fields.isEmpty()) {
      return record;
    }
    return record + "." + (comma < 0 ? fields : fields.substring(0, comma));
  }
}
