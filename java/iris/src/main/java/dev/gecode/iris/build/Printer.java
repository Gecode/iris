package dev.gecode.iris.build;

import dev.gecode.iris.build.BuildException.Kind;
import dev.gecode.iris.build.Declared.AlphabetBase;
import dev.gecode.iris.build.Declared.AlphabetBlank;
import dev.gecode.iris.build.Declared.AlphabetOption;
import dev.gecode.iris.build.Declared.AlphabetSpec;
import dev.gecode.iris.build.Declared.AlphabetStrip;
import dev.gecode.iris.build.Declared.BaseSpec;
import dev.gecode.iris.build.Declared.NamedSpec;
import dev.gecode.iris.build.Declared.RangeSpec;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Deque;
import java.util.HashMap;
import java.util.HashSet;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import org.jspecify.annotations.Nullable;

/** The printer. It writes what {@code iris fmt} writes with no line limit. */
final class Printer {
  /**
   * The emitted text, and where each of its statements came from.
   *
   * @param text the text
   * @param lines the line, counted from 1, on which each marked statement starts
   * @param sites the call site of each marked statement
   */
  record Printed(String text, List<Integer> lines, List<StackTraceElement> sites) {
    /** The call site of the first statement that starts on {@code line}, or before it. */
    @Nullable StackTraceElement siteOfLine(int line) {
      StackTraceElement before = null;
      for (int k = 0; k < lines.size(); k++) {
        if (lines.get(k) == line) {
          return sites.get(k);
        }
        if (lines.get(k) > line) {
          break;
        }
        before = sites.get(k);
      }
      return before;
    }
  }

  /**
   * The words that end a range with no upper bound, from the Specification's {@code upper bound
   * ahead} predicate.
   */
  static final Set<String> STOP_SET =
      Set.of(
          "comment",
          "stop",
          "expect",
          "tokens",
          "separators",
          "ignore",
          "param",
          "list",
          "line",
          "find",
          "skip",
          "index",
          "repeat",
          "each",
          "instances",
          "sections",
          "if",
          "let",
          "check",
          "type",
          "layout",
          "offset",
          "until");

  /** The words the type dispatch takes before a range. */
  static final Set<String> TYPE_WORDS = Set.of("int", "float", "num", "word", "rest", "alphabet");

  // The Specification's ten precedence levels, loosest first.
  static final int DISJUNCTION = 1;
  static final int CONJUNCTION = 2;
  static final int EQUALITY = 3;
  static final int RELATION = 4;
  static final int ADDITIVE = 5;
  static final int MULTIPLICATIVE = 6;
  static final int UNARY = 7;
  static final int POWER = 8;
  static final int POSTFIX = 9;
  static final int PRIMARY = 10;

  private static int binaryLevel(String symbol) {
    return switch (symbol) {
      case "||" -> DISJUNCTION;
      case "&&" -> CONJUNCTION;
      case "==", "!=" -> EQUALITY;
      case "<", "<=", ">", ">=" -> RELATION;
      case "+", "-" -> ADDITIVE;
      case "*", "/", "%" -> MULTIPLICATIVE;
      case "^" -> POWER;
      default -> throw new IllegalArgumentException(symbol);
    };
  }

  private static int level(Op op) {
    return switch (op) {
      case Op.BinaryOp binary -> binaryLevel(binary.symbol());
      case Op.IntOp literal ->
          literal.value() < 0 && literal.value() != Long.MIN_VALUE ? UNARY : PRIMARY;
      case Op.FloatOp literal -> Math.copySign(1.0, literal.value()) < 0 ? UNARY : PRIMARY;
      case Op.UnaryOp unary -> UNARY;
      case Op.IndexOp index -> POSTFIX;
      case Op.MemberOp member -> POSTFIX;
      case Op.ListMemberOp member -> POSTFIX;
      default -> PRIMARY;
    };
  }

