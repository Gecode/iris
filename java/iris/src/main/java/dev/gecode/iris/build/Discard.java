package dev.gecode.iris.build;

/** The discard {@code _}: a target that reads without storing, and {@code {_}} in a pattern. */
public final class Discard {
  static final Discard INSTANCE = new Discard();

  private Discard() {}

  @Override
  public String toString() {
    return "_";
  }
}
