package dev.gecode.iris;

import static java.nio.charset.StandardCharsets.UTF_8;
import static java.util.Objects.requireNonNull;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTimeoutPreemptively;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.gecode.iris.internal.syntax.Ast;
import dev.gecode.iris.internal.syntax.Ast.Expr;
import dev.gecode.iris.internal.syntax.Ast.ReadingKind;
import dev.gecode.iris.internal.syntax.Ast.ReadingType;
import dev.gecode.iris.internal.syntax.Ast.Stmt;
import dev.gecode.iris.internal.syntax.LargeStack;
import dev.gecode.iris.internal.syntax.Lexer;
import dev.gecode.iris.internal.syntax.Parser;
import dev.gecode.iris.internal.syntax.Resolver;
import java.time.Duration;
import java.util.List;
import java.util.concurrent.atomic.AtomicReference;
import org.jspecify.annotations.Nullable;
import org.junit.jupiter.api.Test;

class SpecTest {
  /** The stack of the thread in the nesting tests, a quarter of the usual size. */
  private static final long SMALL_STACK = 512 * 1024;

  private static IrisError errorOf(String source) {
    return assertThrows(IrisException.class, () -> Spec.parse(source, "spec")).error();
  }

  @Test
  void reportsCodeAndLocationOfTheFirstError() {
    IrisError error = errorOf("let a = 1\nlet = 2\nlet = 3\n");
    assertEquals(ErrorCode.SPEC_SYNTAX, error.code());
    assertEquals(new Location("spec", 2, null), error.specLocation());
    assertNull(error.inputLocation());
    assertNull(error.context());
    assertNull(error.recordPath());
  }

  @Test
  void rendersTheLocationInTheMessage() {
    var failure = assertThrows(IrisException.class, () -> Spec.parse("let", "spec"));
    assertEquals("spec:1: expected a name, found end of spec", failure.getMessage());
  }

  @Test
  void reportsInvalidBytesWithAColumn() {
    byte[] source = {'l', 'e', 't', '\n', 'a', (byte) 0xc3, (byte) 0xa5, (byte) 0xff};
    var failure = assertThrows(IrisException.class, () -> Spec.parse(source));
    assertEquals(ErrorCode.SPEC_UTF8, failure.code());
    assertEquals(new Location(null, 2, 4), failure.error().specLocation());
  }

  @Test
  void reportsAnUnpairedSurrogateInText() {
    IrisError error = errorOf("# \uD83D\uDE00 paired\nlet a\uD83D = 1\n");
    assertEquals(ErrorCode.SPEC_UTF8, error.code());
    assertEquals(new Location("spec", 2, 6), error.specLocation());
  }

  @Test
  void acceptsTextThatEncodesToTheSameBytes() {
    String source = "# \uD83D\uDE00 å\nline \"{a:int}\"\n";
    assertEquals(Spec.parse(source).program(), Spec.parse(source.getBytes(UTF_8)).program());
  }

  @Test
  void reportsAStringThatEndsWithTheSource() {
    assertEquals(ErrorCode.SPEC_STRING, errorOf("let s = \"a").code());
    assertEquals(ErrorCode.SPEC_STRING, errorOf("let s = \"a\\").code());
  }

  @Test
  void readsFormatNames() {
    assertEquals("dimacs-cnf", Spec.parse("format dimacs-cnf").format());
    assertEquals("a.b-1", Spec.parse("format a.b-1").format());
    assertEquals("two words", Spec.parse("format \"two words\"").format());
    assertEquals("b", Spec.parse("format a\nformat b").format());
  }

  @Test
  void onlyGluedWordsJoinAFormatName() {
    assertEquals(ErrorCode.SPEC_SYNTAX, errorOf("format a -b").code());
    assertEquals(ErrorCode.SPEC_SYNTAX, errorOf("format a- b").code());
    assertEquals(ErrorCode.SPEC_SYNTAX, errorOf("format a-\"b\"").code());
  }

  private static String blocks(int depth, String innermost) {
    return "if 1 {\n".repeat(depth) + innermost + "\n" + "}\n".repeat(depth);
  }

