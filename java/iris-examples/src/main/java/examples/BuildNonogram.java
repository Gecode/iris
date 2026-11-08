package examples;

import static dev.gecode.iris.build.Iris.*;

import dev.gecode.iris.Data;
import dev.gecode.iris.Spec;
import dev.gecode.iris.build.IntExpr;
import dev.gecode.iris.build.IntField;
import dev.gecode.iris.build.RecordName;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.List;
import java.util.stream.Collectors;

/** A nonogram reader: the board size and the clues of each row and column. */
public final class BuildNonogram {
  public static final IntField<Long> width = int0("width");
  public static final IntField<Long> height = int0("height");
  public static final IntField<List<long[]>> row = int2("row");
  public static final IntField<List<long[]>> col = int2("col");

  // The lines of each kind, which the size check counts.
  private static final RecordName rows = recordName();
  private static final RecordName cols = recordName();

  public static final Spec spec =
      spec(
          "nonogram",
          () -> {
            separators(",");
            sections(
                skipOther(),
                () -> {
                  arm(pattern("width ", capture(width, atLeast(1))));
                  arm(pattern("height ", capture(height, atLeast(1))));
                  arm("rows", () -> clueLines(height, rows, row));
                  arm("columns", () -> clueLines(width, cols, col));
                });

            check(
                height.le(lit(1_000_000).div(width)),
                "the board must have at most a million fields");
            check(
                size(rows.list()).eq(height).and(size(cols.list()).eq(width)),
                "the numbers of rows and columns differ from the board size");
            check(zeroStandsAlone(row), "a 0 must be the only clue of its row");
            check(zeroStandsAlone(col), "a 0 must be the only clue of its column");
            check(
                cluesFit(row, width), "the clues of a row do not fit into the width of the board");
            check(
                cluesFit(col, height),
                "the clues of a column do not fit into the height of the board");
          });

  private BuildNonogram() {}

  /** Reads {@code count} lines of comma-separated clues into {@code clues}. */
  static void clueLines(IntField<Long> count, RecordName lines, IntField<List<long[]>> clues) {
    repeat(count, lines, () -> list(clues, atLeast(0).untilEol()));
  }

  /** A 0 clue says the line is empty, so it must stand alone. */
  static IntExpr zeroStandsAlone(IntField<List<long[]>> clues) {
    return count(clues).le(1).or(min(clues).gt(0));
  }

  /** The clues and one gap between each pair of them fit into {@code length}. */
  static IntExpr cluesFit(IntField<List<long[]>> clues, IntField<Long> length) {
    return sum(clues).plus(count(clues)).minus(1).le(length);
  }

  public static void main(String[] args) {
    Data data = spec.read(Path.of("formats/nonogram/cases/five-by-five/input.txt"));
    System.out.println(data.get(width) + "x" + data.get(height) + " board");
    System.out.println("rows: " + rows(data.get(row)));
    System.out.println("columns: " + rows(data.get(col)));
  }

  /** {@code rows} as {@code [[1, 2], [3]]}. */
  private static String rows(List<long[]> rows) {
    return rows.stream().map(Arrays::toString).collect(Collectors.joining(", ", "[", "]"));
  }
}
