package dev.gecode.iris.internal.read;

import dev.gecode.iris.ErrorCode;
import dev.gecode.iris.ValueType;
import java.util.Arrays;

/**
 * The leaves of an array being built. Their type follows the values: integers become floats if a
 * float joins them, and strings do not mix with numbers.
 */
final class Leaves {
  private static final int MAX_CAPACITY = Integer.MAX_VALUE - 8;
  private static final int INITIAL_CAPACITY = 8;

  private ValueType type;
  private int size = 0;
  private long[] ints = {};
  private double[] floats = {};
  private String[] strings = {};

  Leaves(ValueType type) {
    this.type = type;
  }

  /** Returns {@code leaves} of type {@code type}, all of them zero, to be set with {@link #set}. */
  static Leaves zeros(ValueType type, int size) {
    Leaves leaves = new Leaves(type);
    leaves.size = size;
    if (type == ValueType.INT) {
      leaves.ints = new long[size];
    } else {
      leaves.floats = new double[size];
    }
    return leaves;
  }

  /** Returns {@code size} as an array size, failing like an allocation at the limit. */
  static int arraySize(long size) {
    if (size > MAX_CAPACITY) {
      throw new OutOfMemoryError("too many values for an array");
    }
    return (int) size;
  }

  /** Returns the capacity after {@code capacity}, failing like an allocation at the limit. */
  static int grown(int capacity) {
    arraySize(capacity + 1L);
    return (int) Math.min(Math.max(2L * capacity, INITIAL_CAPACITY), MAX_CAPACITY);
  }

  ValueType type() {
    return type;
  }

  int size() {
    return size;
  }

  /** Sets the type of leaves that do not exist yet. */
  void setType(ValueType newType) {
    type = newType;
  }

  void add(long value) {
    if (size == ints.length) {
      ints = Arrays.copyOf(ints, grown(size));
    }
    ints[size++] = value;
  }

  void add(double value) {
    if (size == floats.length) {
      floats = Arrays.copyOf(floats, grown(size));
    }
    floats[size++] = value;
  }

  void add(String value) {
    if (size == strings.length) {
      strings = Arrays.copyOf(strings, grown(size));
    }
    strings[size++] = value;
  }

  /**
   * Appends the single value {@code value}, adapting the type.
   *
   * @throws ShapeError if strings and numbers would mix
   */
  void push(Stored value) {
    if (size == 0) {
      type = value.type();
    } else if ((type == ValueType.STRING) != (value.type() == ValueType.STRING)) {
      throw new ShapeError(ErrorCode.VALUE_TYPE_MIX, "strings and numbers are mixed");
    } else if (type == ValueType.INT && value.type() == ValueType.FLOAT) {
      toFloats();
    }
    switch (value) {
      case Stored.Int v -> {
        if (type == ValueType.INT) {
          add(v.value());
        } else {
          add((double) v.value());
        }
      }
      case Stored.Real v -> add(v.value());
      case Stored.Text v -> add(v.value());
      case Stored.Array _ -> throw new IllegalArgumentException("not a single value");
    }
  }

  /**
   * Appends the leaves {@code from} up to {@code to} of {@code table}, of this type or integers.
   */
  void addAll(Table table, int from, int to) {
    int count = to - from;
    switch (table.type()) {
      case INT -> {
        if (type == ValueType.INT) {
          ints = reservedInts(count);
          System.arraycopy(table.ints(), from, ints, size, count);
          size += count;
        } else {
          for (int k = from; k < to; k++) {
            add((double) table.ints()[k]);
          }
        }
      }
      case FLOAT -> {
        floats = reservedFloats(count);
        System.arraycopy(table.floats(), from, floats, size, count);
        size += count;
      }
      case STRING -> {
        strings = reservedStrings(count);
        System.arraycopy(table.strings(), from, strings, size, count);
        size += count;
      }
    }
  }

  private long[] reservedInts(int count) {
    return size + (long) count <= ints.length ? ints : Arrays.copyOf(ints, reserved(count));
  }

  private double[] reservedFloats(int count) {
    return size + (long) count <= floats.length ? floats : Arrays.copyOf(floats, reserved(count));
  }

  private String[] reservedStrings(int count) {
    return size + (long) count <= strings.length
        ? strings
        : Arrays.copyOf(strings, reserved(count));
  }

  private int reserved(int count) {
    int needed = arraySize((long) size + count);
    return (int) Math.max(needed, Math.min(2L * size, MAX_CAPACITY));
  }

  /** Converts integer leaves to floats. */
  void toFloats() {
    if (type != ValueType.INT) {
      return;
    }
    floats = new double[Math.max(size, INITIAL_CAPACITY)];
    for (int k = 0; k < size; k++) {
      floats[k] = ints[k];
    }
    ints = new long[0];
    type = ValueType.FLOAT;
  }

  /**
   * Adds {@code offset} to every number, and returns whether no integer overflowed. Adding zero
   * still matters, as it turns a float -0.0 into 0.0 (§6.7).
   */
  boolean offset(long offset) {
    if (type == ValueType.INT && offset != 0) {
      for (int k = 0; k < size; k++) {
        long sum = ints[k] + offset;
        if (((ints[k] ^ sum) & (offset ^ sum)) < 0) {
          return false;
        }
        ints[k] = sum;
      }
    } else if (type == ValueType.FLOAT) {
      for (int k = 0; k < size; k++) {
        floats[k] += (double) offset;
      }
    }
    return true;
  }

  /** Sets leaf {@code index}, of a matching type, to leaf {@code from} of {@code source}. */
  void set(int index, Leaves source, int from) {
    if (type == ValueType.INT) {
      ints[index] = source.ints[from];
    } else {
      floats[index] = source.floats[from];
    }
  }

  long[] ints() {
    return ints.length == size ? ints : Arrays.copyOf(ints, size);
  }

  double[] floats() {
    return floats.length == size ? floats : Arrays.copyOf(floats, size);
  }

  String[] strings() {
    return strings.length == size ? strings : Arrays.copyOf(strings, size);
  }
}
