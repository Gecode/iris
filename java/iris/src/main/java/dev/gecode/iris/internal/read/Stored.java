package dev.gecode.iris.internal.read;

import dev.gecode.iris.ValueType;

/**
 * A value during a read: a single value, or an array that is a row of a {@link Table}.
 *
 * <p>Values never change, so that defining a name to an existing value, and selecting a part of an
 * array, take no copy of its leaves. Costs that §10.3 charges by the leaves are therefore bounds
 * here, not the work done.
 */
public sealed interface Stored {
  /** Returns the type of the leaves. */
  ValueType type();

  /** Returns the number of levels of nesting, 0 for a single value. */
  default int depth() {
    return 0;
  }

  /** Returns the number of leaves. */
  default int leaves() {
    return 1;
  }

  /** Returns this single number as a float. */
  default double asDouble() {
    throw new IllegalStateException("the value is not a single number");
  }

  /** A single integer. */
  record Int(long value) implements Stored {
    @Override
    public ValueType type() {
      return ValueType.INT;
    }

    @Override
    public double asDouble() {
      return value;
    }
  }

  /** A single float. */
  record Real(double value) implements Stored {
    @Override
    public ValueType type() {
      return ValueType.FLOAT;
    }

    @Override
    public double asDouble() {
      return value;
    }
  }

  /** A single string. */
  record Text(String value) implements Stored {
    @Override
    public ValueType type() {
      return ValueType.STRING;
    }
  }

  /**
   * An array.
   *
   * @param table the storage
   * @param level the level of its row
   * @param row the index of its row in that level
   */
  record Array(Table table, int level, int row) implements Stored {
    @Override
    public ValueType type() {
      return table.type();
    }

    @Override
    public int depth() {
      return table.depth() - level;
    }

    @Override
    public int leaves() {
      return endLeaf() - firstLeaf();
    }

    /** Returns the number of elements. */
    public int size() {
      int[] offsets = table.offsets(level);
      return offsets[row + 1] - offsets[row];
    }

    /** Returns the first of the leaves in the table. */
    public int firstLeaf() {
      return table.firstLeaf(level, row);
    }

    /** Returns the end of the leaves in the table. */
    public int endLeaf() {
      return table.firstLeaf(level, row + 1);
    }

    /** Returns element {@code index}, counted from 0, which must exist. */
    public Stored element(int index) {
      int position = table.offsets(level)[row] + index;
      return level + 1 == table.depth()
          ? table.leaf(position)
          : new Array(table, level + 1, position);
    }
  }
}
