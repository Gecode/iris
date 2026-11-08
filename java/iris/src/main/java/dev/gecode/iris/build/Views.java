package dev.gecode.iris.build;

import dev.gecode.iris.Data;
import dev.gecode.iris.Value;
import dev.gecode.iris.ValueType;
import dev.gecode.iris.internal.access.ViewTypes;
import dev.gecode.iris.internal.syntax.OutputEntry.Leaf;
import org.jspecify.annotations.Nullable;

/** Typed access to data through a handle. */
final class Views {
  private Views() {}

  /** The value of {@code handle}'s entry as the Java type the handle promises. */
  static @Nullable Object read(Handle handle, Data data) {
    Value value = data.find(handle.name());
    if (value == null) {
      if (!handle.required()) {
        return null;
      }
      throw new IllegalStateException("no value named '" + handle.name() + "'");
    }
    if (handle.tag() == Tag.DYN) {
      return value;
    }
    Integer levels = handle.levels();
    if (!matches(handle.tag(), value.type()) || (levels != null && levels != value.depth())) {
      throw new IllegalStateException(
          "the entry '"
              + handle.name()
              + "' is "
              + value.type().text()
              + " at depth "
              + value.depth()
              + ", but its handle reads "
              + handle.describe());
    }
    if (handle.valueView() || levels == null) {
      return value;
    }
    return ViewTypes.view(value, leaf(handle.tag()), levels);
  }

  private static boolean matches(Tag tag, ValueType type) {
    return switch (tag) {
      case INT -> type == ValueType.INT;
      case FLOAT -> type == ValueType.FLOAT;
      case STR -> type == ValueType.STRING;
      case NUM -> type == ValueType.INT || type == ValueType.FLOAT;
      case DYN -> true;
    };
  }

  /** The value type of a handle with a static type. */
  private static Leaf leaf(Tag tag) {
    return switch (tag) {
      case INT -> Leaf.INT;
      case FLOAT -> Leaf.FLOAT;
      case STR -> Leaf.STRING;
      case NUM -> Leaf.NUM;
      case DYN -> throw new IllegalArgumentException("a dynamic handle has no value type");
    };
  }
}
