package dev.gecode.iris.build;

/** The host constants of Gecode. */
public final class Gecode {
  private Gecode() {}

  /** {@code gecode.int_max}, the largest integer of a Gecode integer variable. */
  public static final IntExpr intMax = Exprs.intExpr(new Node(new Op.HostOp("gecode.int_max")));

  /** {@code gecode.int_min}, the smallest integer of a Gecode integer variable. */
  public static final IntExpr intMin = Exprs.intExpr(new Node(new Op.HostOp("gecode.int_min")));
}
