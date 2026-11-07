package dev.gecode.iris.internal.read;

import static dev.gecode.iris.internal.read.Execution.quote;
import static java.nio.charset.StandardCharsets.UTF_8;

import dev.gecode.iris.ErrorCode;
import dev.gecode.iris.IrisException;
import dev.gecode.iris.ValueType;
import dev.gecode.iris.internal.read.Execution.Context;
import dev.gecode.iris.internal.syntax.Ast.Arm;
import dev.gecode.iris.internal.syntax.Ast.Expr;
import dev.gecode.iris.internal.syntax.Ast.Program;
import dev.gecode.iris.internal.syntax.Ast.ReadingKind;
import dev.gecode.iris.internal.syntax.Ast.ReadingType;
import dev.gecode.iris.internal.syntax.Ast.RepeatCount;
import dev.gecode.iris.internal.syntax.Ast.Stmt;
import dev.gecode.iris.internal.syntax.Ast.Terminators;
import dev.gecode.iris.internal.syntax.Numbers;
import dev.gecode.iris.internal.syntax.Resolution;
import dev.gecode.iris.internal.syntax.Utf8;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.OptionalDouble;
import java.util.OptionalLong;
import java.util.SequencedMap;
import java.util.function.Supplier;
import java.util.stream.Collectors;
import org.jspecify.annotations.Nullable;

/**
 * Runs a reader spec on an input and returns the data read or the catalog of its instances (§9).
 */
public final class Interpreter {
  private final Program program;
  private final Resolution resolution;
  private final Map<String, String> params;
  private final Input input;
  private final Execution execution;
  private final Evaluator evaluator;
  private final Domains domains;
  private final ValueReader reader;
  private final PatternMatcher matcher;

  /** The row of the innermost repeat, or -1 outside any. */
  private int row = -1;

  /** Whether input after the reader spec is allowed. */
  private boolean ignoresTrailing = false;

  /** The value of the parameter {@code instance}, or null if it selects no instance. */
  private @Nullable String wanted = null;

  /** The number of the instance selected, unless it is selected by name. */
  private OptionalLong wantedNumber = OptionalLong.empty();

  /** The number of the instance being read, or 0 if there is none. */
  private int current = 0;

  /** The frame of the instance being read. */
  private int instanceFrame = 0;

  /** The first rejection of the instance being read, or null. */
  private @Nullable IrisException deferred = null;

  /** Whether the selected instance has been read. */
  private boolean isSelectedRead = false;

  /** What a catalog found so far, or null during a read. */
  private @Nullable Catalog catalog = null;

  /**
   * Prepares a read or a catalog of {@code text}.
   *
   * @param program the reader spec
   * @param resolution what the reader spec refers to
   * @param specOrigin the name of the reader spec in errors, or null if it has none
   * @param text the input
   * @param origin the name of the input in errors, or null if it has none
   * @param params the parameters, in the order they were given
   */
  public Interpreter(
      Program program,
      Resolution resolution,
      @Nullable String specOrigin,
      byte[] text,
      @Nullable String origin,
      Map<String, String> params) {
    this.program = program;
    this.resolution = resolution;
    this.params = params;
    this.input = new Input(text, origin);
    this.execution = new Execution(input, specOrigin);
    this.evaluator = new Evaluator(execution, resolution);
    this.domains = new Domains(execution, evaluator, program.types());
    this.reader = new ValueReader(execution, evaluator);
    this.matcher = new PatternMatcher(execution, domains, reader);
  }

  /**
   * Runs the reader spec.
   *
   * @return the values read, in definition order
   * @throws IrisException for the first error
   */
  public SequencedMap<String, Stored> run() {
    execute();
    SequencedMap<String, Stored> data = new LinkedHashMap<>();
    execution
        .frame(0)
        .forEach((name, value) -> {
          if (!Resolution.isInternal(name)) {
            data.put(name, value);
          }
        });
    return data;
  }

  /**
   * Runs the reader spec on every instance of the input, keeping none of their values (§9.6).
   *
   * @return the entries of the catalog, in order
   * @throws IrisException for the first error that is not a rejection in an instance
   */
  public SequencedMap<String, Stored> catalog() {
    Catalog result = new Catalog();
    catalog = result;
    execute();
    return result.entries();
  }

