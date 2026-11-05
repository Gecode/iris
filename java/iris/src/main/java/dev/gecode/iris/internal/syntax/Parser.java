package dev.gecode.iris.internal.syntax;

import static java.nio.charset.StandardCharsets.UTF_8;
import static java.util.Map.entry;

import dev.gecode.iris.ErrorCode;
import dev.gecode.iris.IrisException;
import dev.gecode.iris.internal.syntax.Ast.Arm;
import dev.gecode.iris.internal.syntax.Ast.BinaryOperator;
import dev.gecode.iris.internal.syntax.Ast.Binding;
import dev.gecode.iris.internal.syntax.Ast.ComprehensionFunction;
import dev.gecode.iris.internal.syntax.Ast.Expr;
import dev.gecode.iris.internal.syntax.Ast.Function;
import dev.gecode.iris.internal.syntax.Ast.Pattern;
import dev.gecode.iris.internal.syntax.Ast.Program;
import dev.gecode.iris.internal.syntax.Ast.ReadingKind;
import dev.gecode.iris.internal.syntax.Ast.ReadingType;
import dev.gecode.iris.internal.syntax.Ast.RepeatCount;
import dev.gecode.iris.internal.syntax.Ast.Stmt;
import dev.gecode.iris.internal.syntax.Ast.Terminators;
import dev.gecode.iris.internal.syntax.Ast.UnaryOperator;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.SequencedMap;
import java.util.Set;
import org.jspecify.annotations.Nullable;

/**
 * Parses the tokens of a reader spec, following {@code grammar.ebnf}.
 *
 * <p>Failures in parsing itself are reported here, in source order. The checks that need the whole
 * spec are the {@link Resolver}'s.
 */
public final class Parser {
  /** The limit of the block and expression counters (§5.12). */
  private static final int MAX_NESTING = 256;

  private static final Set<String> SETTINGS =
      Set.of("comment", "stop", "tokens", "separators", "ignore");

  /** The names that end a range without an upper bound (§5.7). */
  private static final Set<String> RANGE_STOPS =
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

  private static final Map<String, Level> BINARY_LEVELS =
      Map.ofEntries(
          entry("||", new Level(BinaryOperator.OR, 1)),
          entry("&&", new Level(BinaryOperator.AND, 2)),
          entry("==", new Level(BinaryOperator.EQUAL, 3)),
          entry("!=", new Level(BinaryOperator.NOT_EQUAL, 3)),
          entry("<", new Level(BinaryOperator.LESS, 4)),
          entry(">", new Level(BinaryOperator.GREATER, 4)),
          entry("<=", new Level(BinaryOperator.LESS_EQUAL, 4)),
          entry(">=", new Level(BinaryOperator.GREATER_EQUAL, 4)),
          entry("+", new Level(BinaryOperator.ADD, 5)),
          entry("-", new Level(BinaryOperator.SUBTRACT, 5)),
          entry("*", new Level(BinaryOperator.MULTIPLY, 6)),
          entry("/", new Level(BinaryOperator.DIVIDE, 6)),
          entry("%", new Level(BinaryOperator.REMAINDER, 6)));

  private final @Nullable String origin;
  private List<Token> tokens;
  private int position = 0;

  /** The two independent counters of §5.12. */
  private int blockDepth = 0;

  private int expressionDepth = 0;
  private int repeatsParsed = 0;
  private int checksParsed = 0;
  private int sites = 0;

  /** Whether the next statement is the first of the body of a repeat. */
  private boolean atRepeatStart = false;

  /** Whether the statement being parsed may be {@code each}. */
  private boolean eachAllowed = false;

  private final List<String> enclosingBindings = new ArrayList<>();
  private final List<Binding> completedBindings = new ArrayList<>();

  /** The visible named types by name, in definition order. A name is visible only once. */
  private final SequencedMap<String, Stmt.TypeDefinition> visibleTypes = new LinkedHashMap<>();

  private final List<Stmt.TypeDefinition> definedTypes = new ArrayList<>();

