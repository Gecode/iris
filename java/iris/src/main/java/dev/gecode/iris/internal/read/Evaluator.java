package dev.gecode.iris.internal.read;

import static dev.gecode.iris.internal.read.Execution.quote;

import dev.gecode.iris.ErrorCode;
import dev.gecode.iris.IrisException;
import dev.gecode.iris.ValueType;
import dev.gecode.iris.internal.syntax.Ast.BinaryOperator;
import dev.gecode.iris.internal.syntax.Ast.ComprehensionFunction;
import dev.gecode.iris.internal.syntax.Ast.Expr;
import dev.gecode.iris.internal.syntax.Ast.Function;
import dev.gecode.iris.internal.syntax.Ast.Stmt;
import dev.gecode.iris.internal.syntax.Ast.UnaryOperator;
import dev.gecode.iris.internal.syntax.Numbers;
import dev.gecode.iris.internal.syntax.Resolution;
import dev.gecode.iris.internal.syntax.Resolution.CheckRecord;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Locale;
import java.util.Objects;
import org.jspecify.annotations.Nullable;

/**
 * Evaluates the expressions and checks of a reader spec with the values defined so far (§8).
 *
 * <p>Left operands can nest far deeper than any stack (see {@link Expr}), so an expression is
 * evaluated by folding over its left spine, and only its other operands recurse.
 */
final class Evaluator {
  private static final Stored TRUE = new Stored.Int(1);
  private static final Stored FALSE = new Stored.Int(0);
  private static final int[] NO_PATH = {};

  private final Execution execution;
  private final Input input;
  private final Resolution resolution;

  /** The innermost name bound by the comprehensions being evaluated. */
  private @Nullable Binding bindings = null;

  /** The record names of the check being evaluated. */
  private List<CheckRecord> checkRecords = List.of();

  /** The values of those record names, null if not defined. */
  private @Nullable Stored[] records = {};

  /** The record of the check being evaluated: an index per repeat level of its deepest name. */
  private int[] recordAt = NO_PATH;

  private int recordLevels = 0;

  /** The index of the deepest record name of the check, or -1 if no record name is defined. */
  private int deepest = -1;

  /**
   * Why the last condition evaluated is false, for messages: the elements of the all-comprehensions
   * that failed, innermost first.
   */
  private final List<Witness> why = new ArrayList<>();

  /**
   * Records or a list of records of a named repeat. The indices {@code path} select them in the
   * record starts of the repeat, and in each name defined in the repeat.
   */
  private static final class RecordRef {
    private final Stmt.Repeat repeat;
    private final int[] path;

    RecordRef(Stmt.Repeat repeat, int[] path) {
      this.repeat = repeat;
      this.path = path;
    }

    Stmt.Repeat repeat() {
      return repeat;
    }

    int[] path() {
      return path;
    }
  }

  /**
   * An element of an all-comprehension that made it false.
   *
   * @param comprehension the comprehension
   * @param records the records that are the elements, or null for the elements of an array
   * @param index the index of the element
   */
  private record Witness(
      Expr.Comprehension comprehension, @Nullable RecordRef records, int index) {}

  /** A name bound by a comprehension, to a value or to a record of a named repeat. */
  private static final class Binding {
    final String name;
    final @Nullable Binding outer;
    @Nullable Stored value = null;
    @Nullable RecordRef record = null;

    Binding(String name, @Nullable Binding outer) {
      this.name = name;
      this.outer = outer;
    }
  }

  Evaluator(Execution execution, Resolution resolution) {
    this.execution = execution;
    this.input = execution.input();
    this.resolution = resolution;
  }

  /*
   * Names and records
   */

  private IrisException undefined(String name) {
    return execution.failure(ErrorCode.NAME_UNDEFINED, "'" + name + "' is not defined yet");
  }

  private @Nullable Stored lookup(String name) {
    for (Binding binding = bindings; binding != null; binding = binding.outer) {
      if (binding.name.equals(name)) {
        return binding.value;
      }
    }
    return execution.lookup(name);
  }