  private void execute() {
    // A misspelt parameter would otherwise be ignored silently.
    for (String name : params.keySet()) {
      if (!resolution.parameters().contains(name)) {
        throw execution.specFailure(
            ErrorCode.PARAM_UNKNOWN, "the spec has no parameter '" + name + "'");
      }
    }
    // The whole input must be valid UTF-8 (§4.10).
    int invalid = Utf8.firstInvalid(input.bytes());
    if (invalid >= 0) {
      input.advance(invalid);
      throw execution.inputFailure(ErrorCode.UTF8, "input is not valid UTF-8");
    }
    run(program.body());
    execution.setSpecLine(0);
    // Reading stops after the selected instance.
    if (!ignoresTrailing && !isSelectedRead && input.skipToData()) {
      String rest = input.string(input.position(), input.contentEnd(input.position()));
      throw execution.inputFailure(
          ErrorCode.TRAILING_INPUT, "unexpected input after the end of the spec: " + quote(rest));
    }
  }

  private void run(List<Stmt> block) {
    for (Stmt statement : block) {
      run(statement);
    }
  }

  private void run(Stmt statement) {
    execution.setSpecLine(statement.line());
    switch (statement) {
      case Stmt.Comment s -> input.addComments(s.prefixes());
      case Stmt.Stop s -> input.addStops(s.prefixes());
      case Stmt.Tokens s -> input.addSplit(s.characters());
      case Stmt.Separators s -> input.addSeparators(s.characters());
      case Stmt.IgnoreTrailing _ -> ignoresTrailing = true;
      case Stmt.Param s -> param(s);
      case Stmt.Declaration s -> declaration(s);
      case Stmt.ListRead s -> list(s);
      case Stmt.Line s -> line(s);
      case Stmt.Find s -> find(s);
      case Stmt.Skip s -> skipLines(s);
      case Stmt.Expect s -> expect(s);
      case Stmt.IndexBase s -> index(s);
      case Stmt.Repeat s -> repeat(s);
      case Stmt.Sections s -> sections(s);
      case Stmt.Instances s -> instances(s);
      case Stmt.TypeDefinition s -> domains.define(s);
      case Stmt.Let s -> execution.define(s.name(), evaluator.evaluate(s.value()));
      case Stmt.If s ->
          run(Evaluator.truth(evaluator.evaluate(s.condition())) ? s.body() : s.orElse());
      case Stmt.Check s -> check(s);
      case Stmt.Each _ -> {}
    }
  }

  /**
   * {@code param x [= default]}: a given value has the type of the default. Without a default, it
   * is an integer if possible, otherwise a float or a string (§9.1).
   */
  private void param(Stmt.Param s) {
    Expr defaultExpression = s.defaultValue();
    Stored defaultValue = defaultExpression == null ? null : evaluator.evaluate(defaultExpression);
    String given = params.get(s.name());
    if (given == null) {
      if (defaultValue == null) {
        throw execution.failure(
            ErrorCode.PARAM_MISSING, "parameter '" + s.name() + "' must be given");
      }
      execution.define(s.name(), defaultValue);
    } else {
      execution.define(s.name(), paramValue(s.name(), given, defaultValue));
    }
  }

  private Stored paramValue(String name, String given, @Nullable Stored defaultValue) {
    ValueType type = defaultValue == null ? null : defaultValue.type();
    if (type == ValueType.STRING) {
      return new Stored.Text(given);
    }
    byte[] bytes = given.getBytes(UTF_8);
    if (type != ValueType.FLOAT) {
      OptionalLong integer = Numbers.parseInteger(bytes, 0, bytes.length);
      if (integer.isPresent()) {
        return new Stored.Int(integer.getAsLong());
      }
    }
    if (type != ValueType.INT) {
      OptionalDouble real = Numbers.parseFloat(bytes, 0, bytes.length);
      if (real.isPresent()) {
        return new Stored.Real(real.getAsDouble());
      }
    }
    if (type == null) {
      return new Stored.Text(given);
    }
    throw execution.failure(
        ErrorCode.PARAM_TYPE,
        "parameter '"
            + name
            + "' must be "
            + (type == ValueType.INT ? "an integer" : "a number")
            + ", not "
            + quote(given));
  }

