package dev.gecode.iris.internal.read;

import dev.gecode.iris.ErrorCode;
import dev.gecode.iris.ValueType;

/**
 * An array built row by row from the values of a repeated statement.
 *
 * <p>Row r is the value the name had in iteration r. For example, the rows [1, 2], [] and [3] give
 * a value of depth 2. A missing row, from an iteration that did not define the name, becomes an
 * empty list, which needs rows that are lists. Depth and type come from the rows added. A column
 * without rows takes them from the statement defining the name.
 */
final class Column {
  private final Leaves leaves;

  /** The offsets below the outermost level, which are the same for every column. */
  private IntList[] levels = {};

  /** The depth of the rows. */
  private int depth;

  private int rows = 0;

  /** Missing rows before the first row added. */
  private int pending = 0;

  private boolean isStarted = false;
  private boolean isMissingRows = false;

  Column(ValueType type, int depth) {
    this.leaves = new Leaves(type);
    this.depth = depth;
  }

  private void startLevels() {
    levels = new IntList[depth];
    for (int k = 0; k < depth; k++) {
      levels[k] = new IntList();
      levels[k].add(0);
    }
  }

  /**
   * Adds a missing row.
   *
   * @throws ShapeError with a phrase completing the name
   */
  void skip() {
    rows++;
    isMissingRows = true;
    if (!isStarted) {
      pending++;
    } else if (depth == 0) {
      throw new ShapeError(ErrorCode.REPEAT_MISSING, "is defined in only some repetitions");
    } else {
      levels[0].add(levels[0].last());
    }
  }

  /**
   * Adds the row {@code row}.
   *
   * @throws ShapeError with a phrase completing the name
   */
  void add(Stored row) {
    rows++;
    if (!isStarted) {
      if (isMissingRows && row.depth() == 0) {
        throw new ShapeError(ErrorCode.REPEAT_MISSING, "is defined in only some repetitions");
      }
      isStarted = true;
      depth = row.depth();
      leaves.setType(row.type());
      startLevels();
      for (int k = 0; k < pending; k++) {
        levels[0].add(0);
      }
    } else if (row.depth() != depth) {
      throw new ShapeError(ErrorCode.REPEAT_DEPTH, "has rows of different depths");
    }
    switch (row) {
      case Stored.Array array -> addArray(array);
      case Stored.Int _, Stored.Real _, Stored.Text _ -> addScalar(row);
    }
  }

  private void addScalar(Stored row) {
    requireCompatible(row);
    leaves.push(row);
  }

  private void addArray(Stored.Array array) {
    if (array.leaves() > 0) {
      requireCompatible(array);
    }
    // The rows of the array follow those of the earlier arrays one level down.
    Table table = array.table();
    int from = array.row();
    int to = from + 1;
    for (int k = array.level(); k < table.depth(); k++) {
      IntList offsets = levels[k - array.level()];
      int[] rowOffsets = table.offsets(k);
      int base = offsets.last() - rowOffsets[from];
      for (int j = from + 1; j <= to; j++) {
        offsets.add(base + rowOffsets[j]);
      }
      from = rowOffsets[from];
      to = rowOffsets[to];
    }
    leaves.addAll(table, from, to);
  }

  /** Follows the values with the type: integers become floats if needed. */
  private void requireCompatible(Stored row) {
    if (leaves.size() == 0) {
      leaves.setType(row.type());
    } else if ((leaves.type() == ValueType.STRING) != (row.type() == ValueType.STRING)) {
      throw new ShapeError(ErrorCode.VALUE_TYPE_MIX, "mixes strings and numbers");
    } else if (leaves.type() == ValueType.INT && row.type() == ValueType.FLOAT) {
      leaves.toFloats();
    }
  }

  /**
   * Returns the array.
   *
   * @throws ShapeError with a phrase completing the name
   */
  Stored finish() {
    if (!isStarted) {
      // No rows added: every row is missing, so empty if it is a list.
      if (depth == 0 && rows > 0) {
        throw new ShapeError(ErrorCode.REPEAT_MISSING, "is defined in no repetition");
      }
      startLevels();
      for (int k = 0; depth > 0 && k < rows; k++) {
        levels[0].add(0);
      }
    }
    int[][] offsets = new int[depth + 1][];
    offsets[0] = new int[] {0, rows};
    for (int k = 0; k < depth; k++) {
      offsets[k + 1] = levels[k].toArray();
    }
    return new Table(offsets, leaves).root();
  }
}