  private static String listName(Stmt.Repeat repeat) {
    String list = repeat.list();
    return list == null ? "" : list;
  }

  private Stored name(Expr.Name e) {
    int record = resolution.checkRecord(e);
    Stored value = record >= 0 ? records[record] : lookup(e.name());
    if (value == null) {
      throw undefined(e.name());
    }
    return value;
  }

  /** Returns the part of {@code root} that {@code path} selects, or null if it is out of range. */
  private static @Nullable Stored locate(Stored root, int[] path, int length) {
    Stored at = root;
    for (int k = 0; k < length; k++) {
      if (!(at instanceof Stored.Array array) || path[k] >= array.size()) {
        return null;
      }
      at = array.element(path[k]);
    }
    return at;
  }

  private int countRecords(RecordRef list) {
    Stmt.Repeat repeat = list.repeat();
    Stored starts = execution.lookup(Resolution.startsName(repeat));
    if (starts == null) {
      throw undefined(listName(repeat));
    }
    return locate(starts, list.path(), list.path().length) instanceof Stored.Array records
        ? records.size()
        : 0;
  }

  /** Returns where the record {@code path} starts in the input of {@code repeat}, or -1. */
  private int recordStart(Stmt.Repeat repeat, int[] path) {
    return recordStart(Resolution.startsName(repeat), path);
  }

  private int recordStart(String startsName, int[] path) {
    Stored starts = execution.lookup(startsName);
    if (starts != null && locate(starts, path, path.length) instanceof Stored.Int start) {
      return (int) start.value();
    }
    return -1;
  }

  private static int[] appended(int[] path, int index) {
    int[] longer = Arrays.copyOf(path, path.length + 1);
    longer[path.length] = index;
    return longer;
  }

  /** Evaluates {@code e}, a record or a list of records. */
  private RecordRef records(Expr e) {
    RecordRef records = null;
    for (Expr node : e.leftSpine()) {
      records = recordsStep(node, records);
    }
    return Objects.requireNonNull(records);
  }

  /** Evaluates {@code node}, of records, whose left operand has the records {@code left}. */
  private RecordRef recordsStep(Expr node, @Nullable RecordRef left) {
    Stmt.Repeat repeat = Objects.requireNonNull(resolution.denotedRepeat(node));
    return switch (node) {
      case Expr.Name e -> recordsOfName(e, repeat);
      // A list of records in a record.
      case Expr.Field _ -> new RecordRef(repeat, Objects.requireNonNull(left).path());
      case Expr.Index e -> recordOfList(e, Objects.requireNonNull(left));
      default -> throw new IllegalStateException("not an expression of records");
    };
  }

  private RecordRef recordsOfName(Expr.Name e, Stmt.Repeat repeat) {
    int record = resolution.checkRecord(e);
    if (record >= 0) {
      // Records that a check ranges over.
      CheckRecord checkRecord = checkRecords.get(record);
      if (records[record] == null) {
        throw undefined(checkRecord.label());
      }
      return new RecordRef(repeat, Arrays.copyOf(recordAt, checkRecord.repeats().size()));
    }
    for (Binding binding = bindings; binding != null; binding = binding.outer) {
      if (binding.name.equals(e.name())) {
        return Objects.requireNonNull(binding.record);
      }
    }
    // A record name outside a check is the current record.
    if (resolution.isRecordList(e) && execution.lookup(Resolution.startsName(repeat)) == null) {
      throw undefined(e.name());
    }
    return new RecordRef(repeat, NO_PATH);
  }

  private RecordRef recordOfList(Expr.Index e, RecordRef list) {
    long index = index(e.index());
    int count = countRecords(list);
    if (index < 0 || index >= count) {
      throw execution.failure(
          ErrorCode.INDEX_RANGE,
          "index "
              + index
              + " is out of range for '"
              + listName(list.repeat())
              + "' (size "
              + count
              + ", indices start at 0)");
    }
    execution.chargeEffort(1);
    return new RecordRef(list.repeat(), appended(list.path(), (int) index));
  }

