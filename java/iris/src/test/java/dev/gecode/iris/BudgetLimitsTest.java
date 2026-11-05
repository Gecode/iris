package dev.gecode.iris;

import static java.nio.charset.StandardCharsets.UTF_8;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;

import java.util.List;
import java.util.Objects;
import java.util.concurrent.TimeUnit;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.Timeout;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.MethodSource;

/**
 * Exact boundaries of the work budgets (§10). Each test charges a known cost, then uses up all but
 * none of the budget, which must pass, or goes one unit over, which must fail. A charge that is
 * lost, added or off by one moves the boundary, whichever operation makes it.
 */
class BudgetLimitsTest {
  private static final long BASE = 1L << 20;
  private static final long EFFORT_PER_INPUT_BYTE = 16;
  private static final int BURN_COLUMNS = 1023;
  private static final String IDLE_MESSAGE =
      "too many repetitions or empty rows without reading input";
  private static final String EFFORT_MESSAGE =
      "the expressions do too much work for the size of the input";

  /** The two operations that run budgets. */
  private enum Operation {
    READ,
    CATALOG;

    Data on(Spec spec, byte[] input, Params params) {
      return switch (this) {
        case READ -> spec.read(input, "input", params);
        case CATALOG -> spec.catalog(input, "input", params);
      };
    }
  }

  /**
   * A reader spec with a known cost.
   *
   * @param spec its text
   * @param input the input, which it reads to the end
   * @param cost the units it charges
   */
  private record Priced(String spec, String input, long cost) {
    @Override
    public String toString() {
      return spec + " on '" + input + "' costs " + cost;
    }
  }

  private static byte[] bytes(String text) {
    return text.getBytes(UTF_8);
  }

  private static IrisError budgetFailure(
      Operation operation, Spec spec, byte[] input, Params params) {
    IrisError error =
        assertThrows(IrisException.class, () -> operation.on(spec, input, params)).error();
    assertEquals(ErrorCode.BUDGET, error.code(), error.message());
    assertNull(error.recordPath());
    return error;
  }

  /** Requires {@code allowed} to pass, even twice on one spec, and {@code excessive} to fail. */
  private static void requireBoundary(String allowed, String excessive, byte[] input) {
    Spec spec = Spec.parse(allowed, "spec");
    Spec over = Spec.parse(excessive, "spec");
    for (Operation operation : Operation.values()) {
      operation.on(spec, input, Params.of());
      operation.on(spec, input, Params.of());
      budgetFailure(operation, over, input, Params.of());
    }
  }

  /** Returns statements that charge exactly {@code units} of effort, and no more than 2^12 idle. */
  private static String burn(long units) {
    long rows = units / (BURN_COLUMNS + 1);
    int rest = (int) (units % (BURN_COLUMNS + 1));
    String burn =
        "repeat "
            + rows
            + " as burnRows {}\nrepeat "
            + BURN_COLUMNS
            + " as burnColumns {}\ncheck all(r in burnRows: all(c in burnColumns: 1))\n";
    return rest == 0 ? burn : burn + "let burnRest=sum(" + "1,".repeat(rest - 1) + "1)\n";
  }

  private static void requireEffort(String spec, byte[] input, long cost) {
    long left = BASE + EFFORT_PER_INPUT_BYTE * input.length - cost;
    requireBoundary(spec + "\n" + burn(left), spec + "\n" + burn(left + 1), input);
  }

  private static void requireIdle(String spec, byte[] input, long cost) {
    long left = BASE - cost;
    requireBoundary(
        spec + "\nrepeat " + left + " {}", spec + "\nrepeat " + (left + 1) + " {}", input);
  }

  /*
   * Idle work (§10.2)
   */

