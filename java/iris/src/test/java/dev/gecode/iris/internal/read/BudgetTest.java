package dev.gecode.iris.internal.read;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;

class BudgetTest {
  private static final long BASE = 1L << 20;

  @Test
  void allowsIdleWorkUpToTheLimitWhateverTheInput() {
    var budget = new Budget(1000);
    assertTrue(budget.chargeIdle(BASE));
    assertTrue(budget.chargeIdle(0));
    assertFalse(budget.chargeIdle(1));
  }

  @Test
  void refusesOnePastTheIdleLimit() {
    assertFalse(new Budget(1000).chargeIdle(BASE + 1));
  }

  @Test
  void allowsEffortUpToTheLimitOfTheInput() {
    var budget = new Budget(3);
    assertTrue(budget.chargeEffort(BASE + 48));
    assertTrue(budget.chargeEffort(0));
    assertFalse(budget.chargeEffort(1));
  }

  @Test
  void refusesOnePastTheEffortLimit() {
    assertFalse(new Budget(3).chargeEffort(BASE + 49));
  }

  @Test
  void addsChargesUp() {
    var budget = new Budget(0);
    assertTrue(budget.chargeIdle(BASE - 1));
    assertFalse(budget.chargeIdle(2));
    assertTrue(budget.chargeEffort(BASE - 1));
    assertFalse(budget.chargeEffort(2));
  }

  @Test
  void aRefusedChargeTakesNothing() {
    var budget = new Budget(0);
    assertFalse(budget.chargeIdle(BASE + 1));
    assertTrue(budget.chargeIdle(BASE));
    assertFalse(budget.chargeEffort(BASE + 1));
    assertTrue(budget.chargeEffort(BASE));
  }

  @Test
  void countsTheBudgetsApart() {
    var budget = new Budget(0);
    assertTrue(budget.chargeIdle(BASE));
    assertTrue(budget.chargeEffort(BASE));
  }

  @Test
  void refusesTheLargestChargeWithoutOverflowing() {
    var budget = new Budget(Integer.MAX_VALUE);
    assertFalse(budget.chargeIdle(Long.MAX_VALUE));
    assertFalse(budget.chargeEffort(Long.MAX_VALUE));
    assertTrue(budget.chargeEffort(BASE + 16L * Integer.MAX_VALUE));
  }
}