  private Parser(List<Token> tokens, @Nullable String origin) {
    this.tokens = tokens;
    this.origin = origin;
  }

  /** Parses {@code tokens}, the tokens of a whole reader spec from {@code origin}. */
  public static Program parse(List<Token> tokens, @Nullable String origin) {
    return new Parser(tokens, origin).program();
  }

  /** A binary operator and its precedence. */
  private record Level(BinaryOperator operator, int precedence) {}

  private Program program() {
    @Nullable String format = null;
    List<Stmt> body = new ArrayList<>();
    while (peek().kind() != Token.Kind.END) {
      if (atKeyword("format")) {
        format = formatName();
      } else if (!body.isEmpty() && body.getLast() instanceof Stmt.Instances) {
        throw failure(
            ErrorCode.SPEC_PLACEMENT, "'instances' must be the last statement of the spec");
      } else {
        body.add(statement());
      }
    }
    return new Program(
        format,
        List.copyOf(body),
        List.copyOf(completedBindings),
        List.copyOf(definedTypes),
        sites,
        checksParsed,
        repeatsParsed);
  }

  /** {@code format name}, whose words can be joined by glued {@code -} or {@code .} (§5.9). */
  private String formatName() {
    next();
    if (peek().kind() != Token.Kind.IDENTIFIER && peek().kind() != Token.Kind.STRING) {
      throw failure(ErrorCode.SPEC_SYNTAX, "expected a format name, found " + describe(peek()));
    }
    var name = new StringBuilder(next().text());
    while (atNameJoiner()) {
      name.append(next().text());
      name.append(next().text());
    }
    return name.toString();
  }

  private boolean atNameJoiner() {
    if (!(atPunctuation("-") || atPunctuation(".")) || !peek().glued()) {
      return false;
    }
    Token following = tokens.get(position + 1);
    return following.glued()
        && (following.kind() == Token.Kind.IDENTIFIER || following.kind() == Token.Kind.INTEGER);
  }

  private Token peek() {
    return tokens.get(position);
  }

  private Token next() {
    Token token = peek();
    if (position + 1 < tokens.size()) {
      position++;
    }
    return token;
  }

  private IrisException failure(ErrorCode code, String message) {
    return failureAt(code, peek().line(), message);
  }

  private IrisException failureAt(ErrorCode code, int line, String message) {
    return SpecErrors.at(code, origin, line, message);
  }

  private static String describe(Token token) {
    return switch (token.kind()) {
      case END -> "end of spec";
      case STRING -> "\"" + token.text() + "\"";
      case IDENTIFIER, INTEGER, FLOAT, PUNCTUATION -> "'" + token.text() + "'";
    };
  }

  private boolean atKeyword(String keyword) {
    return peek().kind() == Token.Kind.IDENTIFIER && peek().text().equals(keyword);
  }

  private boolean atPunctuation(String text) {
    return isPunctuation(peek(), text);
  }

  private static boolean isPunctuation(Token token, String text) {
    return token.kind() == Token.Kind.PUNCTUATION && token.text().equals(text);
  }

  private void expectPunctuation(String text) {
    if (!atPunctuation(text)) {
      throw failure(ErrorCode.SPEC_SYNTAX, "expected '" + text + "', found " + describe(peek()));
    }
    next();
  }

  private void expectKeyword(String keyword) {
    if (!atKeyword(keyword)) {
      throw failure(ErrorCode.SPEC_SYNTAX, "expected '" + keyword + "', found " + describe(peek()));
    }
    next();
  }

  /** Consumes {@code keyword} if it comes next, and returns whether it did. */
  private boolean accept(String keyword) {
    if (!atKeyword(keyword)) {
      return false;
    }
    next();
    return true;
  }

  /**
   * Consumes option {@code keyword} if it comes next, and returns whether it did. A declaration
   * with the same name ends the statement instead.
   */
  private boolean acceptOption(String keyword) {
    return !atDeclaration() && accept(keyword);
  }

