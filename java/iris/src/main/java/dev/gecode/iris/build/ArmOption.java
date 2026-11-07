package dev.gecode.iris.build;

/** An option of a {@code sections} arm. */
public enum ArmOption {
  /** {@code optional}: the arm may not match, so what it defines may be absent. */
  OPTIONAL,
  /** {@code last}: the sections end after this arm. */
  LAST
}
