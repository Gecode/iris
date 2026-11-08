package dev.gecode.iris.internal.syntax;

import dev.gecode.iris.ValueType;
import java.util.HashSet;
import java.util.Set;
import org.jspecify.annotations.Nullable;

/**
 * An entry that a successful read can leave in its data, as {@link OutputEntries} finds it.
 *
 * @param name the name of the entry
 * @param facts the value types of its leaves and the depths it can have
 * @param shape what a typed handle of the entry reads
 * @param possiblyAbsent whether a successful read can leave no such entry. {@code false} is a
 *     proof; {@code true} can also mean only that the analysis found no proof.
 */
public record OutputEntry(String name, Facts facts, Shape shape, boolean possiblyAbsent) {
  /** The value type of a typed handle. */
  public enum Leaf {
    INT("int"),
    FLOAT("float"),
    STRING("string"),
    /** Int or float, decided by the data. */
    NUM("num");

    private final String text;

    Leaf(String text) {
      this.text = text;
    }

    /** {@return the name of this value type in messages, such as {@code int}} */
    public String text() {
      return text;
    }
  }

  /** What reading an entry gives. */
  public sealed interface Shape {
    /** Values of one value type and depth. */
    record Typed(Leaf leaf, int depth) implements Shape {}

    /** Values whose value type or depth stays open, for which only a dynamic value fits. */
    record Open() implements Shape {}
  }

  /**
   * The value types and depths a value can have. A null set means that any is possible.
   *
   * @param types the value types of the leaves, or null if they are unknown
   * @param depths the depths, or null if they are unknown
   */
  public record Facts(@Nullable Set<ValueType> types, @Nullable Set<Integer> depths) {
    /** Nothing known. */
    public static final Facts UNKNOWN = new Facts(null, null);

    /** Copies the sets, so that facts stay immutable. */
    public Facts {
      types = types == null ? null : Set.copyOf(types);
      depths = depths == null ? null : Set.copyOf(depths);
    }

    /** {@return the facts of a value of one value type and depth} */
    public static Facts known(ValueType type, int depth) {
      return new Facts(Set.of(type), Set.of(depth));
    }

    /** {@return the facts of a value that has these facts or {@code other}} */
    public Facts join(Facts other) {
      return new Facts(union(types, other.types), union(depths, other.depths));
    }

    /** {@return these facts with {@code levels} added to every depth} */
    public Facts shifted(int levels) {
      if (depths == null) {
        return this;
      }
      Set<Integer> shifted = new HashSet<>();
      for (int depth : depths) {
        if (depth + levels < 0) {
          return new Facts(types, null);
        }
        shifted.add(depth + levels);
      }
      return new Facts(types, shifted);
    }

    private static <T> @Nullable Set<T> union(@Nullable Set<T> first, @Nullable Set<T> second) {
      if (first == null || second == null) {
        return null;
      }
      Set<T> union = new HashSet<>(first);
      union.addAll(second);
      return union;
    }
  }
}
