package dev.gecode.iris;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;

/** What the dzn cases of the conformance suite leave out. */
class DznTest {
  private static String dzn(String spec, String input) {
    return Spec.parse(spec).read(input).toDzn();
  }

  @Test
  void writesNothingForNoEntries() {
    assertEquals("", dzn("repeat 2 {}", ""));
  }

  @Test
  void writesARowOfALargerArray() {
    assertEquals(
        "a = [| 1, 2\n  | 3, 4 |];\nrow = [3, 4];\n", dzn("a[2][2]:int let row=a[1]", "1 2 3 4"));
  }

  @Test
  void writesAMatrixOfALargerArray() {
    String spec = "a[2][2][2]:int let m=a[1]";
    assertEquals(
        "a = array3d(1..2, 1..2, 1..2, [1, 2, 3, 4, 5, 6, 7, 8]);\nm = [| 5, 6\n  | 7, 8 |];\n",
        dzn(spec, "1 2 3 4 5 6 7 8"));
  }

  @Test
  void judgesARowApartFromTheRaggedArrayAroundIt() {
    String spec = "repeat 2 {\nn:int\nlist x:int until 0\n}\nlet first=x[1]";
    assertEquals(
        """
        n = [1, 2];
        % x: array[int] of tuple(array[int] of int)
        x = [([1],), ([2, 3],)];
        first = [2, 3];
        """,
        dzn(spec, "1 1 0 2 2 3 0"));
  }

  @Test
  void judgesAPartOfANestedRaggedArrayByItsOwnRows() {
    String spec =
        "repeat 2 {\nk:int\nrepeat k {\nlist x:int until 0\n}\n}\nlet row=x[1]\nlet ragged=x[0]";
    String text = dzn(spec, "2 1 0 2 3 0 2 4 5 0 6 7 0");
    assertTrue(text.contains("row = [| 4, 5\n  | 6, 7 |];\n"), text);
    assertTrue(
        text.contains(
            "% ragged: array[int] of tuple(array[int] of int)\nragged = [([1],), ([2, 3],)];\n"),
        text);
  }

  @Test
  void aRaggedArrayWhoseSiblingsMatchIsStillRagged() {
    // The rows of each parent match, but not those under different parents.
    String spec = "repeat 2 {\nk:int\nrepeat k {\nlist x:int until 0\n}\n}";
    assertEquals(
        """
        k = [2, 2];
        % x: array[int] of tuple(array[int] of tuple(array[int] of int))
        x = [([([1],), ([2],)],), ([([3, 4],), ([5, 6],)],)];
        """,
        dzn(spec, "2 1 0 2 0 2 3 4 0 5 6 0"));
  }

  @Test
  void aSubArrayAboveTheDepthLimitOfItsTableIsRectangular() {
    String spec = "a[1][1][1][1][1][1][1][2]:int let inner=a[0]";
    assertEquals(
        "% a: array[int] of tuple(array[int] of tuple(array[int] of tuple(array[int] of tuple("
            + "array[int] of tuple(array[int] of tuple(array[int] of tuple(array[int] of int)))))))\n"
            + "a = [([([([([([([([1, 2],)],)],)],)],)],)],)];\n"
            + "% inner: array[int] of tuple(array[int] of tuple(array[int] of tuple(array[int] of "
            + "tuple(array[int] of tuple(array[int] of tuple(array[int] of int))))))\n"
            + "inner = [([([([([([([1, 2],)],)],)],)],)],)];\n",
        dzn(spec, "1 2"));
    assertTrue(
        dzn(spec + " let six=a[0][0]", "1 2")
            .contains("six = array6d(1..1, 1..1, 1..1, 1..1, 1..1, 1..2, [1, 2]);\n"));
  }

  @Test
  void writesEveryDepthWithoutRecursion() {
    int depth = 20_000;
    String text = dzn("a" + "[1]".repeat(depth) + ":int", "1");
    String value = "a = " + "[(".repeat(depth - 1) + "[1]" + ",)]".repeat(depth - 1) + ";\n";
    assertTrue(text.endsWith(value));
  }

  @Test
  void writesTheValueOfAnEmptyEntryAtDepthAboveSix() {
    assertEquals(
        "% a: array[int] of tuple(array[int] of tuple(array[int] of tuple(array[int] of tuple("
            + "array[int] of tuple(array[int] of tuple(array[int] of float))))))\na = [];\n",
        dzn("a[0][1][1][1][1][1][1]:float", ""));
  }

  @Test
  void quotesNames() {
    assertEquals(
        "'var' = 1;\n'_a' = 2;\nVar = 3;\nann1 = 4;\n",
        dzn("let var=1 let _a=2 let Var=3 let ann1=4", ""));
  }

  @Test
  void escapesStringsByteByByte() {
    assertEquals("s = \"\\x1b\\t\\\\\\\"é\u007f\";\n", dzn("s:rest", "\u001b\t\\\"é\u007f\n"));
  }
}
