package dev.gecode.iris.build;

import dev.gecode.iris.Data;
import dev.gecode.iris.Value;
import dev.gecode.iris.ValueType;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
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
    return view(value, levels, handle.tag());
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

  private static List<Number> numbers(Value value) {
    List<Number> numbers = new ArrayList<>(value.size());
    if (value.type() == ValueType.INT) {
      for (long number : value.longs()) {
        numbers.add(number);
      }
    } else {
      for (double number : value.doubles()) {
        numbers.add(number);
      }
    }
    return Collections.unmodifiableList(numbers);
  }

  private static Object view(Value value, int levels, Tag tag) {
    if (levels == 0) {
      return switch (tag) {
        case INT -> value.asLong();
        case FLOAT -> value.asDouble();
        case STR -> value.asString();
        default -> value.type() == ValueType.INT ? (Object) value.asLong() : value.asDouble();
      };
    }
    if (levels == 1) {
      return switch (tag) {
        case INT -> value.longs();
        case FLOAT -> value.doubles();
        case STR -> value.strings();
        default -> numbers(value);
      };
    }
    List<Object> rows = new ArrayList<>(value.size());
    for (int k = 0; k < value.size(); k++) {
      rows.add(view(value.get(k), levels - 1, tag));
    }
    return Collections.unmodifiableList(rows);
  }
}