  private void store(String name, Stored value) {
    if (!name.equals("_")) {
      execution.define(name, value);
    }
  }

  /** {@code x: type} and {@code x[d1][d2]...: type}. */
  private void declaration(Stmt.Declaration s) {
    execution.enter(Context.of(s.name()));
    Expr offsetExpression = s.offset();
    long offset = offsetExpression == null ? 0 : evaluator.offset(offsetExpression);
    Domain domain = domains.of(s.type());
    if (s.dimensions().isEmpty()) {
      store(s.name(), reader.withOffset(reader.value(domain), offset));
    } else {
      int[] dimensions = new int[s.dimensions().size()];
      for (int k = 0; k < dimensions.length; k++) {
        dimensions[k] = evaluator.count(s.dimensions().get(k), "array size");
      }
      store(s.name(), arrayOf(s, domain, dimensions, offset));
    }
    execution.leave();
  }

  private Stored arrayOf(Stmt.Declaration s, Domain domain, int[] dimensions, long offset) {
    if (domain.kind() == ReadingKind.SYMBOL) {
      return reader.symbols(alphabet(s.type()), dimensions[0], dimensions[1]);
    }
    Expr layout = s.layout();
    if (layout == null) {
      return reader.array(domain, dimensions, offset);
    }
    if (evaluator.evaluate(layout) instanceof Stored.Text name) {
      return reader.layout(domain, dimensions[0], dimensions[1], name.value(), offset);
    }
    throw execution.failure(ErrorCode.LAYOUT, "matrix layout must be a string");
  }

  private ReadingType.Alphabet alphabet(ReadingType type) {
    return switch (type) {
      case ReadingType.Alphabet alphabet -> alphabet;
      case ReadingType.Named named -> resolution.alphabet(named);
      case ReadingType.Base _, ReadingType.Range _ ->
          throw new IllegalArgumentException("the type is not an alphabet");
    };
  }

  /** {@code list x: type until (eol | terminator)}. */
  private void list(Stmt.ListRead s) {
    execution.enter(Context.of(s.name()));
    Stored value = reader.list(s, domains.of(s.type()));
    execution.leave();
    store(s.name(), value);
  }

  /** {@code line "pattern"}: the rest of this line, or the next if this one is used up, matches. */
  private void line(Stmt.Line s) {
    if (!matcher.match(s.pattern())) {
      throw execution.failure(
          ErrorCode.LINE_MATCH,
          "expected a line matching \""
              + s.pattern().source()
              + "\""
              + (input.isAtEnd() ? ", found end of input" : ""));
    }
  }

  /** {@code find "pattern"}: skips lines until one matches. */
  private void find(Stmt.Find s) {
    while (!matcher.match(s.pattern())) {
      if (!input.skipToData()) {
        throw execution.failure(
            ErrorCode.FIND_MATCH, "no line matching \"" + s.pattern().source() + "\" found");
      }
      input.toEndOfLine();
    }
  }

  /** {@code skip e lines}. */
  private void skipLines(Stmt.Skip s) {
    int count = evaluator.count(s.count(), "number of lines");
    for (int k = 0; k < count; k++) {
      if (!input.skipToData()) {
        throw execution.failure(ErrorCode.SKIP_EOF, "cannot skip a line at end of input");
      }
      input.toEndOfLine();
    }
  }

  /** {@code expect "a" "b" ...}: literal tokens. */
  private void expect(Stmt.Expect s) {
    for (String token : s.tokens()) {
      if (!input.skipToData()) {
        throw execution.failure(
            ErrorCode.EXPECTED_TOKEN, "expected " + quote(token) + ", found end of input");
      }
      if (!input.isToken(token)) {
        String found = input.string(input.position(), input.tokenEnd());
        throw execution.failure(
            ErrorCode.EXPECTED_TOKEN, "expected " + quote(token) + ", found " + quote(found));
      }
      input.moveOnLine(input.tokenEnd());
    }
  }