  /** Where each subexpression starts, indexed by the operation that ends it. */
  private static int[] extents(List<Op> ops) {
    int[] starts = new int[ops.size()];
    int[] stack = new int[ops.size()];
    int height = 0;
    for (int index = 0; index < ops.size(); index++) {
      int first = height - ops.get(index).operands();
      int start = first < height ? stack[first] : index;
      height = first;
      stack[height++] = start;
      starts[index] = start;
    }
    return starts;
  }

  /** The last operation of each of the {@code count} operands before {@code end}. */
  private static int[] operands(int[] starts, int end, int count) {
    int[] ends = new int[count];
    int following = end;
    for (int k = count - 1; k >= 0; k--) {
      following--;
      ends[k] = following;
      following = starts[following];
    }
    return ends;
  }

  static String string(String text) {
    StringBuilder out = new StringBuilder(text.length() + 2).append('"');
    for (int k = 0; k < text.length(); k++) {
      char c = text.charAt(k);
      switch (c) {
        case '\\' -> out.append("\\\\");
        case '"' -> out.append("\\\"");
        case '\n' -> out.append("\\n");
        case '\t' -> out.append("\\t");
        default -> out.append(c);
      }
    }
    return out.append('"').toString();
  }

  private static String firstWord(String text) {
    int end = 0;
    while (end < text.length() && isWordCharacter(text.charAt(end))) {
      end++;
    }
    return text.substring(0, end);
  }