  /**
   * Evaluates the field {@code e} of the record {@code base}: the field of a list is the array of
   * the field of each record.
   */
  private Stored field(Expr.Field e, RecordRef base) {
    Stored value = lookup(e.name());
    if (value == null) {
      throw undefined(e.name());
    }
    int[] path = base.path();
    if (path.length == 0) {
      return value;
    }
    Stored part = locate(value, path, path.length);
    if (part == null) {
      throw execution.failure(
          ErrorCode.FIELD_UNDEFINED,
          "'" + e.name() + "' is not defined in this record of '" + listName(base.repeat()) + "'");
    }
    execution.chargeEffort(1L + part.leaves());
    return part;
  }

  /*
   * Expressions
   */

  /** Tests whether {@code value} is true. */
  static boolean truth(Stored value) {
    return switch (value) {
      case Stored.Int v -> v.value() != 0;
      case Stored.Real v -> v.value() != 0.0;
      case Stored.Text v -> !v.value().isEmpty();
      case Stored.Array v -> v.size() > 0;
    };
  }

  private static Stored bool(boolean value) {
    return value ? TRUE : FALSE;
  }

  /** Evaluates {@code expression}. */
  Stored evaluate(Expr expression) {
    why.clear();
    switch (expression) {
      case Expr.IntLiteral e -> {
        return new Stored.Int(e.value());
      }
      case Expr.Name e when resolution.denotedRepeat(e) == null -> {
        return name(e);
      }
      default -> {}
    }
    List<Expr> spine = expression.leftSpine();
    int next = 0;
    RecordRef base = null;
    while (next < spine.size() && resolution.denotedRepeat(spine.get(next)) != null) {
      base = recordsStep(spine.get(next), base);
      next++;
    }
    if (next == spine.size()) {
      throw new IllegalStateException("records are not a value");
    }
    Stored value = leftmost(spine.get(next), base);
    clearUnlessExplaining(spine.get(next));
    for (next++; next < spine.size(); next++) {
      Expr node = spine.get(next);
      value =
          switch (node) {
            case Expr.Binary e -> binary(e, value);
            case Expr.Index e -> subscript(e, value);
            default -> throw new IllegalStateException("not a left operand");
          };
      clearUnlessExplaining(node);
    }
    return value;
  }

  /** Evaluates {@code node}, which has no left operand, or the records {@code base} as one. */
  private Stored leftmost(Expr node, @Nullable RecordRef base) {
    return switch (node) {
      case Expr.IntLiteral e -> new Stored.Int(e.value());
      case Expr.FloatLiteral e -> new Stored.Real(e.value());
      case Expr.StringLiteral e -> new Stored.Text(e.value());
      case Expr.Name e -> name(e);
      case Expr.Unary e -> unary(e);
      case Expr.Call e -> call(e);
      case Expr.Comprehension e -> comprehension(e);
      case Expr.Field e -> field(e, Objects.requireNonNull(base));
      case Expr.Binary _, Expr.Index _ -> throw new IllegalStateException("has a left operand");
    };
  }

  private void clearUnlessExplaining(Expr node) {
    if (!explains(node)) {
      why.clear();
    }
  }

  /** Tests whether {@code e} passes on why it is false (see {@link #why}). */
  private static boolean explains(Expr e) {
    return switch (e) {
      case Expr.Binary binary ->
          binary.operator() == BinaryOperator.OR || binary.operator() == BinaryOperator.AND;
      case Expr.Comprehension comprehension ->
          comprehension.function() == ComprehensionFunction.ALL;
      default -> false;
    };
  }

  private long integer(Stored value, String operator) {
    if (value instanceof Stored.Int integer) {
      return integer.value();
    }
    throw execution.failure(ErrorCode.INTEGER_TYPE, "'" + operator + "' needs integers");
  }