  /** Runs {@code action} on a small stack, and returns what it threw. */
  private static @Nullable Throwable onSmallStack(Runnable action) throws InterruptedException {
    var thrown = new AtomicReference<Throwable>();
    var thread = new Thread(null, action, "small-stack", SMALL_STACK);
    thread.setUncaughtExceptionHandler((t, e) -> thrown.set(e));
    thread.start();
    thread.join();
    return thrown.get();
  }

  @Test
  void keepsTheInterruptOfTheCaller() {
    Thread.currentThread().interrupt();
    Spec.parse("let a = 1");
    assertTrue(Thread.interrupted());
  }

  @Test
  void limitsTheBlockCounter() {
    assertFalse(Spec.parse(blocks(256, "")).program().body().isEmpty());
    assertEquals(ErrorCode.SPEC_NESTING, errorOf(blocks(257, "")).code());
  }

  @Test
  void limitsTheExpressionCounter() {
    String legal = "(".repeat(255) + "1" + ")".repeat(255);
    assertFalse(Spec.parse("let a = " + legal).program().body().isEmpty());
    assertEquals(ErrorCode.SPEC_NESTING, errorOf("let a = (" + legal + ")").code());
  }

  @Test
  void legalNestingFitsOnASmallStack() throws InterruptedException {
    String parentheses = "(".repeat(255) + "1" + ")".repeat(255);
    String negations = "- ".repeat(255) + "1";
    String calls = "min(".repeat(255) + "1" + ")".repeat(255);
    String sections = "sections {\n\"x\" {\n".repeat(256) + "}\n}\n".repeat(256);
    String sectionsAroundCalls = sections.replace("{\n}", "{\nlet a = " + calls + "\n}");
    assertNull(onSmallStack(() -> Spec.parse(blocks(256, "let a = " + parentheses))));
    assertNull(onSmallStack(() -> Spec.parse(blocks(256, "let a = " + negations))));
    assertNull(onSmallStack(() -> Spec.parse(sectionsAroundCalls)));
  }

  @Test
  void excessiveNestingIsAnErrorOnASmallStack() throws InterruptedException {
    String source = "(".repeat(100_000);
    Throwable thrown = onSmallStack(() -> Spec.parse("let a = " + source));
    assertEquals(ErrorCode.SPEC_NESTING, assertInstanceOf(IrisException.class, thrown).code());
  }

  /**
   * Returns the longest left spine that §5.12 allows for {@code step}, which holds one entry. An
   * operator chain alone stops at 255, but closing a parenthesis releases its entries, so each
   * level of parentheses can start a new chain.
   */
  private static String leftNested(String operand, String step) {
    return leftNested(255, operand, step);
  }

  /** Returns a left spine of {@code levels} chains, which end up to {@code levels} entries deep. */
  private static String leftNested(int levels, String operand, String step) {
    var source = new StringBuilder("(".repeat(levels)).append(operand);
    for (int count = 0; count < levels; count++) {
      source.append(')').append(step.repeat(count));
    }
    return source.toString();
  }

  private static Ast.Program parsed(String source) {
    return LargeStack.call(() -> Parser.parse(Lexer.lex(source.getBytes(UTF_8), null), null));
  }

  @Test
  void aPlainChainStopsAtTheExpressionCounter() {
    assertFalse(Spec.parse("let a = 1" + "+1".repeat(255)).program().body().isEmpty());
    assertEquals(ErrorCode.SPEC_NESTING, errorOf("let a = 1" + "+1".repeat(256)).code());
  }

  @Test
  void leftSpinesAreLongerThanTheCounters() {
    var let = (Stmt.Let) parsed("let a = " + leftNested("1", "+1")).body().getFirst();
    assertEquals(32_386, let.value().leftSpine().size());
  }

  @Test
  void leftSpineStartsAtTheLeftmostOperand() {
    var let = (Stmt.Let) parsed("let a = b[1].c + 2").body().getFirst();
    List<Expr> spine = let.value().leftSpine();
    assertEquals(new Expr.Name("b", 0), spine.getFirst());
    assertEquals(let.value(), spine.getLast());
    assertEquals(4, spine.size());
  }

  @Test
  void resolvingALeftSpineFitsOnASmallStack() throws InterruptedException {
    for (String step : List.of("+1", "*2", "[0]", "+min(1)")) {
      Ast.Program program = parsed("let a = " + leftNested("1", step));
      assertNull(onSmallStack(() -> Resolver.resolve(program, null)), step);
    }
  }