  private static boolean isWordCharacter(char c) {
    return (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') || c == '_';
  }

  /** Whether {@code name} is an Iris name. */
  static boolean isIdentifier(String name) {
    if (name.isEmpty()) {
      return false;
    }
    char first = name.charAt(0);
    if (!((first >= 'A' && first <= 'Z') || (first >= 'a' && first <= 'z') || first == '_')) {
      return false;
    }
    for (int k = 1; k < name.length(); k++) {
      if (!isWordCharacter(name.charAt(k))) {
        return false;
      }
    }
    return true;
  }

  /**
   * Whether a format name can be written without quotes: names and integers glued with {@code -} or
   * {@code .}, starting with a name.
   */
  static boolean isBareFormat(String name) {
    String rest = name;
    boolean first = true;
    while (true) {
      int dash = rest.indexOf('-');
      int dot = rest.indexOf('.');
      int end = rest.length();
      if (dash >= 0) {
        end = Math.min(end, dash);
      }
      if (dot >= 0) {
        end = Math.min(end, dot);
      }
      String part = rest.substring(0, end);
      String tail = rest.substring(end);
      boolean integer = isInteger(part);
      if (!(isIdentifier(part) || (!first && integer && !tail.startsWith(".")))) {
        return false;
      }
      if (tail.isEmpty()) {
        return true;
      }
      rest = tail.substring(1);
      first = false;
    }
  }

  private static boolean isInteger(String part) {
    if (part.isEmpty()) {
      return false;
    }
    for (int k = 0; k < part.length(); k++) {
      char c = part.charAt(k);
      if (c < '0' || c > '9') {
        return false;
      }
    }
    String digits = part.replaceFirst("^0+(?=.)", "");
    String max = Long.toString(Long.MAX_VALUE);
    return digits.length() < max.length()
        || (digits.length() == max.length() && digits.compareTo(max) <= 0);
  }

  /** Internal names, chosen after the whole stream is known. */
  private static final class Names {
    final Map<Integer, String> each = new HashMap<>();
    final Map<Integer, String> list = new HashMap<>();
    final IdentityHashMap<Op.Binding, String> bindings = new IdentityHashMap<>();

    /**
     * Choose names in stream order, skipping every name in the spec, so the same calls always give
     * the same text.
     */
    static Names assign(State state) {
      Set<Integer> needsEach = new HashSet<>();
      Set<Integer> needsList = new HashSet<>();
      Set<String> taken = state.taken();
      for (Stmt stmt : state.stmts) {
        for (List<Op> ops : expressions(stmt)) {
          for (Op op : ops) {
            switch (op) {
              case Op.RecordOp r -> needsEach.add(state.recordIndex(r.record()));
              case Op.RecordListOp r -> needsList.add(state.recordIndex(r.record()));
              case Op.ListMemberOp r -> needsList.add(state.recordIndex(r.record()));
              case Op.ComprehensionOp c -> {
                if (c.name() != null) {
                  taken.add(c.name());
                }
              }
              default -> {}
            }
          }
        }
      }
      Fresh fresh = new Fresh(taken);
      Names names = new Names();
      for (Stmt stmt : state.stmts) {
        if (stmt instanceof Stmt.Repeat repeat) {
          State.RecordInfo info = state.records.get(repeat.record());
          if (info.each == null && needsEach.contains(repeat.record())) {
            names.each.put(repeat.record(), fresh.name("record"));
          }
          if (info.list == null && needsList.contains(repeat.record())) {
            names.list.put(repeat.record(), fresh.name("records"));
          }
        }
        for (List<Op> ops : expressions(stmt)) {
          names.nameBindings(ops, fresh);
        }
      }
      return names;
    }

    void nameBindings(List<Op> ops, Fresh fresh) {
      for (Op op : ops) {
        if (op instanceof Op.ComprehensionOp c && c.name() != null) {
          bindings.put(c.binding(), c.name());
        }
      }
      for (Op op : ops) {
        Op.Binding binding =
            switch (op) {
              case Op.ComprehensionOp c -> c.binding();
              case Op.BindingOp b -> b.binding();
              default -> null;
            };
        if (binding != null && !bindings.containsKey(binding)) {
          bindings.put(binding, fresh.name("element"));
        }
      }
    }
  }

  private static List<List<Op>> expressions(Stmt stmt) {
    List<List<Op>> out = new ArrayList<>();
    for (Mention mention : Stmt.mentions(stmt)) {
      if (mention instanceof Mention.Expression expression) {
        out.add(expression.ops());
      }
    }
    return out;
  }

  private static final class Fresh {
    private final Set<String> taken;
    private final Map<String, Integer> next = new HashMap<>();

    Fresh(Set<String> taken) {
      this.taken = taken;
    }

    String name(String prefix) {
      int number = next.getOrDefault(prefix, 1);
      while (true) {
        String name = prefix + "_" + number;
        number++;
        if (taken.add(name)) {
          next.put(prefix, number);
          return name;
        }
      }
    }
  }

  /** A pending piece of an expression's text. */
  private sealed interface Task {}

  private record TextTask(String text) implements Task {}

  private record ExpressionTask(int end, int minimum) implements Task {}

  /** {@code .name} after the operand just written. */
  private record MemberTask(String name) implements Task {}

  private final State state;
  private final Names names;
  private final StringBuilder out = new StringBuilder();
  private final List<Integer> starts = new ArrayList<>();
  private final List<StackTraceElement> sites = new ArrayList<>();
  private int current;
  private int indent;
  private @Nullable BuildException issue;

  private Printer(State state) {
    this.state = state;
    this.names = Names.assign(state);
  }

  /** Print the statements as text. */
  static Printed print(State state) {
    Printer printer = new Printer(state);
    printer.statements();
    if (printer.issue != null) {
      throw printer.issue;
    }
    String text = printer.out.toString();
    List<Integer> lines = new ArrayList<>(printer.starts.size());
    int line = 1;
    int position = 0;
    for (int start : printer.starts) {
      for (; position < start; position++) {
        if (text.charAt(position) == '\n') {
          line++;
        }
      }
      lines.add(line);
    }
    return new Printed(text, List.copyOf(lines), List.copyOf(printer.sites));
  }

  private void write(String text) {
    out.append(text);
  }

  private void markSite(int index) {
    starts.add(out.length());
    sites.add(state.sites.get(index));
  }

  private void unprintable(String message) {
    if (issue == null) {
      issue = BuildException.check(Kind.UNPRINTABLE, message, state.sites.get(current));
    }
  }

  private void startLine() {
    write("  ".repeat(indent));
  }

  private void statements() {
    List<Stmt> stmts = state.stmts;
    int index = 0;
    boolean continuing = false;
    while (index < stmts.size()) {
      Stmt stmt = stmts.get(index);
      current = index;
      markSite(index);
      index++;
      if (stmt instanceof Stmt.Close) {
        indent--;
        startLine();
        write("}");
        continuing = afterClose(stmts, index);
        continue;
      }
      if (!continuing) {
        startLine();
      }
      continuing = false;
      if (stmt instanceof Stmt.Format && index >= 2 && endsOpen(stmts.get(index - 2))) {
        unprintable(
            "a range with no upper bound cannot come directly before `format`: `format` would be"
                + " its upper bound");
      }
      statement(stmt);
      if (!Stmt.opensBlock(stmt)) {
        write("\n");
        continue;
      }
      String each = stmt instanceof Stmt.Repeat repeat ? recordEach(repeat.record()) : null;
      if (each == null && index < stmts.size() && stmts.get(index) instanceof Stmt.Close) {
        write(" {");
        markSite(index);
        write("}");
        index++;
        continuing = afterClose(stmts, index);
        continue;
      }
      write(" {\n");
      indent++;
      if (each != null) {
        startLine();
        write("each " + each + "\n");
      }
    }
  }

  /** End the line after {@code }}, unless an {@code else} continues it. */
  private boolean afterClose(List<Stmt> stmts, int following) {
    boolean continues = following < stmts.size() && Stmt.continuesIf(stmts.get(following));
    write(continues ? " " : "\n");
    return continues;
  }

  private @Nullable String recordEach(int record) {
    String name = state.records.get(record).each;
    return name != null ? name : names.each.get(record);
  }

  private @Nullable String recordList(int record) {
    String name = state.records.get(record).list;
    return name != null ? name : names.list.get(record);
  }

  private void statement(Stmt stmt) {
    switch (stmt) {
      case Stmt.Format f ->
          write("format " + (isBareFormat(f.name()) ? f.name() : string(f.name())));
      case Stmt.Strings s -> {
        write(s.keyword());
        strings(s.strings());
      }
      case Stmt.IgnoreTrailing i -> write("ignore trailing");
      case Stmt.Param p -> {
        write("param ");
        target(p.definition());
        if (p.defaultValue() != null) {
          write(" = ");
          expression(p.defaultValue());
        }
      }
      case Stmt.Declaration d -> {
        if ("format".equals(d.definition().name()) && indent == 0) {
          unprintable(
              "an entry named `format` has no spelling at the top level: use a `let` or a"
                  + " `list`, or another name");
        }
        declaration(d.definition(), d.declared());
      }
      case Stmt.ElseDeclaration e -> {
        write("else ");
        declaration(e.definition(), e.declared());
      }
      case Stmt.ListStmt l -> list(l);
      case Stmt.TypeDefinition t -> {
        write("type " + t.name() + " = ");
        typeWithOptions(t.declared());
      }
      case Stmt.Line l -> {
        write(l.keyword() + " ");
        pattern(l.pieces());
      }
      case Stmt.SkipLine s -> write("skip line");
      case Stmt.SkipLines s -> {
        write("skip ");
        guarded(s.count(), Set.of("line"));
        write(" lines");
      }
      case Stmt.Index i -> {
        write("index ");
        expression(i.position());
      }
      case Stmt.Let l -> {
        write("let ");
        target(l.definition());
        write(" = ");
        expression(l.value());
      }
      case Stmt.Check c -> {
        write("check ");
        expression(c.condition());
        if (c.message() != null) {
          write(" " + string(c.message()));
        }
      }
      case Stmt.Repeat r -> repeat(r);
      case Stmt.Sections s -> sections(s);
      case Stmt.Arm a -> {
        pattern(a.pieces());
        write(a.optional() ? " optional" : "");
        write(a.last() ? " last" : "");
      }
      case Stmt.If i -> {
        write("if ");
        expression(i.condition());
      }
      case Stmt.ElseIf e -> {
        write("else if ");
        expression(e.condition());
      }
      case Stmt.Else e -> write("else");
      case Stmt.Instances i -> write("instances");
      case Stmt.Close c -> throw new IllegalStateException("a close is not a statement");
    }
  }

  private void strings(List<String> strings) {
    for (String text : strings) {
      write(" " + string(text));
    }
  }

  private void target(Stmt.Definition definition) {
    write(definition.name() != null ? definition.name() : "_");
  }

  private void declaration(Stmt.Definition definition, Declared declared) {
    target(definition);
    for (List<Op> dimension : declared.dimensions()) {
      write("[");
      expression(dimension);
      write("]");
    }
    write(": ");
    typeWithOptions(declared);
  }

  private void list(Stmt.ListStmt stmt) {
    if (stmt.declared().layout() != null) {
      unprintable("a `list` has no `layout`");
    }
    write("list ");
    target(stmt.definition());
    write(": ");
    typeName(stmt.declared());
    write(" until ");
    if (stmt.end() == null) {
      write("eol");
    } else {
      guarded(stmt.end(), Set.of("eol"));
    }
    options(stmt.declared());
  }

  private void typeWithOptions(Declared declared) {
    typeName(declared);
    options(declared);
  }

  private void options(Declared declared) {
    if (declared.layout() != null) {
      write(" layout ");
      expression(declared.layout());
    }
    if (declared.offset() != null) {
      write(" offset ");
      expression(declared.offset());
    }
  }

  private void typeName(Declared declared) {
    switch (declared.type()) {
      case BaseSpec base -> write(base.name());
      case NamedSpec named -> write(named.name());
      case AlphabetSpec alphabet -> {
        write("alphabet " + string(alphabet.symbols()));
        for (AlphabetOption option : alphabet.options()) {
          alphabetOption(option);
        }
      }
      case RangeSpec range -> {
        if (range.lower() != null) {
          lowerBound(range.lower());
        }
        write(range.inclusive() ? ".." : "..<");
        if (range.upper() != null) {
          guarded(range.upper(), STOP_SET);
        }
      }
    }
  }

  private void alphabetOption(AlphabetOption option) {
    switch (option) {
      case AlphabetBlank blank -> write(" blank " + string(blank.characters()));
      case AlphabetStrip strip -> write(" strip " + string(strip.characters()));
      case AlphabetBase base -> {
        if (base.value() == Long.MIN_VALUE) {
          unprintable("the alphabet base -9223372036854775808 has no spelling");
        } else {
          write(" base " + base.value());
        }
      }
    }
  }

  private void lowerBound(List<Op> ops) {
    String text = expressionText(ops);
    String word = firstWord(text);
    write(TYPE_WORDS.contains(word) || state.types.contains(word) ? "(" + text + ")" : text);
  }

  /**
   * Write an expression in parentheses if its first word is one of {@code words}, which would
   * otherwise be read as a keyword.
   */
  private void guarded(List<Op> ops, Set<String> words) {
    String text = expressionText(ops);
    write(words.contains(firstWord(text)) ? "(" + text + ")" : text);
  }

  private String expressionText(List<Op> ops) {
    int mark = out.length();
    expression(ops);
    String text = out.substring(mark);
    out.setLength(mark);
    return text;
  }

  private void repeat(Stmt.Repeat stmt) {
    write("repeat ");
    switch (stmt.count()) {
      case Stmt.Times times -> guarded(times.count(), Set.of("until"));
      case Stmt.UntilEof eof -> write("until eof");
      case Stmt.Terminated terminated -> {
        write("until");
        strings(terminated.terminators());
        store(terminated.store());
      }
    }
    String records = recordList(stmt.record());
    if (records != null) {
      write(" as " + records);
    }
  }

  private void store(Stmt.@Nullable Definition store) {
    if (store != null) {
      write(" store ");
      target(store);
    }
  }

  private void sections(Stmt.Sections stmt) {
    write("sections");
    for (Stmt.Clause clause : stmt.clauses()) {
      switch (clause) {
        case Stmt.SkipOtherClause skip -> write(" skip other");
        case Stmt.Terminated terminated -> {
          write(" until");
          strings(terminated.terminators());
          store(terminated.store());
        }
      }
    }
  }

  private @Nullable String captureType(@Nullable Declared capture) {
    if (capture == null) {
      return null;
    }
    int mark = out.length();
    typeWithOptions(capture);
    String text = out.substring(mark);
    out.setLength(mark);
    if (text.contains("}")) {
      unprintable("the capture type `" + text + "` contains `}`");
    }
    return text;
  }

  private void pattern(List<Piece> pieces) {
    StringBuilder text = new StringBuilder();
    for (Piece piece : pieces) {
      switch (piece) {
        case Piece.Text t -> {
          if (t.text().contains("{") || t.text().contains("...")) {
            unprintable(
                "the pattern text `"
                    + t.text()
                    + "` contains `{` or `...`, which have no literal spelling");
          }
          text.append(t.text());
        }
        case Piece.Bind bind -> text.append(namedCapture(bind.handle().name(), bind.capture()));
        case Piece.Reference reference ->
            text.append("{$").append(reference.handle().name()).append('}');
        case Piece.Validate validate -> text.append(namedCapture("", validate.capture()));
        case Piece.Blank blank -> text.append("{}");
        case Piece.DiscardPiece discard -> text.append("{_}");
        case Piece.DiscardAs discard -> text.append(namedCapture("_", discard.capture()));
        case Piece.RestOfLine rest -> text.append("...");
      }
    }
    write(string(text.toString()));
  }

  private String namedCapture(String name, @Nullable Declared capture) {
    String type = captureType(capture);
    return type == null ? "{" + name + "}" : "{" + name + ":" + type + "}";
  }

  private void literal(Op op) {
    switch (op) {
      case Op.IntOp i ->
          write(
              i.value() == Long.MIN_VALUE
                  ? "(-9223372036854775807 - 1)"
                  : Long.toString(i.value()));
      case Op.FloatOp f -> {
        if (Double.isFinite(f.value())) {
          write(FloatText.of(f.value()));
        } else {
          unprintable("the float " + FloatText.nonFinite(f.value()) + " has no spelling");
        }
      }
      case Op.StrOp s -> write(string(s.text()));
      default -> throw new IllegalArgumentException("not a literal");
    }
  }

  /**
   * Write {@code .name} after the operand just written. {@code gecode.x} is one host constant
   * token, so after a whole name {@code gecode} the dot keeps its spaces.
   */
  private void member(String name) {
    int length = out.length();
    boolean whole =
        length >= 6
            && out.substring(length - 6).equals("gecode")
            && (length == 6 || !isWordCharacter(out.charAt(length - 7)));
    write((whole ? " . " : ".") + name);
  }

  private void expression(List<Op> ops) {
    int[] starts = extents(ops);
    Deque<Task> tasks = new ArrayDeque<>();
    tasks.push(new ExpressionTask(ops.size() - 1, DISJUNCTION));
    while (!tasks.isEmpty()) {
      switch (tasks.pop()) {
        case TextTask t -> write(t.text());
        case ExpressionTask e -> {
          if (level(ops.get(e.end())) < e.minimum()) {
            write("(");
            tasks.push(new TextTask(")"));
          }
          operation(ops, starts, e.end(), tasks);
        }
        case MemberTask m -> member(m.name());
      }
    }
  }

  private void operation(List<Op> ops, int[] starts, int end, Deque<Task> tasks) {
    Op op = ops.get(end);
    switch (op) {
      case Op.BinaryOp binary -> {
        int[] sides = operands(starts, end, 2);
        int level = binaryLevel(binary.symbol());
        boolean power = binary.symbol().equals("^");
        tasks.push(new ExpressionTask(sides[1], power ? UNARY : level + 1));
        tasks.push(new TextTask(" " + binary.symbol() + " "));
        tasks.push(new ExpressionTask(sides[0], power ? POSTFIX : level));
      }
      case Op.NameOp name -> write(name.handle().name());
      case Op.IntOp i -> literal(i);
      case Op.FloatOp f -> literal(f);
      case Op.StrOp s -> literal(s);
      case Op.HostOp host -> write(host.name());
      case Op.BindingOp binding -> write(bindingName(binding.binding()));
      case Op.RecordOp record -> write(nonNull(recordEach(state.recordIndex(record.record()))));
      case Op.RecordListOp record -> write(nonNull(recordList(state.recordIndex(record.record()))));
      case Op.UnaryOp unary -> {
        write(unary.symbol());
        tasks.push(new ExpressionTask(end - 1, UNARY));
      }
      case Op.IndexOp index -> {
        int[] parts = operands(starts, end, 2);
        tasks.push(new TextTask("]"));
        tasks.push(new ExpressionTask(parts[1], DISJUNCTION));
        tasks.push(new TextTask("["));
        tasks.push(new ExpressionTask(parts[0], POSTFIX));
      }
      case Op.MemberOp member -> {
        tasks.push(new MemberTask(member.handle().name()));
        memberBase(ops, end - 1, tasks);
      }
      case Op.ListMemberOp member -> {
        tasks.push(new MemberTask(nonNull(recordList(state.recordIndex(member.record())))));
        memberBase(ops, end - 1, tasks);
      }
      case Op.CallOp call -> {
        write(call.function() + "(");
        tasks.push(new TextTask(")"));
        int[] arguments = operands(starts, end, call.arguments());
        for (int position = arguments.length - 1; position >= 0; position--) {
          tasks.push(new ExpressionTask(arguments[position], DISJUNCTION));
          if (position > 0) {
            tasks.push(new TextTask(", "));
          }
        }
      }
      case Op.ComprehensionOp comprehension -> {
        int[] parts = operands(starts, end, 2);
        String name = bindingName(comprehension.binding());
        if (!isIdentifier(name)) {
          unprintable("`" + name + "` is not an Iris name");
        }
        write(comprehension.function() + "(" + name + " in ");
        tasks.push(new TextTask(")"));
        tasks.push(new ExpressionTask(parts[1], DISJUNCTION));
        tasks.push(new TextTask(": "));
        tasks.push(new ExpressionTask(parts[0], DISJUNCTION));
      }
    }
  }

  private String bindingName(Op.Binding binding) {
    return nonNull(names.bindings.get(binding));
  }

  private static String nonNull(@Nullable String name) {
    return name == null ? "" : name;
  }

  /** A number before {@code .} would lex as a malformed number, so it takes parentheses. */
  private void memberBase(List<Op> ops, int base, Deque<Task> tasks) {
    Op op = ops.get(base);
    int minimum = op instanceof Op.IntOp || op instanceof Op.FloatOp ? PRIMARY + 1 : POSTFIX;
    tasks.push(new ExpressionTask(base, minimum));
  }

  private static boolean endsOpen(Stmt stmt) {
    return switch (stmt) {
      case Stmt.Declaration d -> d.declared().isOpenEnded();
      case Stmt.ElseDeclaration e -> e.declared().isOpenEnded();
      case Stmt.TypeDefinition t -> t.declared().isOpenEnded();
      default -> false;
    };
  }
}