  private Stored unary(Expr.Unary e) {
    Stored operand = evaluate(e.operand());
    if (e.operator() == UnaryOperator.NOT) {
      return bool(!truth(operand));
    }
    if (operand instanceof Stored.Real real) {
      return new Stored.Real(-real.value());
    }
    long value = integer(operand, "-");
    if (value == Long.MIN_VALUE) {
      throw execution.failure(ErrorCode.INTEGER_OVERFLOW, "integer overflow in '-'");
    }
    return new Stored.Int(-value);
  }

  private Stored binary(Expr.Binary e, Stored left) {
    BinaryOperator operator = e.operator();
    // Only evaluate the second operand if needed.
    if (operator == BinaryOperator.OR) {
      boolean isDecided = truth(left);
      why.clear();
      return isDecided ? TRUE : bool(truth(evaluate(e.right())));
    }
    if (operator == BinaryOperator.AND) {
      if (!truth(left)) {
        return FALSE;
      }
      why.clear();
      return bool(truth(evaluate(e.right())));
    }
    Stored right = evaluate(e.right());
    return switch (operator) {
      case EQUAL, NOT_EQUAL, LESS, GREATER, LESS_EQUAL, GREATER_EQUAL ->
          bool(compare(left, right, operator));
      // Arithmetic is on integers: reader specs compute sizes and counts.
      case ADD, SUBTRACT, MULTIPLY, DIVIDE, REMAINDER, POWER -> {
        String text = text(operator);
        long x = integer(left, text);
        long y = integer(right, text);
        yield new Stored.Int(arithmetic(operator, x, y));
      }
      case OR, AND -> throw new IllegalStateException("handled above");
    };
  }

  private static String text(BinaryOperator operator) {
    return switch (operator) {
      case OR -> "or";
      case AND -> "and";
      case EQUAL -> "==";
      case NOT_EQUAL -> "!=";
      case LESS -> "<";
      case GREATER -> ">";
      case LESS_EQUAL -> "<=";
      case GREATER_EQUAL -> ">=";
      case ADD -> "+";
      case SUBTRACT -> "-";
      case MULTIPLY -> "*";
      case DIVIDE -> "/";
      case REMAINDER -> "%";
      case POWER -> "^";
    };
  }

  private long arithmetic(BinaryOperator operator, long x, long y) {
    try {
      return switch (operator) {
        case ADD -> Math.addExact(x, y);
        case SUBTRACT -> Math.subtractExact(x, y);
        case MULTIPLY -> Math.multiplyExact(x, y);
        case POWER -> {
          if (y < 0) {
            throw execution.failure(ErrorCode.EXPONENT, "negative exponent in '^'");
          }
          yield Numbers.power(x, y);
        }
        // Division and remainder truncate towards zero.
        case DIVIDE -> {
          requireDivisor(y);
          yield Math.divideExact(x, y);
        }
        case REMAINDER -> {
          requireDivisor(y);
          if (x == Long.MIN_VALUE && y == -1) {
            throw new ArithmeticException("overflow");
          }
          yield x % y;
        }
        default -> throw new IllegalStateException("not arithmetic");
      };
    } catch (ArithmeticException _) {
      throw execution.failure(
          ErrorCode.INTEGER_OVERFLOW, "integer overflow in '" + text(operator) + "'");
    }
  }

  private void requireDivisor(long divisor) {
    if (divisor == 0) {
      throw execution.failure(ErrorCode.DIVISION_ZERO, "division by zero");
    }
  }