  private String identifier(String what) {
    if (peek().kind() != Token.Kind.IDENTIFIER) {
      throw failure(ErrorCode.SPEC_SYNTAX, "expected " + what + ", found " + describe(peek()));
    }
    return next().text();
  }

  /**
   * Returns a name that the spec defines. The check follows the name, so a failure reports the line
   * of the next token (§5.4).
   */
  private String newName(String what) {
    String name = identifier(what);
    if (name.startsWith(Lexer.HOST_PREFIX)) {
      throw failure(
          ErrorCode.SPEC_NAME,
          "'"
              + name
              + "' cannot be defined: names starting with '"
              + Lexer.HOST_PREFIX
              + "' are Gecode constants");
    }
    return name;
  }

  /** Returns a name for a value that is kept, so not {@code _}. */
  private String keptName(String what) {
    String name = newName(what);
    if (name.equals("_")) {
      throw failure(ErrorCode.SPEC_NAME, "'_' cannot be defined here");
    }
    return name;
  }

  private String string(String what) {
    if (peek().kind() != Token.Kind.STRING) {
      throw failure(
          ErrorCode.SPEC_SYNTAX, "expected " + what + " (a string), found " + describe(peek()));
    }
    return next().text();
  }

  private List<String> strings(String what) {
    List<String> strings = new ArrayList<>();
    strings.add(string(what));
    while (peek().kind() == Token.Kind.STRING) {
      strings.add(next().text());
    }
    return List.copyOf(strings);
  }

  private void enterBlock() {
    if (++blockDepth > MAX_NESTING) {
      throw failure(ErrorCode.SPEC_NESTING, "blocks are nested too deeply");
    }
  }

  private void enterExpression() {
    if (++expressionDepth > MAX_NESTING) {
      throw failure(ErrorCode.SPEC_NESTING, "expressions are nested too deeply");
    }
  }

  private List<Stmt> block() {
    expectPunctuation("{");
    enterBlock();
    // The types defined in the block are only visible inside it.
    int outerTypes = visibleTypes.size();
    List<Stmt> statements = new ArrayList<>();
    while (!atPunctuation("}")) {
      if (peek().kind() == Token.Kind.END) {
        throw failure(ErrorCode.SPEC_SYNTAX, "missing '}'");
      }
      statements.add(statement());
    }
    next();
    while (visibleTypes.size() > outerTypes) {
      visibleTypes.pollLastEntry();
    }
    blockDepth--;
    return List.copyOf(statements);
  }

  /*
   * Statements
   */

  /** Tests whether a declaration starts here: a name, any dimensions, and a colon (§5.6). */
  private boolean atDeclaration() {
    if (peek().kind() != Token.Kind.IDENTIFIER) {
      return false;
    }
    int k = position + 1;
    while (isPunctuation(tokens.get(k), "[")) {
      k = afterBrackets(k);
      if (k < 0) {
        return false;
      }
    }
    return isPunctuation(tokens.get(k), ":");
  }

  /** Returns the index after the bracket group opening at {@code k}, or -1 if it does not end. */
  private int afterBrackets(int k) {
    int depth = 0;
    int index = k;
    do {
      Token token = tokens.get(index);
      if (token.kind() == Token.Kind.END) {
        return -1;
      }
      if (isPunctuation(token, "[")) {
        depth++;
      } else if (isPunctuation(token, "]")) {
        depth--;
      }
      index++;
    } while (depth > 0);
    return index;
  }

  private Stmt statement() {
    Token first = peek();
    eachAllowed = atRepeatStart;
    atRepeatStart = false;
    if (first.kind() != Token.Kind.IDENTIFIER) {
      throw failure(ErrorCode.SPEC_SYNTAX, "expected a statement, found " + describe(first));
    }
    if (atDeclaration()) {
      return declaration(first.line());
    }
    if (SETTINGS.contains(first.text()) && blockDepth > 0) {
      throw failure(
          ErrorCode.SPEC_PLACEMENT,
          "the setting '" + first.text() + "' must be at the top level of the spec");
    }
    next();
    return keywordStatement(first);
  }

