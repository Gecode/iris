package dev.gecode.iris.internal.read;

/**
 * The work budgets of one read or catalog (§10): idle work reads no input, and effort visits values
 * in expressions.
 *
 * <p>Each budget keeps the units still allowed. A charge is compared whole with them, so that it
 * cannot overflow however large it is (§10.1), and a charge that fails ends the operation.
 */
final class Budget {
  /** B, the idle limit and the part of the effort limit that does not depend on the input. */
  private static final long BASE_LIMIT = 1L << 20;

  private static final long EFFORT_PER_INPUT_BYTE = 16;

  private long idleLeft = BASE_LIMIT;
  private long effortLeft;

  /** Makes the budgets of an input of {@code inputBytes} bytes, which a byte array can hold. */
  Budget(long inputBytes) {
    effortLeft = BASE_LIMIT + EFFORT_PER_INPUT_BYTE * inputBytes;
  }

  /** Charges {@code units} of idle work, and returns whether the budget allows them (§10.2). */
  boolean chargeIdle(long units) {
    if (units > idleLeft) {
      return false;
    }
    idleLeft -= units;
    return true;
  }

  /** Charges {@code units} of effort, and returns whether the budget allows them (§10.3). */
  boolean chargeEffort(long units) {
    if (units > effortLeft) {
      return false;
    }
    effortLeft -= units;
    return true;
  }
}