  private boolean compare(Stored a, Stored b, BinaryOperator operator) {
    String text = text(operator);
    if (a.depth() > 0 || b.depth() > 0) {
      throw execution.failure(
          ErrorCode.COMPARISON_TYPE, "'" + text + "' compares single values, not arrays");
    }
    int order;
    if (a.type() == ValueType.STRING || b.type() == ValueType.STRING) {
      if (operator != BinaryOperator.EQUAL && operator != BinaryOperator.NOT_EQUAL) {
        throw execution.failure(ErrorCode.COMPARISON_TYPE, "'" + text + "' compares numbers");
      }
      order = a.equals(b) ? 0 : 1;
    } else if (a instanceof Stored.Int x && b instanceof Stored.Int y) {
      order = Long.compare(x.value(), y.value());
    } else {
      // An integer compared with a float is converted to a float.
      double x = a.asDouble();
      double y = b.asDouble();
      order = x < y ? -1 : (x > y ? 1 : 0);
    }
    return switch (operator) {
      case EQUAL -> order == 0;
      case NOT_EQUAL -> order != 0;
      case LESS -> order < 0;
      case GREATER -> order > 0;
      case LESS_EQUAL -> order <= 0;
      case GREATER_EQUAL -> order >= 0;
      default -> throw new IllegalStateException("not a comparison");
    };
  }

  private static String text(Function function) {
    return function.name().toLowerCase(Locale.ROOT);
  }

  private static String text(ComprehensionFunction function) {
    return function.name().toLowerCase(Locale.ROOT);
  }

  private Stored call(Expr.Call e) {
    Function function = e.function();
    return switch (function) {
      case DEFINED -> bool(isDefined((Expr.Name) e.arguments().getFirst()));
      case COUNT, SIZE -> new Stored.Int(size(function, e.arguments().getFirst()));
      case DISTINCT -> bool(areDistinct(e));
      case MIN, MAX, SUM -> new Stored.Int(fold(e));
    };
  }

  private boolean isDefined(Expr.Name e) {
    for (Binding binding = bindings; binding != null; binding = binding.outer) {
      if (binding.name.equals(e.name())) {
        return true;
      }
    }
    Stmt.Repeat repeat = resolution.denotedRepeat(e);
    if (repeat != null) {
      return execution.lookup(Resolution.startsName(repeat)) != null;
    }
    return execution.lookup(e.name()) != null;
  }

  private long size(Function function, Expr argument) {
    if (resolution.denotedRepeat(argument) != null) {
      return countRecords(records(argument));
    }
    if (evaluate(argument) instanceof Stored.Array array) {
      return array.size();
    }
    throw execution.failure(ErrorCode.FUNCTION_TYPE, "'" + text(function) + "' takes an array");
  }

  /** Returns the integer leaves of {@code value}, an argument of {@code function}, as a list. */
  private void addLeaves(Stored value, Function function, Leaves into) {
    if (value.type() != ValueType.INT && value.leaves() > 0) {
      throw execution.failure(ErrorCode.FUNCTION_TYPE, "'" + text(function) + "' takes integers");
    }
    execution.chargeEffort(value.leaves());
    switch (value) {
      case Stored.Int v -> into.add(v.value());
      case Stored.Array v -> {
        if (v.leaves() > 0) {
          into.addAll(v.table(), v.firstLeaf(), v.endLeaf());
        }
      }
      case Stored.Real _, Stored.Text _ -> throw new IllegalStateException("not an integer");
    }
  }

  /** min, max and sum over all integers in all arguments. */
  private long fold(Expr.Call e) {
    Function function = e.function();
    boolean isAny = false;
    long result = 0;
    for (Expr argument : e.arguments()) {
      Leaves integers = new Leaves(ValueType.INT);
      addLeaves(evaluate(argument), function, integers);
      for (long x : integers.ints()) {
        if (function == Function.SUM) {
          result = checkedSum(result, x);
        } else if (!isAny || (function == Function.MIN ? x < result : x > result)) {
          result = x;
        }
        isAny = true;
      }
    }
    if (!isAny && function != Function.SUM) {
      throw execution.failure(ErrorCode.FUNCTION_EMPTY, "'" + text(function) + "' of no integers");
    }
    return result;
  }

  private long checkedSum(long x, long y) {
    try {
      return Math.addExact(x, y);
    } catch (ArithmeticException _) {
      throw execution.failure(ErrorCode.INTEGER_OVERFLOW, "integer overflow in 'sum'");
    }
  }