  /** Parses the rest of the statement that starts with {@code keyword}. */
  private Stmt keywordStatement(Token keyword) {
    int line = keyword.line();
    return switch (keyword.text()) {
      case "comment" -> new Stmt.Comment(line, strings("a line prefix"));
      case "stop" -> new Stmt.Stop(line, strings("a line prefix"));
      case "expect" -> new Stmt.Expect(line, strings("a literal token"));
      case "tokens" -> new Stmt.Tokens(line, string("single-character tokens"));
      case "separators" -> new Stmt.Separators(line, string("separator characters"));
      case "ignore" -> ignoreTrailing(line);
      case "param" -> param(line);
      case "list" -> list(line);
      case "line" -> new Stmt.Line(line, pattern());
      case "find" -> new Stmt.Find(line, pattern());
      case "skip" -> skip(line);
      case "index" -> new Stmt.IndexBase(line, expression());
      case "repeat" -> repeat(line);
      case "each" -> each(line);
      case "instances" -> instances(line);
      case "sections" -> sections(line);
      case "if" -> conditional(line);
      case "let" -> let(line);
      case "check" -> check(line);
      case "type" -> typeDefinition(line);
      default ->
          throw failureAt(
              ErrorCode.SPEC_SYNTAX, line, "unknown statement '" + keyword.text() + "'");
    };
  }

  private Stmt ignoreTrailing(int line) {
    expectKeyword("trailing");
    return new Stmt.IgnoreTrailing(line);
  }

  private Stmt param(int line) {
    String name = keptName("a parameter name");
    if (!atPunctuation("=")) {
      return new Stmt.Param(line, name, null);
    }
    next();
    return new Stmt.Param(line, name, expression());
  }

  private Stmt declaration(int line) {
    String name = newName("a name");
    List<Expr> dimensions = new ArrayList<>();
    while (atPunctuation("[")) {
      next();
      dimensions.add(expression());
      expectPunctuation("]");
    }
    expectPunctuation(":");
    ReadingType type = type();
    if (type.kind() == ReadingKind.REST && !dimensions.isEmpty()) {
      throw failure(ErrorCode.SPEC_TYPE, "'rest' values cannot be arrays");
    }
    if (type.kind() == ReadingKind.SYMBOL && dimensions.size() != 2) {
      throw failure(ErrorCode.SPEC_TYPE, "an alphabet type needs a two-dimensional array");
    }
    Expr layout = layout(type, dimensions.size());
    Expr offset = offset(type);
    return new Stmt.Declaration(line, name, List.copyOf(dimensions), type, layout, offset);
  }

  private @Nullable Expr layout(ReadingType type, int dimensions) {
    if (!acceptOption("layout")) {
      return null;
    }
    if (dimensions != 2 || !type.kind().isNumeric()) {
      throw failure(ErrorCode.SPEC_TYPE, "'layout' needs a two-dimensional array of numbers");
    }
    return expression();
  }

  private @Nullable Expr offset(ReadingType type) {
    if (!acceptOption("offset")) {
      return null;
    }
    if (!type.kind().isNumeric()) {
      throw failure(ErrorCode.SPEC_TYPE, "'offset' needs numbers");
    }
    return expression();
  }

  private Stmt list(int line) {
    String name = newName("a name");
    expectPunctuation(":");
    ReadingType type = type();
    if (type.kind() == ReadingKind.REST || type.kind() == ReadingKind.SYMBOL) {
      throw failure(ErrorCode.SPEC_TYPE, "a list cannot be of 'rest' or of an alphabet type");
    }
    expectKeyword("until");
    Expr sentinel = accept("eol") ? null : expression();
    return new Stmt.ListRead(line, name, type, sentinel, offset(type));
  }

