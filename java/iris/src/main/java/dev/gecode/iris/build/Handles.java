package dev.gecode.iris.build;

/** Checks on handles that their classes share. */
final class Handles {
  private Handles() {}

  static Handle optional(Handle handle) {
    if (handle.origin() != Handle.Origin.DECLARED) {
      throw new IllegalStateException(
          "the handle `"
              + handle.name()
              + "` belongs to a builder, which fixed its presence; declare an optional handle"
              + " with a factory, as in `int0(name).optional()`");
    }
    return handle.optional();
  }
}