  private boolean areDistinct(Expr.Call e) {
    Leaves integers = new Leaves(ValueType.INT);
    for (Expr argument : e.arguments()) {
      addLeaves(evaluate(argument), e.function(), integers);
    }
    long[] sorted = integers.ints().clone();
    Arrays.sort(sorted);
    for (int k = 1; k < sorted.length; k++) {
      if (sorted[k - 1] == sorted[k]) {
        return false;
      }
    }
    return true;
  }

  /** Describes the array of the index expression {@code e}, for messages. */
  private static String indexed(Expr.Index e) {
    Expr base = e.base();
    boolean isNested = false;
    while (base instanceof Expr.Index inner) {
      base = inner.base();
      isNested = true;
    }
    String prefix = isNested ? "an element of '" : "'";
    return switch (base) {
      case Expr.Name name -> prefix + name.name() + "'";
      case Expr.Field field -> prefix + field.name() + "'";
      default -> "the array";
    };
  }

  private long index(Expr e) {
    if (evaluate(e) instanceof Stored.Int index) {
      return index.value();
    }
    throw execution.failure(ErrorCode.INDEX_TYPE, "an index must be an integer");
  }

  /**
   * Returns a hint for indexing {@code e}, a single value. A record name in a check is one record,
   * so that indexing the records needs their name.
   */
  private String scalarHint(Expr e) {
    if (!(e instanceof Expr.Name name) || resolution.checkRecord(name) < 0) {
      return "";
    }
    CheckRecord record = checkRecords.get(resolution.checkRecord(name));
    String list = record.repeats().getLast().list();
    String hint = ", which is one value of each record here; to index the records, ";
    if (list == null) {
      return hint
          + "name them, as in 'repeat n as nodes { each node ... }', and use 'nodes[0]."
          + name.name()
          + "'";
    }
    return hint + "use '" + list + "[0]." + name.name() + "'";
  }

  /** {@code a[i]}: element i, counted from 0, of the outermost list of {@code a}. */
  private Stored subscript(Expr.Index e, Stored base) {
    if (!(base instanceof Stored.Array array)) {
      throw execution.failure(
          ErrorCode.INDEX_SCALAR,
          "only arrays can be indexed, not " + indexed(e) + scalarHint(e.base()));
    }
    long index = index(e.index());
    if (index < 0 || index >= array.size()) {
      throw execution.failure(
          ErrorCode.INDEX_RANGE,
          "index "
              + index
              + " is out of range for "
              + indexed(e)
              + " (size "
              + array.size()
              + ", indices start at 0)");
    }
    Stored element = array.element((int) index);
    execution.chargeEffort(1L + element.leaves());
    return element;
  }

  /*
   * Comprehensions
   */

  /**
   * all, any, count and sum over the elements of an array or a list of records. The name of {@code
   * e} is bound to each in turn. {@code all} and {@code any} stop at the first element that decides
   * the result.
   */
  private Stored comprehension(Expr.Comprehension e) {
    Expr domain = e.domain();
    Stored.Array array = null;
    RecordRef list = null;
    RecordRef elements = null;
    int count;
    if (resolution.denotedRepeat(domain) != null) {
      list = records(domain);
      elements = list;
      count = countRecords(list);
    } else {
      int[] basePath = NO_PATH;
      Stored value;
      if (domain instanceof Expr.Field field) {
        RecordRef base = records(field.base());
        basePath = base.path();
        value = field(field, base);
      } else {
        value = evaluate(domain);
      }
      if (!(value instanceof Stored.Array values)) {
        throw execution.failure(
            ErrorCode.FUNCTION_TYPE,
            "'" + text(e.function()) + "(" + e.binding() + " in ...)' needs an array");
      }
      array = values;
      count = values.size();
      Stmt.Repeat repeat = resolution.elements(e);
      elements = repeat == null ? null : new RecordRef(repeat, basePath);
    }
    Binding binding = new Binding(e.binding(), bindings);
    bindings = binding;
    Stored result = iterate(e, binding, array, list, elements, count);
    bindings = binding.outer;
    return result;
  }