  private Stmt typeDefinition(int line) {
    String name = keptName("a type name");
    if (baseKind(name) != null || name.equals("alphabet")) {
      throw failure(ErrorCode.SPEC_NAME_CONFLICT, "'" + name + "' is a base type");
    }
    if (visibleTypes.containsKey(name)) {
      throw failure(ErrorCode.SPEC_NAME_CONFLICT, "the type '" + name + "' is already defined");
    }
    expectPunctuation("=");
    var definition = new Stmt.TypeDefinition(definedTypes.size(), line, name, type());
    visibleTypes.put(name, definition);
    definedTypes.add(definition);
    return definition;
  }

  private Stmt skip(int line) {
    if (accept("line")) {
      return new Stmt.Skip(line, new Expr.IntLiteral(1));
    }
    Expr count = expression();
    expectKeyword("lines");
    return new Stmt.Skip(line, count);
  }

  private Stmt repeat(int line) {
    int id = repeatsParsed++;
    RepeatCount count = repeatCount();
    String list = accept("as") ? newName("a name for the records") : null;
    atRepeatStart = true;
    List<Stmt> body = block();
    atRepeatStart = false;
    String each = !body.isEmpty() && body.getFirst() instanceof Stmt.Each e ? e.name() : null;
    return new Stmt.Repeat(id, line, count, list, each, body);
  }

  private RepeatCount repeatCount() {
    if (!accept("until")) {
      return new RepeatCount.Times(expression());
    }
    if (accept("eof")) {
      return new RepeatCount.UntilEof();
    }
    List<String> tokens = strings("a terminating token");
    return new RepeatCount.Until(
        new Terminators(tokens, accept("store") ? newName("a name") : null));
  }

  private Stmt each(int line) {
    if (!eachAllowed) {
      throw failure(
          ErrorCode.SPEC_PLACEMENT,
          "'each' must be the first statement of a repeat, and comes at most once");
    }
    return new Stmt.Each(line, newName("a name for each record"));
  }

  private Stmt instances(int line) {
    if (blockDepth > 0) {
      throw failure(ErrorCode.SPEC_PLACEMENT, "'instances' must be at the top level of the spec");
    }
    return new Stmt.Instances(line, block());
  }

  /** The options of sections can repeat and interleave (§5.7). */
  private Stmt sections(int line) {
    List<String> terminators = new ArrayList<>();
    @Nullable String store = null;
    boolean skipOther = false;
    while (true) {
      if (accept("until")) {
        terminators.addAll(strings("a terminating token"));
        store = accept("store") ? newName("a name") : store;
      } else if (accept("skip")) {
        expectKeyword("other");
        skipOther = true;
      } else {
        break;
      }
    }
    Terminators until =
        terminators.isEmpty() ? null : new Terminators(List.copyOf(terminators), store);
    return new Stmt.Sections(line, until, skipOther, arms());
  }

  /** The outer braces of sections enclose arms and do not count as a block (§5.12). */
  private List<Arm> arms() {
    expectPunctuation("{");
    List<Arm> arms = new ArrayList<>();
    while (!atPunctuation("}")) {
      if (peek().kind() == Token.Kind.END) {
        throw failure(ErrorCode.SPEC_SYNTAX, "missing '}'");
      }
      Pattern pattern = pattern();
      boolean optional = accept("optional");
      boolean last = accept("last");
      arms.add(new Arm(pattern, optional, last, atPunctuation("{") ? block() : List.of()));
    }
    next();
    return List.copyOf(arms);
  }

  private Stmt conditional(int line) {
    Expr condition = expression();
    List<Stmt> body = block();
    List<Stmt> orElse = accept("else") ? elseBody() : List.of();
    return new Stmt.If(line, condition, body, orElse);
  }

  /** A statement after {@code else} starts with {@code if} and can be a declaration (§5.6). */
  private List<Stmt> elseBody() {
    if (!atKeyword("if")) {
      return block();
    }
    enterBlock();
    Stmt chained = statement();
    blockDepth--;
    return List.of(chained);
  }

  private Stmt let(int line) {
    String name = keptName("a name");
    expectPunctuation("=");
    return new Stmt.Let(line, name, expression());
  }