  /** {@code index b}: the row number of the innermost repeat, counted from b. */
  private void index(Stmt.IndexBase s) {
    if (row < 0) {
      throw execution.failure(ErrorCode.INDEX_CONTEXT, "'index' must be inside a repeat");
    }
    // The base is at most the largest int, so the sum cannot overflow.
    long expected = (long) row + evaluator.count(s.base(), "index base");
    input.skip();
    int start = input.position();
    long found = reader.integer();
    if (found != expected) {
      input.moveOnLine(start);
      throw execution.failure(
          ErrorCode.INDEX_MISMATCH, "expected index " + expected + ", found " + found);
    }
  }

  /** Tests whether the next token ends a statement with {@code terminators}, and consumes it. */
  private boolean isEnded(@Nullable Terminators terminators) {
    if (terminators == null || !input.skipToData()) {
      return false;
    }
    for (String token : terminators.tokens()) {
      if (input.isToken(token)) {
        input.moveOnLine(input.tokenEnd());
        String store = terminators.store();
        if (store != null) {
          execution.define(store, new Stored.Text(token));
        }
        return true;
      }
    }
    return false;
  }

  /** Describes {@code terminators}, for messages. */
  private static String describe(Terminators terminators) {
    return terminators.tokens().stream().map(Execution::quote).collect(Collectors.joining(" or "));
  }

  /*
   * Rejections
   */

  /**
   * Tests whether the instance being read may not be the selected one. When selecting by name, it
   * is the selected one if it has the name, and may not be while the name is not defined yet. A
   * catalog selects no instance.
   */
  private boolean mayBeSkipped() {
    if (current == 0) {
      return false;
    }
    if (catalog != null) {
      return true;
    }
    String name = wanted;
    if (name == null) {
      return current != 1;
    }
    if (wantedNumber.isPresent()) {
      return current != wantedNumber.getAsLong();
    }
    return !instanceName(execution.frame(instanceFrame)).equals(name);
  }

  /**
   * Rejects the instance being read with {@code error}. Reading stops, unless the instance may not
   * be the selected one: then the first rejection is kept, and reading continues (see {@link
   * #instances}). A catalog keeps its {@code summary} instead of the error. Both are only made if
   * they are needed, as an instance can be rejected often.
   */
  private void reject(Supplier<IrisException> error, Supplier<String> summary) {
    if (!mayBeSkipped()) {
      throw error.get();
    }
    Catalog listing = catalog;
    if (listing != null) {
      listing.reject(summary);
    } else if (deferred == null) {
      deferred = error.get();
    }
  }

  /** {@code check e "message"}. */
  private void check(Stmt.Check s) {
    if (!evaluator.holds(s)) {
      reject(() -> evaluator.failure(s), () -> checkSummary(s));
    }
  }

  private static String checkSummary(Stmt.Check s) {
    String message = s.message();
    return message == null || message.isEmpty()
        ? "check on line " + s.line() + " of the spec"
        : message;
  }

  /*
   * Instances
   */

  /** Returns the name of the instance in {@code frame}, or the empty string. */
  private static String instanceName(Map<String, Stored> frame) {
    return switch (frame.get("name")) {
      case Stored.Text name -> name.value();
      case Stored.Int name -> Long.toString(name.value());
      case null, default -> "";
    };
  }

  /** Reads instance {@code number} into a frame, and returns it. */
  private SequencedMap<String, Stored> instance(List<Stmt> body, int number) {
    current = number;
    deferred = null;
    execution.enter(new Context("instance", number, -1));
    execution.pushFrame(new LinkedHashMap<>());
    instanceFrame = execution.innermostFrame();
    run(body);
    SequencedMap<String, Stored> frame = execution.popFrame();
    execution.leave();
    current = 0;
    return frame;
  }