  private Stored iterate(
      Expr.Comprehension e,
      Binding binding,
      Stored.@Nullable Array array,
      @Nullable RecordRef list,
      @Nullable RecordRef elements,
      int count) {
    ComprehensionFunction function = e.function();
    long result = 0;
    for (int k = 0; k < count; k++) {
      if (array != null) {
        Stored element = array.element(k);
        execution.chargeEffort(1L + element.leaves());
        binding.value = element;
      } else {
        RecordRef all = Objects.requireNonNull(list);
        execution.chargeEffort(1);
        binding.record = new RecordRef(all.repeat(), appended(all.path(), k));
      }
      if (function == ComprehensionFunction.SUM) {
        result = checkedSum(result, integer(evaluate(e.body()), "sum"));
      } else if (truth(evaluate(e.body()))) {
        if (function == ComprehensionFunction.ANY) {
          return TRUE;
        }
        result++;
      } else if (function == ComprehensionFunction.ALL) {
        why.add(new Witness(e, elements, k));
        return FALSE;
      }
    }
    why.clear();
    return switch (function) {
      case ALL -> TRUE;
      case ANY -> FALSE;
      case COUNT, SUM -> new Stored.Int(result);
    };
  }

  /** Returns the value of {@code e}, a count described by {@code what}. */
  int count(Expr e, String what) {
    if (!(evaluate(e) instanceof Stored.Int integer)) {
      throw execution.failure(ErrorCode.COUNT, what + " must be an integer");
    }
    long count = integer.value();
    if (count < 0) {
      throw execution.failure(ErrorCode.COUNT, what + " is negative (" + count + ")");
    }
    if (count > Integer.MAX_VALUE) {
      throw execution.failure(ErrorCode.COUNT, what + " is too large (" + count + ")");
    }
    return (int) count;
  }

  /** Returns the value of the offset {@code e} (§6.7). */
  long offset(Expr e) {
    if (evaluate(e) instanceof Stored.Int integer) {
      return integer.value();
    }
    throw execution.failure(ErrorCode.OFFSET_TYPE, "offset must be an integer");
  }

  /*
   * Checks
   */

  /**
   * Tests whether {@code check} holds. It must hold for every record of its record names that are
   * defined. The others are not defined when its expression uses them. If it does not hold, {@link
   * #failure} can then say why.
   */
  boolean holds(Stmt.Check check) {
    checkRecords = resolution.checkRecords(check);
    records = new Stored[checkRecords.size()];
    deepest = -1;
    for (int k = 0; k < records.length; k++) {
      records[k] = execution.lookup(checkRecords.get(k).name());
      if (ranges(k, deepest < 0 ? 0 : levelsOf(deepest))) {
        deepest = k;
      }
    }
    recordAt = new int[deepest < 0 ? 0 : levelsOf(deepest)];
    recordLevels = 0;
    return holdsFromLevel(check);
  }

  private int levelsOf(int record) {
    return checkRecords.get(record).repeats().size();
  }

  /** Tests whether record name {@code record} is defined and ranges over {@code level}. */
  private boolean ranges(int record, int level) {
    return records[record] != null && levelsOf(record) > level;
  }

