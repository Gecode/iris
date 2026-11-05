package dev.gecode.iris.internal.read;

/**
 * The work budgets of a read (§10): idle work reads no input, and effort visits values in
 * expressions.
 *
 * <p>Every site that §10 charges calls this class, but neither budget is enforced yet: each charge
 * succeeds. A read that needs a budget to end may therefore run until it exhausts time or memory.
 */
final class Budget {
  /** Charges {@code units} of idle work, and returns whether the budget allows them (§10.2). */
  boolean chargeIdle(long units) {
    return units >= 0;
  }

  /** Charges {@code units} of effort, and returns whether the budget allows them (§10.3). */
  boolean chargeEffort(long units) {
    return units >= 0;
  }
}