  private static List<Priced> idleCosts() {
    return List.of(
        new Priced("", "", 0),
        new Priced("repeat 0 {}", "", 0),
        new Priced("repeat 5 {}", "", 5),
        new Priced("repeat 2 {let x=1}", "", 2),
        new Priced("repeat 2 {repeat 3 {}}", "", 8),
        new Priced("repeat 2 {repeat 0 {}}", "", 2),
        new Priced("repeat 3 {skip 1 lines}", "a\nb\nc\n", 0),
        new Priced("repeat 2 {_:int}", "1 2", 0),
        new Priced("repeat 2 {skip 0 lines}", "", 2),
        new Priced("n:int repeat n {} repeat 2 {_:int}", "3 1 2", 3),
        new Priced("_[0]:int", "", 0),
        new Priced("_[0][5]:int", "", 0),
        new Priced("_[3][0]:int", "", 3),
        new Priced("_[3][0][7]:int", "", 3),
        new Priced("_[3][4][0]:int", "", 15),
        new Priced("_[2][3][4][0][9]:int", "", 32),
        new Priced("x[3][0]:float", "", 3),
        new Priced("_[2][2]:int", "1 2 3 4", 0),
        new Priced("_[3][0]:int layout \"FULL_MATRIX\"", "", 3),
        new Priced("_[0][3]:int layout \"FULL_MATRIX\"", "", 0),
        new Priced("_[0][0]:int layout \"UPPER_ROW\"", "", 0),
        new Priced("_[1][1]:int layout \"LOWER_ROW\"", "", 0),
        new Priced("_[3][0]:alphabet \"01\"", "", 3),
        new Priced("_[0][5]:alphabet \"01\"", "", 0),
        new Priced("_[2][2]:alphabet \"01\"", "0101", 0),
        new Priced("repeat 2 {_[2][0]:int}", "", 6));
  }

  @ParameterizedTest
  @MethodSource("idleCosts")
  void idleWorkCostsExactly(Priced priced) {
    requireIdle(priced.spec(), bytes(priced.input()), priced.cost());
  }

  @Test
  void theIdleLimitIsTheSameForAnyInput() {
    requireBoundary("repeat 1048576 {}", "repeat 1048577 {}", new byte[0]);
    String spec = "ignore trailing\nrepeat 1048576 {}";
    requireBoundary(spec, spec + "\nrepeat 1 {}", new byte[1000]);
  }

  @Test
  void idleWorkDoesNotResetWhenInputAdvances() {
    requireBoundary(
        "repeat 1048576 {}\n_:int", "repeat 1048576 {}\n_:int\nrepeat 1 {}", bytes("0"));
  }

  @Test
  void oneIdleBudgetSpansThePrefixAndEveryInstance() {
    Spec spec = Spec.parse("repeat 1048574 {}\ninstances {\n_:int\nrepeat 1 {}\n}");
    Spec over = Spec.parse("repeat 1048575 {}\ninstances {\n_:int\nrepeat 1 {}\n}");
    assertEquals(new Value.IntValue(2), spec.catalog("1 2").get("instances"));
    budgetFailure(Operation.CATALOG, over, bytes("1 2"), Params.of());
    budgetFailure(Operation.READ, over, bytes("1 2"), Params.of());
    spec.read(bytes("1 2"), "input", Params.of("instance", "2"));
  }

  @Test
  void selectionEndsChargesButTraversalDoesNot() {
    Spec spec = Spec.parse("instances {\nn:int\nrepeat n {}\n}");
    byte[] input = bytes("1048576 1");
    spec.read(input, "input", Params.of("instance", "1"));
    budgetFailure(Operation.READ, spec, input, Params.of());
    budgetFailure(Operation.CATALOG, spec, input, Params.of("instance", "1"));
  }

  /*
   * Expression effort (§10.3, §10.4)
   */