  /**
   * Evaluates {@code check} for the records inside the current ones. Recursion is by the repeat
   * levels, which nest as far as blocks do. If the check fails, the state stays as it was, for
   * {@link #failure}.
   */
  private boolean holdsFromLevel(Stmt.Check check) {
    int level = recordLevels;
    if (deepest < 0 || level == levelsOf(deepest)) {
      return truth(evaluate(check.condition()));
    }
    int count = 0;
    int first = -1;
    for (int k = 0; k < records.length; k++) {
      if (ranges(k, level)) {
        String label = checkRecords.get(k).label();
        if (!(records[k] instanceof Stored.Array array)) {
          throw execution.failure(ErrorCode.CHECK_RECORDS, "'" + label + "' has no records");
        }
        if (first < 0) {
          first = k;
          count = array.size();
        } else if (array.size() != count) {
          throw execution.failure(
              ErrorCode.CHECK_RECORDS,
              "'"
                  + checkRecords.get(first).label()
                  + "' and '"
                  + label
                  + "' have different numbers of records");
        }
      }
    }
    @Nullable Stored[] outer = records.clone();
    for (int row = 0; row < count; row++) {
      for (int k = 0; k < records.length; k++) {
        if (ranges(k, level) && outer[k] instanceof Stored.Array array) {
          Stored element = array.element(row);
          execution.chargeEffort(1L + element.leaves());
          records[k] = element;
        }
      }
      recordAt[level] = row;
      recordLevels = level + 1;
      if (!holdsFromLevel(check)) {
        return false;
      }
      recordLevels = level;
    }
    records = outer;
    return true;
  }

  /** Returns why {@code check} failed in the last call of {@link #holds}. */
  IrisException failure(Stmt.Check check) {
    // The innermost record found is where the check failed.
    StringBuilder text = new StringBuilder();
    int position = -1;
    for (Witness witness : why) {
      text.append(text.isEmpty() ? "" : " in ").append(describe(witness));
      RecordRef records = witness.records();
      if (position < 0 && records != null) {
        position = recordStart(records.repeat(), appended(records.path(), witness.index()));
      }
    }
    int[] at = Arrays.copyOf(recordAt, recordLevels);
    if (deepest >= 0 && at.length > 0) {
      CheckRecord record = checkRecords.get(deepest);
      text.append(text.isEmpty() ? "" : " in ").append(describe(record, at));
      if (position < 0) {
        position = recordStart(record.starts(), at);
      }
    }
    String message = text.isEmpty() ? "check failed" : "check failed for " + text;
    String explanation = check.message();
    if (explanation != null && !explanation.isEmpty()) {
      message += ": " + explanation;
    }
    // The lifted record indices, then the witnesses, outermost first.
    List<Integer> path = new ArrayList<>();
    for (int index : at) {
      path.add(index);
    }
    for (Witness witness : why.reversed()) {
      path.add(witness.index());
    }
    Place place =
        position >= 0 && position <= input.position() ? input.place(position) : input.here();
    return execution.checkFailure(message, place, path);
  }

  private static String describe(Witness witness) {
    Expr.Comprehension comprehension = witness.comprehension();
    String number = Integer.toString(witness.index() + 1);
    RecordRef records = witness.records();
    if (records == null) {
      return "element " + number + " of " + arrayName(comprehension.domain());
    }
    String list = records.repeat().list();
    return "record "
        + number
        + " of "
        + (list == null ? arrayName(comprehension.domain()) : quote(list));
  }

  /** Describes the array of a comprehension, for messages. */
  private static String arrayName(Expr array) {
    if (array instanceof Expr.Name name) {
      return "'" + name.name() + "'";
    }
    if (array instanceof Expr.Field field && field.base() instanceof Expr.Name base) {
      return "'" + base.name() + "." + field.name() + "'";
    }
    return "the array";
  }

  /** Describes the records {@code at} of {@code record}, innermost first. */
  private static String describe(CheckRecord record, int[] at) {
    boolean isNamed = record.repeats().stream().allMatch(repeat -> repeat.list() != null);
    StringBuilder text = new StringBuilder();
    for (int k = at.length - 1; k >= 0; k--) {
      text.append("record ").append(at[k] + 1).append(" of ");
      if (isNamed) {
        text.append('\'').append(record.repeats().get(k).list()).append('\'');
        text.append(k > 0 ? " in " : "");
      }
    }
    return isNamed ? text.toString() : text + "'" + record.label() + "'";
  }
}
