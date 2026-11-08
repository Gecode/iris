package dev.gecode.iris.internal.access;

import dev.gecode.iris.Value;
import dev.gecode.iris.ValueType;
import dev.gecode.iris.internal.syntax.OutputEntry.Leaf;
import java.lang.reflect.ParameterizedType;
import java.lang.reflect.Type;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * The Java types that typed handles and record components read values as, and the conversion into
 * them. At depth 0, {@code int}, {@code float}, {@code string} and {@code num} values are {@code
 * Long}, {@code Double}, {@code String} and {@code Number}. At depth 1 they are {@code long[]},
 * {@code double[]}, {@code List<String>} and {@code List<Number>}, and each further level adds a
 * {@code List}.
 */
public final class ViewTypes {
  private ViewTypes() {}

  /**
   * Returns {@code value} as the Java type of {@code leaf} at {@code depth}. An array without
   * leaves reads as empty whatever its type, as the column of a repeat without iterations has the
   * type of its fallback.
   *
   * @param value a value of that value type and depth
   * @param leaf the value type
   * @param depth the depth
   * @return the converted value
   * @throws IllegalStateException if the value has another value type or depth
   */
  public static Object view(Value value, Leaf leaf, int depth) {
    if (depth == 0) {
      return switch (leaf) {
        case INT -> value.asLong();
        case FLOAT -> value.asDouble();
        case STRING -> value.asString();
        case NUM -> value.type() == ValueType.INT ? (Object) value.asLong() : value.asDouble();
      };
    }
    if (depth == 1) {
      boolean empty = value.size() == 0;
      return switch (leaf) {
        case INT -> empty ? new long[0] : value.longs();
        case FLOAT -> empty ? new double[0] : value.doubles();
        case STRING -> empty ? List.of() : value.strings();
        case NUM -> empty ? List.of() : numbers(value);
      };
    }
    List<Object> rows = new ArrayList<>(value.size());
    for (int k = 0; k < value.size(); k++) {
      rows.add(view(value.get(k), leaf, depth - 1));
    }
    return Collections.unmodifiableList(rows);
  }

  /**
   * Tests whether {@code value} holds at least one leaf: it is a single value, or an array with a
   * leaf somewhere inside.
   *
   * @param value a value
   * @return whether it has a leaf
   */
  public static boolean hasLeaves(Value value) {
    if (value.depth() == 0) {
      return true;
    }
    for (int k = 0; k < value.size(); k++) {
      if (hasLeaves(value.get(k))) {
        return true;
      }
    }
    return false;
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

  /**
   * Tests whether {@code type} is the Java type of {@code leaf} at {@code depth}. At depth 0 the
   * primitive {@code long} and {@code double} are as good as {@code Long} and {@code Double}.
   *
   * @param type a type, with its type arguments
   * @param leaf the value type
   * @param depth the depth
   * @return whether values of that value type and depth read as {@code type}
   */
  public static boolean isViewType(Type type, Leaf leaf, int depth) {
    Type row = type;
    for (int level = depth; level > 1; level--) {
      if (!(row instanceof ParameterizedType list && list.getRawType() == List.class)) {
        return false;
      }
      row = list.getActualTypeArguments()[0];
    }
    if (depth > 0) {
      return switch (leaf) {
        case INT -> row == long[].class;
        case FLOAT -> row == double[].class;
        case STRING -> isListOf(row, String.class);
        case NUM -> isListOf(row, Number.class);
      };
    }
    return switch (leaf) {
      case INT -> row == Long.class || row == long.class;
      case FLOAT -> row == Double.class || row == double.class;
      case STRING -> row == String.class;
      case NUM -> row == Number.class;
    };
  }

  private static boolean isListOf(Type type, Class<?> element) {
    return type instanceof ParameterizedType list
        && list.getRawType() == List.class
        && list.getActualTypeArguments()[0] == element;
  }

  /**
   * Names the Java types of {@code leaf} at {@code depth}, as in {@code List<long[]>}, or {@code
   * long, Long} at depth 0.
   *
   * @param leaf the value type
   * @param depth the depth
   * @return the names
   */
  public static String name(Leaf leaf, int depth) {
    String innermost =
        switch (leaf) {
          case INT -> depth == 0 ? "long, Long" : "long[]";
          case FLOAT -> depth == 0 ? "double, Double" : "double[]";
          case STRING -> depth == 0 ? "String" : "List<String>";
          case NUM -> depth == 0 ? "Number" : "List<Number>";
        };
    int lists = Math.max(depth - 1, 0);
    return "List<".repeat(lists) + innermost + ">".repeat(lists);
  }
}