  @Test
  void resolvesLeftSpinesInEveryOperandPosition() {
    String spine = leftNested(250, "1", "+1");
    Spec.parse("a[" + spine + "]: 0.." + spine + "\nlet b = 1 + " + spine);
    Spec.parse("if " + spine + " {\nrepeat " + spine + " {\n}\n}\ncheck " + spine);
    Spec.parse("let a = min(" + spine + ", -" + spine + ")");
    Spec.parse("let a = sum(x in " + spine + ": " + spine + ")");
  }

  @Test
  void comparesRepeatsWhoseCountsAreLeftSpines() {
    String spine = leftNested("1", "+1");
    String repeats =
        "repeat " + spine + " as a {\nx: int\n}\nrepeat " + spine + " as b {\ny: int\n}\n";
    assertEquals(ErrorCode.SPEC_CHECK_SCOPE, errorOf(repeats + "check x == y").code());
  }

  @Test
  void comparesRepeatsThatDifferOnlyByTheirId() {
    String spine = leftNested("1", "+1");
    String repeats = "repeat " + spine + " {x: int} repeat " + spine + " {x: int}\n";
    assertEquals(ErrorCode.SPEC_CHECK_SCOPE, errorOf(repeats + "check x").code());
  }

  @Test
  void reportsAnErrorAtTheEndOfALeftSpine() {
    String records = "repeat 1 as r {\n}\nlet a = ";
    assertEquals(ErrorCode.SPEC_RECORD_VALUE, errorOf(records + leftNested("r", "+1")).code());
    assertEquals(ErrorCode.SPEC_FIELD, errorOf("let a = " + leftNested("1", ".a")).code());
  }

  /**
   * Returns {@code count} definitions {@code t0} to {@code t(count-1)}, each an alias of the last.
   */
  private static String aliases(int count, String type) {
    var source = new StringBuilder("type t0 = ").append(type).append('\n');
    for (int k = 1; k < count; k++) {
      source.append("type t").append(k).append(" = t").append(k - 1).append('\n');
    }
    return source.toString();
  }

  @Test
  void aUseOfAnAliasNeedsNoWalkOfTheChain() throws InterruptedException {
    String source = aliases(40_000, "float") + "a: t39999\n";
    var program = new AtomicReference<Ast.Program>();
    assertNull(onSmallStack(() -> program.set(parsed(source))));
    assertNull(onSmallStack(() -> assertEquals(program.get(), parsed(source))));
    Ast.Program parsed = requireNonNull(program.get());
    var declaration = (Stmt.Declaration) parsed.body().getLast();
    assertEquals(new ReadingType.Named(39_999, ReadingKind.FLOAT), declaration.type());
    assertEquals("t39999", parsed.types().get(39_999).name());
  }

  @Test
  void aLongAliasChainKeepsTheRulesOfItsKind() {
    assertEquals(ErrorCode.SPEC_TYPE, errorOf(aliases(40_000, "rest") + "a[2]: t39999").code());
  }

  @Test
  void aliasChainsParseInLinearTime() {
    String source = aliases(200_000, "int");
    assertTimeoutPreemptively(Duration.ofSeconds(20), () -> Spec.parse(source));
  }

  @Test
  void namedTypesAreVisibleInTheirBlockOnly() {
    assertEquals(ErrorCode.SPEC_UNKNOWN_TYPE, errorOf("if 1 {\ntype t = int\n}\na: t").code());
    Ast.Program program = Spec.parse("if 1 {\ntype t = int\n}\ntype t = float").program();
    assertEquals(List.of(0, 1), program.types().stream().map(Stmt.TypeDefinition::id).toList());
    assertEquals(
        ErrorCode.SPEC_NAME_CONFLICT, errorOf("type t = int\nif 1 {\ntype t = int\n}").code());
  }

  private static int descend(int depth) {
    return depth == 0 ? 0 : 1 + descend(depth - 1);
  }

  @Test
  void anOverflowOfTheLargeStackIsAHostFailure() {
    var failure =
        assertThrows(
            IllegalStateException.class, () -> LargeStack.call(() -> descend(Integer.MAX_VALUE)));
    assertTrue(failure.getCause() instanceof StackOverflowError);
  }
}