  private Stmt check(int line) {
    Expr condition = expression();
    String message = peek().kind() == Token.Kind.STRING ? next().text() : null;
    return new Stmt.Check(checksParsed++, line, condition, message);
  }

  /*
   * Patterns and types
   */

  private Pattern pattern() {
    int line = peek().line();
    String source = string("a line pattern");
    return PatternParser.compile(source, origin, line, text -> captureType(text, line));
  }

  /**
   * Returns the type {@code text} of a capture in a pattern on {@code line}, by parsing it in place
   * of the tokens of the spec. The nesting counters carry over (§5.12).
   */
  private ReadingType captureType(String text, int line) {
    List<Token> captureTokens = Lexer.lex(text.getBytes(UTF_8), origin, line);
    if (captureTokens.size() == 1) {
      throw failureAt(ErrorCode.SPEC_PATTERN, line, "a capture needs a type after ':'");
    }
    List<Token> specTokens = tokens;
    int specPosition = position;
    tokens = captureTokens;
    position = 0;
    ReadingType type = type();
    if (peek().kind() != Token.Kind.END) {
      throw failure(
          ErrorCode.SPEC_PATTERN,
          "unexpected " + describe(peek()) + " in the capture type '" + text + "'");
    }
    tokens = specTokens;
    position = specPosition;
    if (type.kind() == ReadingKind.SYMBOL) {
      throw failureAt(ErrorCode.SPEC_PATTERN, line, "a capture cannot be of an alphabet type");
    }
    return type;
  }

  private static @Nullable ReadingKind baseKind(String name) {
    return switch (name) {
      case "int" -> ReadingKind.INT;
      case "float" -> ReadingKind.FLOAT;
      case "num" -> ReadingKind.NUM;
      case "word" -> ReadingKind.WORD;
      case "rest" -> ReadingKind.REST;
      default -> null;
    };
  }

  /** A base type, an alphabet, a visible named type, or a range, in that order (§5.7). */
  private ReadingType type() {
    if (peek().kind() != Token.Kind.IDENTIFIER) {
      return range();
    }
    String name = peek().text();
    ReadingKind base = baseKind(name);
    if (base != null) {
      next();
      return new ReadingType.Base(base);
    }
    if (name.equals("alphabet")) {
      next();
      return alphabet();
    }
    Stmt.TypeDefinition definition = visibleTypes.get(name);
    if (definition != null) {
      next();
      return new ReadingType.Named(definition.id(), definition.type().kind());
    }
    return range();
  }

  private boolean atRangeOperator() {
    return atPunctuation("..") || atPunctuation("..<");
  }

  private ReadingType range() {
    Expr lower = atRangeOperator() ? null : lowerBound();
    boolean exclusive = atPunctuation("..<");
    next();
    Expr upper = atUpperBound() ? expression() : null;
    if (exclusive && upper == null) {
      throw failure(ErrorCode.SPEC_SYNTAX, "'..<' needs an upper bound");
    }
    if (lower == null && upper == null) {
      throw failure(ErrorCode.SPEC_SYNTAX, "a range needs a lower or an upper bound");
    }
    return new ReadingType.Range(lower, upper, exclusive);
  }

  private Expr lowerBound() {
    Expr lower = expression();
    if (atRangeOperator()) {
      return lower;
    }
    if (lower instanceof Expr.Name unknown) {
      throw failure(ErrorCode.SPEC_UNKNOWN_TYPE, "unknown type '" + unknown.name() + "'");
    }
    throw failure(
        ErrorCode.SPEC_SYNTAX,
        "expected '..' or '..<' after the lower bound of a range, found " + describe(peek()));
  }

  private boolean atUpperBound() {
    Token token = peek();
    return switch (token.kind()) {
      case IDENTIFIER -> !RANGE_STOPS.contains(token.text()) && !atDeclaration();
      case PUNCTUATION -> atPunctuation("(") || atPunctuation("-") || atPunctuation("!");
      case INTEGER, FLOAT, STRING -> true;
      case END -> false;
    };
  }