  /**
   * Reads the instances of a file and keeps the selected one (§9.5).
   *
   * <p>Each instance is read into a frame of its own, which is dropped unless it is the one wanted.
   * The parameter {@code instance} selects an instance by number, from 1, if it is a number, and by
   * the value of {@code name} otherwise. Without it, the file must contain a single instance.
   * Reading stops after the selected instance.
   *
   * <p>A rejection of an instance that is not the selected one does not stop reading, so that an
   * invalid instance does not hide the ones after it. Other errors do, as the input cannot be read
   * on.
   *
   * <p>A catalog reads every instance and keeps none of them, whatever the selection says.
   */
  private void instances(Stmt.Instances s) {
    String selection = params.get("instance");
    wanted = selection;
    if (selection != null) {
      byte[] bytes = selection.getBytes(UTF_8);
      wantedNumber = Numbers.parseInteger(bytes, 0, bytes.length);
      if (wantedNumber.isPresent() && wantedNumber.getAsLong() < 1) {
        throw execution.failure(ErrorCode.INSTANCE_NUMBER, "instance numbers start at 1");
      }
    }
    int count = 0;
    while (input.skipToData()) {
      count++;
      int start = input.position();
      SequencedMap<String, Stored> frame = instance(s.body(), count);
      execution.setSpecLine(s.line());
      if (input.position() == start) {
        throw execution.failure(ErrorCode.NO_PROGRESS, "an instance must read some input");
      }
      if (catalog != null) {
        catalog.finishInstance(instanceName(frame));
      } else if (isChosen(count, frame, selection)) {
        // An instance selected by name may have been rejected before its name was known.
        IrisException rejection = deferred;
        if (rejection != null) {
          throw rejection;
        }
        frame.forEach(execution::define);
        if (selection != null) {
          isSelectedRead = true;
          return;
        }
      }
    }
    if (catalog != null) {
      catalog.setInstances(count);
      return;
    }
    if (count == 0) {
      throw execution.failure(ErrorCode.INSTANCE_EMPTY, "the file contains no instance");
    }
    if (selection != null) {
      throw execution.failure(
          ErrorCode.INSTANCE_NOT_FOUND,
          "there is no instance "
              + quote(selection)
              + " (the file has "
              + count
              + " instance"
              + (count == 1 ? "" : "s")
              + ")");
    }
    if (count > 1) {
      throw execution.failure(
          ErrorCode.INSTANCE_SELECTION,
          "the file has "
              + count
              + " instances; select one with the parameter instance (a number from 1, or a name)");
    }
  }

  private boolean isChosen(int number, Map<String, Stored> frame, @Nullable String selection) {
    if (selection == null) {
      return number == 1;
    }
    return wantedNumber.isPresent()
        ? wantedNumber.getAsLong() == number
        : instanceName(frame).equals(selection);
  }

  /*
   * Repeats
   */

  /**
   * Repeats the body of {@code s}. Each iteration runs in a frame of its own. Afterwards, the value
   * of every name the body defines is added as a row to the name's column, and the frame is
   * cleared, so that only one iteration's values exist at a time besides the columns.
   */
  private void repeat(Stmt.Repeat s) {
    List<Resolution.Declared> names = resolution.declared(s);
    Terminators terminators =
        s.count() instanceof RepeatCount.Until until ? until.terminators() : null;
    boolean untilEof = s.count() instanceof RepeatCount.UntilEof;
    int count =
        s.count() instanceof RepeatCount.Times times
            ? evaluator.count(times.count(), "repeat count")
            : -1;
    List<Column> columns = new ArrayList<>(names.size());
    for (Resolution.Declared name : names) {
      columns.add(new Column(name.type(), name.depth()));
    }
    // Where each record starts, for errors in checks.
    Leaves starts = new Leaves(ValueType.INT);
    SequencedMap<String, Stored> frame = new LinkedHashMap<>();
    int outerRow = row;
    for (int iteration = 0; count < 0 || iteration < count; iteration++) {
      if (untilEof && !input.skipToData()) {
        break;
      }
      if (isEnded(terminators)) {
        break;
      }
      if (terminators != null && !input.skipToData()) {
        throw execution.failure(
            ErrorCode.REPEAT_TERMINATOR,
            "expected " + describe(terminators) + " before end of input");
      }
      execution.enter(new Context("repeat", iteration + 1, count));
      execution.pushFrame(frame);
      row = iteration;
      starts.add((long) input.nextData());
      int start = input.position();
      run(s.body());
      // The errors after the body keep the line of its last statement and belong to the repeat.
      execution.setOwnerLine(s.line());
      if (input.position() == start) {
        if (count < 0) {
          throw execution.failure(
              ErrorCode.NO_PROGRESS, "the body of a repeat until read no input");
        }
        execution.chargeIdle(1);
      }
      row = outerRow;
      addRows(names, columns, frame);
      frame.clear();
      execution.popFrame();
      execution.leave();
      execution.setSpecLine(s.line());
    }
    for (int k = 0; k < columns.size(); k++) {
      String name = names.get(k).name();
      try {
        execution.define(name, columns.get(k).finish());
      } catch (ShapeError error) {
        throw columnFailure(name, error);
      }
    }
    execution.define(Resolution.startsName(s), Table.row(starts));
  }