  private static List<Priced> effortCosts() {
    return List.of(
        new Priced("", "", 0),
        // What §10.3 leaves out.
        new Priced("let v=1+2*3^2", "", 0),
        new Priced("a[3]:int let v=a", "1 2 3", 0),
        new Priced("a[3]:int let v=size(a) let w=count(a)", "1 2 3", 0),
        new Priced("repeat 3 as rs {} let v=count(rs)", "", 0),
        new Priced("let v=defined(a)", "", 0),
        new Priced("repeat 2 as rows {each row x:int let y=row.x+1}", "1 2", 0),
        // Aggregates.
        new Priced("let v=sum(1)", "", 1),
        new Priced("let v=sum(1,2)", "", 2),
        new Priced("a[3]:int let v=min(a)", "1 2 3", 3),
        new Priced("a[3]:int let v=max(a)", "1 2 3", 3),
        new Priced("a[3]:int let v=sum(a)", "1 2 3", 3),
        new Priced("a[3]:int let v=distinct(a)", "1 2 3", 3),
        new Priced("a[3]:int let v=distinct(a,a)", "1 2 3", 6),
        new Priced("a[2]:int let v=distinct(a,a)", "1 1", 4),
        new Priced("a[1]:int let v=distinct(a,a,a)", "1", 3),
        new Priced("a[2][2]:int let v=sum(a)", "1 2 3 4", 4),
        new Priced("a[0]:int let v=sum(a)", "", 0),
        new Priced("a[0]:word let v=sum(a)", "", 0),
        // Indices.
        new Priced("a[2]:int let v=a[1]", "1 2", 2),
        new Priced("a[2][3]:int let v=a[1]", "1 2 3 4 5 6", 4),
        new Priced("a[1][0]:int let v=a[0]", "", 1),
        new Priced("a[2][3]:int let v=a[1][2]", "1 2 3 4 5 6", 6),
        new Priced("a[2]:int let v=a[a[0]]", "0 5", 4),
        new Priced("a[2][3]:int let v=sum(a[1])", "1 2 3 4 5 6", 7),
        // Records and fields.
        new Priced("repeat 2 as rs {x:int} let v=rs[1].x", "1 2", 3),
        new Priced("repeat 1 as rs {list x:int until 0} let v=rs[0].x", "1 2 3 0", 5),
        new Priced("repeat 1 as rs {list x:int until 0} let v=rs[0].x", "0", 2),
        new Priced(
            "repeat 2 as jobs {x:int repeat 1 as ss {y:int}} let v=jobs[1].ss[0].y", "1 2 3 4", 4),
        // Comprehensions.
        new Priced("a[3]:int let v=sum(x in a: x)", "1 2 3", 6),
        new Priced("a[3]:int let v=count(x in a: 1)", "1 2 3", 6),
        new Priced("a[2][3]:int let v=count(r in a: 1)", "1 2 3 4 5 6", 8),
        new Priced("a[2][0]:int let v=count(r in a: 1)", "", 2),
        new Priced("a[0]:int let v=count(x in a: 1)", "", 0),
        new Priced("a[3]:int let v=all(x in a: 0)", "1 2 3", 2),
        new Priced("a[3]:int let v=all(x in a: x<2)", "1 2 3", 4),
        new Priced("a[3]:int let v=any(x in a: 1)", "1 2 3", 2),
        new Priced("a[3]:int let v=any(x in a: 0)", "1 2 3", 6),
        new Priced("repeat 3 as rs {} let v=count(r in rs: 1)", "", 3),
        new Priced("repeat 2 as rs {x:int} let v=sum(r in rs: r.x)", "1 2", 6),
        new Priced("repeat 1 as rs {list x:int until 0} let v=count(e in rs[0].x: 1)", "5 0", 5),
        // Short circuits.
        new Priced("let v=1 || sum(1,2)", "", 0),
        new Priced("let v=0 && sum(1,2)", "", 0),
        new Priced("let v=0 || sum(1,2)", "", 2),
        new Priced("if 0 {let v=sum(1)} else {let v=sum(1,2)}", "", 2),
        // Expressions that statements need.
        new Priced("a[sum(1)]:int", "7", 1),
        new Priced("repeat sum(0) {}", "", 1),
        new Priced("x:0..sum(1,1)", "1", 2),
        new Priced("x:int offset sum(1)", "1", 1),
        new Priced("type t=0..sum(1) a[2]:t", "0 1", 1),
        new Priced("repeat 3 {let v=sum(1,2)}", "", 6),
        // Lifted checks.
        new Priced("repeat 2 {x:int} check x>0", "1 2", 4),
        new Priced("repeat 2 {x:int y:int} check x>0 || y>0", "1 2 3 4", 8),
        new Priced("repeat 2 {x:int} check x>0 && x<9", "1 2", 4),
        new Priced("repeat 1 {list a:int until 0 list b:int until 0} check 1 || a || a", "0 0", 1),
        new Priced("repeat 1 {list a:int until 0 list b:int until 0} check 1 || a || b", "0 0", 2),
        new Priced("repeat 1 {list a:int until 0} check 1 || a", "1 2 3 0", 4),
        new Priced(
            "repeat 1 {each row list a:int until 0 list b:int until 0} check 1 || row.a || row.b",
            "0 0",
            2),
        new Priced("repeat 2 as rows {each row x:int} check row.x>0", "1 2", 8),
        new Priced("repeat 2 {k:int repeat k {x:int}} check x>0", "1 5 2 6 7", 11),
        new Priced(
            "repeat 2 as jobs {each job repeat 2 as steps {each step x:int}} check step.x>0",
            "1 2 3 4",
            22),
        new Priced("repeat 0 {x:int} check x>0", "", 0));
  }

  @ParameterizedTest
  @MethodSource("effortCosts")
  void effortCostsExactly(Priced priced) {
    requireEffort(priced.spec(), bytes(priced.input()), priced.cost());
  }

