package examples;

import static dev.gecode.iris.build.Iris.*;

import dev.gecode.iris.Data;
import dev.gecode.iris.Spec;
import dev.gecode.iris.build.IntField;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.List;

/** A Sudoku reader: the grid size, the box size, and the cells, with 0 for a blank. */
public final class BuildSudoku {
  public static final IntField<Long> n = int0("n");
  public static final IntField<Long> boxWidth = int0("box_width");
  public static final IntField<Long> boxHeight = int0("box_height");
  public static final IntField<List<long[]>> cells = int2("cells");

  /** Cell values from 1 to 36, so {@code 1} reads as 1 and {@code 0} as 36. */
  private static final String SYMBOLS = "123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ0";

  public static final Spec spec =
      spec(
          "sudoku",
          () ->
              instances(() -> {
                line(
                    "# size=",
                    capture(n, atLeast(1)),
                    " box=",
                    capture(boxWidth, atLeast(1)),
                    "x",
                    capture(boxHeight, atLeast(1)));
                read(cells, alphabet(SYMBOLS).blank(".").dim(n).dim(n));
                check(boxWidth.times(boxHeight).eq(n), "boxes must tile the grid");
                check(max(cells).le(n), "a value is larger than the size of the grid");
              }));

  private BuildSudoku() {}

  public static void main(String[] args) {
    Data data = spec.read(Path.of("formats/sudoku/cases/four-by-four/input.txt"));
    long size = data.get(n);
    System.out.printf(
        "%dx%d grid with %dx%d boxes%n", size, size, data.get(boxWidth), data.get(boxHeight));
    for (long[] row : data.get(cells)) {
      System.out.println(Arrays.toString(row));
    }
  }
}
