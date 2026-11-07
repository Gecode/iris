package dev.gecode.iris.build;

import org.jspecify.annotations.Nullable;

/**
 * What a handle names and promises.
 *
 * @param name the entry name
 * @param tag the value type
 * @param levels the total depth, or null when it is open
 * @param required whether the entry is always present
 * @param origin how the handle was made
 * @param owner the builder that owns it, or 0 for a declared handle, which any builder may use
 * @param valueView whether {@code Data.get} returns a {@code Value} rather than a typed view
 */
record Handle(
    String name,
    Tag tag,
    @Nullable Integer levels,
    boolean required,
    Origin origin,
    long owner,
    boolean valueView) {

  /** How a handle was made. */
  enum Origin {
    /** A factory such as {@code int0}: any builder may use it, and its first use declares it. */
    DECLARED,
    /** {@code forward}, which declares the name in its builder. */
    FORWARD,
    /** {@code undefined}, a name that nothing may define. */
    UNDEFINED,
    /** What a definition returns: the name as it stands after that definition. */
    REFERENCE
  }

  static Handle declared(String name, Tag tag, @Nullable Integer levels, boolean valueView) {
    return new Handle(name, tag, levels, true, Origin.DECLARED, 0, valueView);
  }

  Handle optional() {
    return new Handle(name, tag, levels, false, origin, owner, valueView);
  }

  /** The handle a definition of this name returns in the builder {@code owner}. */
  static Handle reference(String name, long owner) {
    return new Handle(name, Tag.DYN, null, false, Origin.REFERENCE, owner, true);
  }

  Shape shape() {
    return new Shape(tag, levels, required);
  }

  /** The value type and depth, as messages describe them. */
  String describe() {
    return Shape.describe(tag, levels, "any");
  }
}
