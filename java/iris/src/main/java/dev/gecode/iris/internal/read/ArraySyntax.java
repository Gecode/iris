package dev.gecode.iris.internal.read;

import java.util.function.IntConsumer;

/**
 * How a document spells the nesting of an array. The outermost array is always in square brackets.
 *
 * @param open what starts an array inside another
 * @param close what ends an array inside another
 * @param separator what stands between two elements
 */
record ArraySyntax(String open, String close, String separator) {
  /**
   * Writes {@code array}, with {@code leaf} writing the leaf of each index in the table. A cursor
   * for each level takes the place of recursion, as any number of dimensions is legal (§5.12).
   */
  void write(StringBuilder out, Stored.Array array, IntConsumer leaf) {
    Table table = array.table();
    int depth = table.depth();
    int outermost = array.level();
    int[] start = new int[depth];
    int[] cursor = new int[depth];
    int[] end = new int[depth];
    int level = outermost;
    enter(table, level, array.row(), start, cursor, end);
    out.append('[');
    while (true) {
      if (cursor[level] == end[level]) {
        if (level == outermost) {
          out.append(']');
          return;
        }
        out.append(close);
        level--;
        continue;
      }
      if (cursor[level] != start[level]) {
        out.append(separator);
      }
      int child = cursor[level]++;
      if (level + 1 == depth) {
        leaf.accept(child);
      } else {
        level++;
        enter(table, level, child, start, cursor, end);
        out.append(open);
      }
    }
  }

  private static void enter(Table table, int level, int row, int[] start, int[] cursor, int[] end) {
    int[] offsets = table.offsets(level);
    start[level] = offsets[row];
    cursor[level] = offsets[row];
    end[level] = offsets[row + 1];
  }
}