  /** The rest of an alphabet, whose options can repeat in any order (§5.7). */
  private ReadingType alphabet() {
    String symbols = string("an alphabet");
    String blank = "";
    String strip = "";
    long base = 1;
    while (true) {
      if (acceptOption("blank")) {
        blank = string("blank symbols");
      } else if (acceptOption("strip")) {
        strip = string("symbols to strip");
      } else if (acceptOption("base")) {
        base = alphabetBase();
      } else {
        return new ReadingType.Alphabet(symbols, blank, strip, base);
      }
    }
  }

  private long alphabetBase() {
    boolean negative = atPunctuation("-");
    if (negative) {
      next();
    }
    if (peek().kind() != Token.Kind.INTEGER) {
      throw failure(ErrorCode.SPEC_SYNTAX, "expected an integer, found " + describe(peek()));
    }
    long base = next().integer();
    return negative ? -base : base;
  }

  /*
   * Expressions, by precedence climbing
   */

  private Expr expression() {
    return expression(0);
  }

  /**
   * Parses binary operators of at least precedence {@code lowest}, which are left-associative. Each
   * operator holds a counter entry until this method returns (§5.12).
   */
  private Expr expression(int lowest) {
    Expr left = unary();
    int held = 0;
    while (true) {
      Level level =
          peek().kind() == Token.Kind.PUNCTUATION ? BINARY_LEVELS.get(peek().text()) : null;
      if (level == null || level.precedence() < lowest) {
        break;
      }
      enterExpression();
      held++;
      next();
      left = new Expr.Binary(level.operator(), left, expression(level.precedence() + 1));
    }
    expressionDepth -= held;
    return left;
  }

  private Expr unary() {
    enterExpression();
    Expr expression;
    if (atPunctuation("-") || atPunctuation("!")) {
      var operator = atPunctuation("-") ? UnaryOperator.NEGATE : UnaryOperator.NOT;
      next();
      expression = new Expr.Unary(operator, unary());
    } else {
      expression = power();
    }
    expressionDepth--;
    return expression;
  }

  /** {@code a ^ b} binds tighter than unary operators, but b is a unary expression (§5.10). */
  private Expr power() {
    Expr base = postfix();
    if (!atPunctuation("^")) {
      return base;
    }
    next();
    return new Expr.Binary(BinaryOperator.POWER, base, unary());
  }

  /** Each index or field holds a counter entry until this method returns (§5.12). */
  private Expr postfix() {
    Expr expression = primary();
    int held = 0;
    while (atPunctuation("[") || atPunctuation(".")) {
      boolean isField = atPunctuation(".");
      next();
      enterExpression();
      held++;
      if (isField) {
        expression = new Expr.Field(expression, identifier("a field name"), sites++);
      } else {
        Expr index = expression(0);
        expectPunctuation("]");
        expression = new Expr.Index(expression, index, sites++);
      }
    }
    expressionDepth -= held;
    return expression;
  }

  private Expr primary() {
    Token token = peek();
    if (token.kind() == Token.Kind.END
        || (token.kind() == Token.Kind.PUNCTUATION && !atPunctuation("("))) {
      throw failure(ErrorCode.SPEC_SYNTAX, "expected an expression, found " + describe(token));
    }
    next();
    return switch (token.kind()) {
      case INTEGER -> new Expr.IntLiteral(token.integer());
      case FLOAT -> new Expr.FloatLiteral(token.real());
      case STRING -> new Expr.StringLiteral(token.text());
      case IDENTIFIER ->
          token.text().startsWith(Lexer.HOST_PREFIX) ? hostConstant(token) : nameOrCall(token);
      case PUNCTUATION, END -> {
        Expr expression = expression(0);
        expectPunctuation(")");
        yield expression;
      }
    };
  }

