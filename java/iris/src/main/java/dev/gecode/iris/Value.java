package dev.gecode.iris;

import dev.gecode.iris.internal.read.CanonicalJson;
import dev.gecode.iris.internal.read.Stored;
import dev.gecode.iris.internal.read.Table;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import org.jspecify.annotations.Nullable;

/**
 * A value that a read produced: a single integer, float or string, or an array of them, which can
 * be nested and ragged (§3.2).
 *
 * <p>Asking a value for something it is not, such as the integer of a string, is a mistake in the
 * calling code, never an {@link IrisException}. It throws {@link IllegalStateException} or, for an
 * index out of range, {@link IndexOutOfBoundsException}.
 *
 * <p>Values are equal if they have the same type, shape and leaves, where floats compare as {@link
 * Double#equals} does.
 */
public sealed interface Value {
  /** {@return the type of the leaves} */
  ValueType type();

  /** {@return the depth: 0 for a single value, and otherwise the levels of nesting} */
  default int depth() {
    return 0;
  }

  /** {@return the number of elements of this array} */
  default int size() {
    throw notAnArray();
  }

  /**
   * Gets an element of this array.
   *
   * @param index the index of the element, counted from 0
   * @return the element
   */
  default Value get(int index) {
    throw notAnArray();
  }

  /**
   * Follows indices into this value.
   *
   * @param path the indices to select in turn
   * @return what they select, or this value if there are none
   */
  default Value at(int... path) {
    Value value = this;
    for (int index : path) {
      value = value.get(index);
    }
    return value;
  }

  /** {@return this single integer} */
  default long asLong() {
    throw wrongType(ValueType.INT);
  }

  /** {@return this single number, as a float if it is an integer} */
  default double asDouble() {
    throw wrongType(ValueType.FLOAT);
  }

  /** {@return this single string} */
  default String asString() {
    throw wrongType(ValueType.STRING);
  }

  /** {@return a copy of the integers of this one-dimensional array} */
  default long[] longs() {
    throw notAnArray();
  }

  /**
   * Narrows the integers of this one-dimensional array.
   *
   * @return a copy of the integers
   * @throws ArithmeticException if an integer does not fit an {@code int}
   */
  default int[] ints() {
    long[] values = longs();
    int[] narrowed = new int[values.length];
    for (int k = 0; k < values.length; k++) {
      narrowed[k] = Math.toIntExact(values[k]);
    }
    return narrowed;
  }

  /** {@return a copy of the numbers of this one-dimensional array, integers as floats} */
  default double[] doubles() {
    throw notAnArray();
  }

  /** {@return the strings of this one-dimensional array, as an unmodifiable list} */
  default List<String> strings() {
    throw notAnArray();
  }

  private static IllegalStateException notAnArray() {
    return new IllegalStateException("the value is not an array");
  }

  private IllegalStateException wrongType(ValueType wanted) {
    return new IllegalStateException(
        depth() > 0
            ? "the value is an array, not a single value"
            : "the value is of type " + type().text() + ", not " + wanted.text());
  }

  /**
   * A single integer.
   *
   * @param value the integer
   */
  record IntValue(long value) implements Value {
    @Override
    public ValueType type() {
      return ValueType.INT;
    }

    @Override
    public long asLong() {
      return value;
    }

    @Override
    public double asDouble() {
      return value;
    }
  }

  /**
   * A single float.
   *
   * @param value the float
   */
  record FloatValue(double value) implements Value {
    @Override
    public ValueType type() {
      return ValueType.FLOAT;
    }

    @Override
    public double asDouble() {
      return value;
    }
  }

  /**
   * A single string.
   *
   * @param value the string
   */
  record StringValue(String value) implements Value {
    @Override
    public ValueType type() {
      return ValueType.STRING;
    }

    @Override
    public String asString() {
      return value;
    }
  }

  /**
   * An array, or a row of a larger array, whose elements stay in place until they are asked for.
   */
  final class ArrayValue implements Value {
    private final Stored.Array array;

    private ArrayValue(Stored.Array array) {
      this.array = array;
    }

    /** Returns {@code value} as a public value. */
    static Value of(Stored value) {
      return switch (value) {
        case Stored.Int v -> new IntValue(v.value());
        case Stored.Real v -> new FloatValue(v.value());
        case Stored.Text v -> new StringValue(v.value());
        case Stored.Array v -> new ArrayValue(v);
      };
    }

    @Override
    public ValueType type() {
      return array.table().type();
    }

    @Override
    public int depth() {
      return array.depth();
    }

    @Override
    public int size() {
      return array.size();
    }

    @Override
    public Value get(int index) {
      return of(array.element(checkedIndex(index)));
    }

    private int checkedIndex(int index) {
      if (index < 0 || index >= size()) {
        throw new IndexOutOfBoundsException(
            "index " + index + " is out of range (size " + size() + ")");
      }
      return index;
    }

    @Override
    public long[] longs() {
      requireRow(ValueType.INT);
      return Arrays.copyOfRange(array.table().ints(), array.firstLeaf(), array.endLeaf());
    }

    @Override
    public double[] doubles() {
      if (array.table().type() == ValueType.INT) {
        return Arrays.stream(longs()).asDoubleStream().toArray();
      }
      requireRow(ValueType.FLOAT);
      return Arrays.copyOfRange(array.table().floats(), array.firstLeaf(), array.endLeaf());
    }

    @Override
    public List<String> strings() {
      requireRow(ValueType.STRING);
      List<String> leaves = Arrays.asList(array.table().strings());
      return Collections.unmodifiableList(leaves.subList(array.firstLeaf(), array.endLeaf()));
    }

    private void requireRow(ValueType wanted) {
      Table table = array.table();
      if (depth() != 1) {
        throw new IllegalStateException("the value is not a one-dimensional array");
      }
      if (table.type() != wanted) {
        throw new IllegalStateException(
            "the array is of type " + table.type().text() + ", not " + wanted.text());
      }
    }

    @Override
    public boolean equals(@Nullable Object other) {
      return other instanceof ArrayValue value && array.hasSameContent(value.array);
    }

    @Override
    public int hashCode() {
      return array.contentHash();
    }

    /** {@return the array in canonical JSON} */
    @Override
    public String toString() {
      return CanonicalJson.value(array);
    }
  }
}