  @Test
  void aCheckCanEndExactlyOnTheLimit() {
    // The input has 2048 bytes: 1056 * 1024 = B + 16*N.
    byte[] input = bytes("0 ".repeat(1024));
    String spec = "a[1024]:int\nrepeat 1056 {\ncheck sum(a)==0\n}";
    requireBoundary(spec, spec + "\ncheck sum(1)==1", input);
    requireBoundary(spec, spec.replace("1056", "1057"), input);
  }

  @Test
  void theEffortLimitCountsEveryInputByte() {
    String settings = "ignore trailing\ncomment \"#\"\nstop \"%\"";
    requireEffort(settings, bytes("é"), 0);
    requireEffort(settings, bytes("😀"), 0);
    requireEffort(settings, bytes("\r\n\r\n"), 0);
    requireEffort(settings, bytes("﻿# comment\r\n% stopped\r\n"), 0);
    requireEffort(settings, new byte[] {0, 0, 0}, 0);
  }

  @Test
  void anInstanceSelectionKeepsTheLimitOfTheWholeInput() {
    long units = BASE + EFFORT_PER_INPUT_BYTE * 6;
    String spec = burn(units) + "instances {\n_:int\n}";
    byte[] input = bytes("1\n2\n3\n");
    Spec.parse(spec).read(input, "input", Params.of("instance", "1"));
    String over = burn(units + 1) + "instances {\n_:int\n}";
    budgetFailure(Operation.READ, Spec.parse(over), input, Params.of("instance", "1"));
  }

  @Test
  void caughtRejectionsAndDiscardedInstancesRefundNothing() {
    String body = "instances {\n_:int\nlet v=sum(1,1,1)\ncheck 0\n}";
    byte[] input = bytes("1 2");
    long units = BASE + EFFORT_PER_INPUT_BYTE * input.length - 6;
    Spec.parse(burn(units) + body).catalog(input);
    budgetFailure(Operation.CATALOG, Spec.parse(burn(units + 1) + body), input, Params.of());
  }

  @Test
  void aGivenParameterStillRunsItsDefaultAndAddsNothing() {
    Params params = Params.of("p", "x".repeat(1000));
    String head = "param p=\"\"\nparam q=sum(1,1)\n";
    Spec spec = Spec.parse(head + burn(BASE - 2));
    spec.read(new byte[0], "input", params.with("q", "7"));
    Spec over = Spec.parse(head + burn(BASE - 1));
    budgetFailure(Operation.READ, over, new byte[0], params.with("q", "7"));
  }

  /*
   * Failures (§10.6)
   */

  @Test
  void aBudgetFailureAfterADeferredRejectionIsHard() {
    Spec spec =
        Spec.parse("_[1048576][0]:int\ninstances {_:int check 0 \"deferred\" repeat 1 {}}", "spec");
    Params params = Params.of("instance", "2");
    for (Operation operation : Operation.values()) {
      IrisError error = budgetFailure(operation, spec, bytes("0\n1"), params);
      assertEquals(IDLE_MESSAGE, error.message());
      assertEquals(new Location("input", 1, 2), error.inputLocation());
      assertEquals(new Location("spec", 2, null), error.specLocation());
      assertEquals("instance [1] > repeat [1 of 1]", error.context());
    }
  }

  @Test
  void anIdleFailureKeepsTheLineOfTheLastBodyStatement() {
    Spec spec = Spec.parse("repeat 1048576 {}\nrepeat 1 {\n  if 1 {}\n}", "spec");
    IrisError error = budgetFailure(Operation.READ, spec, new byte[0], Params.of());
    assertEquals(new Location("spec", 3, null), error.specLocation());
    assertEquals("repeat [1 of 1]", error.context());
  }

  @Test
  void anEmptyRowFailureKeepsTheNameBeingRead() {
    Spec spec = Spec.parse("\nrows[1048577][0]:int", "spec");
    IrisError error = budgetFailure(Operation.READ, spec, new byte[0], Params.of());
    assertEquals(IDLE_MESSAGE, error.message());
    assertEquals(new Location("spec", 2, null), error.specLocation());
    assertEquals("rows", error.context());
  }