  private void addRows(
      List<Resolution.Declared> names, List<Column> columns, Map<String, Stored> frame) {
    for (int k = 0; k < columns.size(); k++) {
      String name = names.get(k).name();
      Stored value = frame.get(name);
      try {
        if (value == null) {
          columns.get(k).skip();
        } else {
          columns.get(k).add(value);
        }
      } catch (ShapeError error) {
        throw columnFailure(name, error);
      }
    }
  }

  private IrisException columnFailure(String name, ShapeError error) {
    return execution.failure(error.code(), "'" + name + "' " + error.phrase());
  }

  /*
   * Sections
   */

  /**
   * {@code sections}: reads lines in any order, each by the first arm that matches it. An arm that
   * is not optional must match a line, and no arm may match two lines (§7.6).
   */
  private void sections(Stmt.Sections s) {
    List<Arm> arms = s.arms();
    // The line on which each arm matched, or 0.
    int[] seen = new int[arms.size()];
    String end = "the end of the input";
    Place endAt = input.here();
    while (true) {
      execution.setSpecLine(s.line());
      if (!input.skipToData()) {
        endAt = input.here();
        break;
      }
      Place at = input.here();
      if (isEnded(s.terminators())) {
        end = describe(Objects.requireNonNull(s.terminators()));
        endAt = at;
        break;
      }
      int found = matchingArm(arms);
      if (found < 0) {
        execution.setOwnerLine(0);
        if (s.skipOther()) {
          input.toEndOfLine();
          continue;
        }
        throw execution.failure(
            ErrorCode.SECTIONS_UNEXPECTED, "unexpected line; expected one of " + expected(arms));
      }
      Arm arm = arms.get(found);
      String source = arm.pattern().source();
      if (seen[found] > 0) {
        int first = seen[found];
        reject(
            () ->
                execution.located(
                    ErrorCode.SECTIONS_REPEATED,
                    "a second line matching \""
                        + source
                        + "\" (the first is on line "
                        + first
                        + ")",
                    at),
            () -> "a second line matching \"" + source + "\"");
      } else {
        seen[found] = at.line();
      }
      matcher.take();
      execution.enter(Context.of("section \"" + source + "\""));
      run(arm.body());
      execution.leave();
      if (arm.last()) {
        end = "the line matching \"" + source + "\"";
        endAt = at;
        break;
      }
    }
    requireArms(arms, seen, end, endAt);
  }

  /**
   * Returns the index of the first arm whose pattern matches the line, or -1. An arm owns the
   * errors of matching its pattern and, if it matches, of taking the captures.
   */
  private int matchingArm(List<Arm> arms) {
    for (int k = 0; k < arms.size(); k++) {
      execution.setOwnerLine(arms.get(k).line());
      if (matcher.matches(arms.get(k).pattern())) {
        return k;
      }
    }
    return -1;
  }

  private static String expected(List<Arm> arms) {
    StringBuilder text = new StringBuilder();
    for (int k = 0; k < arms.size() && k < 6; k++) {
      text.append(k > 0 ? ", \"" : "\"").append(arms.get(k).pattern().source()).append('"');
    }
    return arms.size() > 6 ? text + ", ..." : text.toString();
  }

  private void requireArms(List<Arm> arms, int[] seen, String end, Place endAt) {
    for (int k = 0; k < arms.size(); k++) {
      Arm arm = arms.get(k);
      if (!arm.optional() && seen[k] == 0) {
        execution.setOwnerLine(arm.line());
        String source = arm.pattern().source();
        Supplier<IrisException> error =
            () ->
                execution.located(
                    ErrorCode.LINE_MATCH,
                    "expected a line matching \"" + source + "\" before " + end,
                    endAt);
        // At the end of the input, no later instance can be hidden.
        if (!input.skipToData()) {
          throw error.get();
        }
        reject(error, () -> "no line matching \"" + source + "\"");
        return;
      }
    }
  }
}
