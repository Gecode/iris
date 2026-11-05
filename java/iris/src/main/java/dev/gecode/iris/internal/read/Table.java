package dev.gecode.iris.internal.read;

import dev.gecode.iris.ValueType;

/**
 * The storage of an array: flat leaves of one type, with a level of cumulative offsets for each
 * level of nesting.
 *
 * <p>Level 0 is {0, n} for the outermost row of n elements. Level k has one offset more than there
 * are rows at depth k, so that row j of level k holds the rows or leaves {@code levels[k][j]} up to
 * {@code levels[k][j + 1]} of the level below. For example, [[1, 2], [], [3]] has the levels {0, 3}
 * and {0, 2, 2, 3} and the leaves 1, 2, 3.
 *
 * <p>A table never changes once it is made, so that values can share it. Any number of dimensions
 * is legal (§5.12), so nothing here recurses over the levels, and {@link #firstLeaf} takes no walk
 * down them, however many there are.
 */
public final class Table {
  private static final long[] NO_INTS = {};
  private static final double[] NO_FLOATS = {};
  private static final String[] NO_STRINGS = {};

  private final ValueType type;
  private final int[][] levels;

  /** For each level, where the leaves of each of its rows start. */
  private final int[][] firstLeaves;

  private final long[] ints;
  private final double[] floats;
  private final String[] strings;

  /**
   * Makes the table of {@code leaves} with the structure {@code levels}, which it takes over. There
   * is at least one level.
   */
  Table(int[][] levels, Leaves leaves) {
    this.type = leaves.type();
    this.levels = levels;
    this.firstLeaves = firstLeaves(levels);
    this.ints = type == ValueType.INT ? leaves.ints() : NO_INTS;
    this.floats = type == ValueType.FLOAT ? leaves.floats() : NO_FLOATS;
    this.strings = type == ValueType.STRING ? leaves.strings() : NO_STRINGS;
  }

  private static int[][] firstLeaves(int[][] levels) {
    int[][] firstLeaves = new int[levels.length][];
    firstLeaves[levels.length - 1] = levels[levels.length - 1];
    for (int level = levels.length - 2; level >= 0; level--) {
      int[] offsets = levels[level];
      int[] below = firstLeaves[level + 1];
      int[] first = new int[offsets.length];
      for (int row = 0; row < offsets.length; row++) {
        first[row] = below[offsets[row]];
      }
      firstLeaves[level] = first;
    }
    return firstLeaves;
  }

  /** Returns the row of {@code leaves}. */
  static Stored.Array row(Leaves leaves) {
    return new Table(new int[][] {{0, leaves.size()}}, leaves).root();
  }

  /** Returns the array with dimensions {@code dimensions} around {@code leaves}. */
  static Stored.Array shaped(int[] dimensions, Leaves leaves) {
    int[][] levels = new int[dimensions.length][];
    int rows = 1;
    for (int level = 0; level < dimensions.length; level++) {
      int[] offsets = new int[Leaves.arraySize(rows + 1L)];
      for (int row = 1; row <= rows; row++) {
        offsets[row] = offsets[row - 1] + dimensions[level];
      }
      levels[level] = offsets;
      rows = offsets[rows];
    }
    return new Table(levels, leaves).root();
  }

  /** Returns the array that is the whole table. */
  Stored.Array root() {
    return new Stored.Array(this, 0, 0);
  }

  /** Returns the type of the leaves. */
  public ValueType type() {
    return type;
  }

  /** Returns the number of levels. */
  public int depth() {
    return levels.length;
  }

  /** Returns the offsets of {@code level}. */
  public int[] offsets(int level) {
    return levels[level];
  }

  /** Returns where the leaves of row {@code row} of {@code level} start. */
  int firstLeaf(int level, int row) {
    return firstLeaves[level][row];
  }

  /** Returns the integers, which are empty unless the leaves are integers. */
  public long[] ints() {
    return ints;
  }

  /** Returns the floats, which are empty unless the leaves are floats. */
  public double[] floats() {
    return floats;
  }

  /** Returns the strings, which are empty unless the leaves are strings. */
  public String[] strings() {
    return strings;
  }

  /** Returns leaf {@code index}. */
  Stored leaf(int index) {
    return switch (type) {
      case INT -> new Stored.Int(ints[index]);
      case FLOAT -> new Stored.Real(floats[index]);
      case STRING -> new Stored.Text(strings[index]);
    };
  }
}