  @Test
  void anEffortFailureReportsTheCursorWithoutSkipping() {
    String spec = burn(BASE + EFFORT_PER_INPUT_BYTE * 7) + "_:int\nlet v=sum(1)";
    int line = (int) spec.lines().count();
    IrisError error =
        budgetFailure(Operation.READ, Spec.parse(spec, "spec"), bytes("7  \n\n8 "), Params.of());
    assertEquals(EFFORT_MESSAGE, error.message());
    assertEquals(new Location("input", 1, 2), error.inputLocation());
    assertEquals(new Location("spec", line, null), error.specLocation());
    assertNull(error.context());
  }

  @Test
  void aLiftedFailureHasNoWitnessEvenAfterSomeRows() {
    byte[] input = bytes("1 2 3");
    String spec =
        "repeat 3 {\nx:int\n}\n" + burn(BASE + EFFORT_PER_INPUT_BYTE * 5 - 3) + "check x>0";
    int line = (int) spec.lines().count();
    IrisError error = budgetFailure(Operation.READ, Spec.parse(spec, "spec"), input, Params.of());
    assertEquals(new Location("input", 1, 6), error.inputLocation());
    assertEquals(new Location("spec", line, null), error.specLocation());
  }

  @Test
  void aChargeComesBeforeTheArithmeticOfItsArgument() {
    String input = "9223372036854775807 9223372036854775807";
    long units = BASE + EFFORT_PER_INPUT_BYTE * input.length() - 1;
    String spec = "a[2]:int\n" + burn(units) + "let v=sum(a)";
    budgetFailure(Operation.READ, Spec.parse(spec), bytes(input), Params.of());
  }

  @Test
  void aWrongTypeFailsBeforeItsCharge() {
    String input = "a b";
    long units = BASE + EFFORT_PER_INPUT_BYTE * input.length() - 1;
    Spec spec = Spec.parse("a[2]:word\n" + burn(units) + "let v=sum(a)");
    assertEquals(
        ErrorCode.FUNCTION_TYPE, assertThrows(IrisException.class, () -> spec.read(input)).code());
  }

  @Test
  void anIndexOutOfRangeFailsBeforeItsCharge() {
    String input = "1";
    long units = BASE + EFFORT_PER_INPUT_BYTE * input.length();
    Spec spec = Spec.parse("a[1]:int\n" + burn(units) + "let v=a[1]");
    assertEquals(
        ErrorCode.INDEX_RANGE, assertThrows(IrisException.class, () -> spec.read(input)).code());
  }

  /*
   * Prompt exhaustion
   */

  /**
   * Requires the same failure for the count {@code near} and the largest count, in the text {@code
   * template} of a spec where {@code COUNT} stands for it. With budgets, the far count fails as
   * soon as the near one; without them, it would run for long.
   */
  private static void requireSameFailure(String template, String input, long near) {
    Spec nearSpec = Spec.parse(template.replace("COUNT", Long.toString(near)), "spec");
    Spec farSpec = Spec.parse(template.replace("COUNT", "2147483647"), "spec");
    for (Operation operation : Operation.values()) {
      IrisError nearError = budgetFailure(operation, nearSpec, bytes(input), Params.of());
      IrisError farError = budgetFailure(operation, farSpec, bytes(input), Params.of());
      assertEquals(nearError.message(), farError.message());
      assertEquals(nearError.inputLocation(), farError.inputLocation());
      assertEquals(nearError.specLocation(), farError.specLocation());
      assertEquals(
          Objects.requireNonNull(nearError.context()).replace(" of " + near, ""),
          Objects.requireNonNull(farError.context()).replace(" of 2147483647", ""));
    }
  }

  @Test
  @Timeout(value = 60, unit = TimeUnit.SECONDS, threadMode = Timeout.ThreadMode.SEPARATE_THREAD)
  void aHugeIdleRepeatStopsAtTheLimit() {
    requireSameFailure("repeat COUNT {}", "", BASE + 1);
  }

  @Test
  @Timeout(value = 60, unit = TimeUnit.SECONDS, threadMode = Timeout.ThreadMode.SEPARATE_THREAD)
  void hugeEmptyRowsFailBeforeTheyExist() {
    requireSameFailure("_[COUNT][0]:int", "", BASE + 1);
    requireSameFailure("_[COUNT][0]:alphabet \"01\"", "", BASE + 1);
  }

  @Test
  @Timeout(value = 60, unit = TimeUnit.SECONDS, threadMode = Timeout.ThreadMode.SEPARATE_THREAD)
  void aHugeRepeatOfExpressionsStopsAtTheLimit() {
    requireSameFailure("a[1000]:int\nrepeat COUNT {\nlet v=sum(a)\n}", "0 ".repeat(1000), 100_000);
  }
}
