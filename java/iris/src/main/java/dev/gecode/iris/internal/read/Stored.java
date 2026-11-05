package dev.gecode.iris.internal.read;

import dev.gecode.iris.ValueType;
import java.util.Arrays;

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

    /** Returns whether {@code other} has the type, the shape and the leaves of this array. */
    public boolean hasSameContent(Array other) {
      return type() == other.type()
          && depth() == other.depth()
          && hasSameShape(other)
          && hasSameLeaves(other);
    }

    /** Compares the sizes of the rows, level by level, as the depth has no bound. */
    private boolean hasSameShape(Array other) {
      int first = row;
      int end = row + 1;
      int otherFirst = other.row;
      for (int down = 0; down < depth(); down++) {
        int[] offsets = table.offsets(level + down);
        int[] otherOffsets = other.table.offsets(other.level + down);
        for (int k = 0; k < end - first; k++) {
          if (offsets[first + k + 1] - offsets[first + k]
              != otherOffsets[otherFirst + k + 1] - otherOffsets[otherFirst + k]) {
            return false;
          }
        }
        end = offsets[end];
        first = offsets[first];
        otherFirst = otherOffsets[otherFirst];
      }
      return true;
    }

    private boolean hasSameLeaves(Array other) {
      int first = firstLeaf();
      int end = endLeaf();
      int otherFirst = other.firstLeaf();
      int otherEnd = other.endLeaf();
      return switch (type()) {
        case INT ->
            Arrays.equals(table.ints(), first, end, other.table.ints(), otherFirst, otherEnd);
        case FLOAT ->
            Arrays.equals(table.floats(), first, end, other.table.floats(), otherFirst, otherEnd);
        case STRING ->
            Arrays.equals(table.strings(), first, end, other.table.strings(), otherFirst, otherEnd);
      };
    }

    /** Returns a hash that arrays with the same content share. */
    public int contentHash() {
      int hash = 31 * type().hashCode() + depth();
      int first = row;
      int end = row + 1;
      for (int down = 0; down < depth(); down++) {
        int[] offsets = table.offsets(level + down);
        for (int k = first; k < end; k++) {
          hash = 31 * hash + offsets[k + 1] - offsets[k];
        }
        end = offsets[end];
        first = offsets[first];
      }
      for (int k = firstLeaf(); k < endLeaf(); k++) {
        hash = 31 * hash + table.leaf(k).hashCode();
      }
      return hash;
    }
  }
}