  private Expr hostConstant(Token name) {
    return switch (name.text()) {
      case "gecode.int_max" -> new Expr.IntLiteral(2147483646);
      case "gecode.int_min" -> new Expr.IntLiteral(-2147483646);
      default ->
          throw failureAt(
              ErrorCode.SPEC_CONSTANT,
              name.line(),
              "unknown Gecode constant '"
                  + name.text()
                  + "' (known: gecode.int_max, gecode.int_min)");
    };
  }

  private Expr nameOrCall(Token name) {
    if (!atPunctuation("(")) {
      return new Expr.Name(name.text(), sites++);
    }
    next();
    ComprehensionFunction function = comprehensionFunction(name.text());
    return function != null && atComprehension() ? comprehension(function) : call(name);
  }

  private static @Nullable ComprehensionFunction comprehensionFunction(String name) {
    return switch (name) {
      case "all" -> ComprehensionFunction.ALL;
      case "any" -> ComprehensionFunction.ANY;
      case "sum" -> ComprehensionFunction.SUM;
      case "count" -> ComprehensionFunction.COUNT;
      default -> null;
    };
  }

  private boolean atComprehension() {
    Token binding = peek();
    if (binding.kind() != Token.Kind.IDENTIFIER) {
      return false;
    }
    Token following = tokens.get(position + 1);
    return following.kind() == Token.Kind.IDENTIFIER && following.text().equals("in");
  }

  /** The rest of {@code function(binding in domain: body)}, after the opening parenthesis. */
  private Expr comprehension(ComprehensionFunction function) {
    int line = peek().line();
    String binding = newName("a name");
    if (binding.equals("_")) {
      throw failure(ErrorCode.SPEC_NAME, "'_' cannot be bound");
    }
    if (enclosingBindings.contains(binding)) {
      throw failure(
          ErrorCode.SPEC_NAME_CONFLICT,
          "'" + binding + "' is already bound by an enclosing comprehension");
    }
    expectKeyword("in");
    Expr domain = expression(0);
    expectPunctuation(":");
    // The name is bound in the body only.
    enclosingBindings.add(binding);
    Expr body = expression(0);
    enclosingBindings.removeLast();
    expectPunctuation(")");
    completedBindings.add(new Binding(binding, line));
    return new Expr.Comprehension(function, binding, domain, body, sites++);
  }

  /** The rest of an ordinary call, after the opening parenthesis. */
  private Expr call(Token name) {
    List<Expr> arguments = new ArrayList<>();
    if (!atPunctuation(")")) {
      arguments.add(expression(0));
      while (atPunctuation(",")) {
        next();
        arguments.add(expression(0));
      }
    }
    expectPunctuation(")");
    return new Expr.Call(function(name, arguments), List.copyOf(arguments));
  }

  /** Returns the function that {@code name} calls, with suitable {@code arguments} (§8.8). */
  private Function function(Token name, List<Expr> arguments) {
    String text = name.text();
    Function function =
        switch (text) {
          case "defined" -> Function.DEFINED;
          case "count" -> Function.COUNT;
          case "size" -> Function.SIZE;
          case "min" -> Function.MIN;
          case "max" -> Function.MAX;
          case "sum" -> Function.SUM;
          case "distinct" -> Function.DISTINCT;
          case "all", "any" ->
              throw failureAt(
                  ErrorCode.SPEC_FUNCTION,
                  name.line(),
                  "'" + text + "' needs the form " + text + "(x in a: e)");
          default ->
              throw failureAt(
                  ErrorCode.SPEC_FUNCTION, name.line(), "unknown function '" + text + "'");
        };
    boolean accepted =
        switch (function) {
          case DEFINED -> arguments.size() == 1 && arguments.getFirst() instanceof Expr.Name;
          case COUNT, SIZE -> arguments.size() == 1;
          case MIN, MAX, SUM, DISTINCT -> !arguments.isEmpty();
        };
    if (!accepted) {
      throw failureAt(
          ErrorCode.SPEC_FUNCTION, name.line(), "wrong arguments for function '" + text + "'");
    }
    return function;
  }
}
