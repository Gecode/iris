package examples

import dev.gecode.iris.build.IntExpr
import dev.gecode.iris.build.IntField
import dev.gecode.iris.build.RecordName
import dev.gecode.iris.kotlin.build.IrisSpec
import dev.gecode.iris.kotlin.build.Scope0
import dev.gecode.iris.kotlin.build.and
import dev.gecode.iris.kotlin.build.atLeast
import dev.gecode.iris.kotlin.build.count
import dev.gecode.iris.kotlin.build.eq
import dev.gecode.iris.kotlin.build.gt
import dev.gecode.iris.kotlin.build.int2
import dev.gecode.iris.kotlin.build.irisSpec
import dev.gecode.iris.kotlin.build.le
import dev.gecode.iris.kotlin.build.lit
import dev.gecode.iris.kotlin.build.min
import dev.gecode.iris.kotlin.build.or
import dev.gecode.iris.kotlin.build.size
import dev.gecode.iris.kotlin.build.sum
import java.nio.file.Path

private const val MAX_FIELDS = 1_000_000L

/** The handles of a nonogram: the board size and the clues of each row and column. */
class Board(
    val width: IntField<Long>,
    val height: IntField<Long>,
    val row: IntField<List<LongArray>>,
    val col: IntField<List<LongArray>>,
)

/** A nonogram reader: the board size and the clues of each row and column. */
val nonogram: IrisSpec<Board> =
    irisSpec("nonogram") {
        separators(",")
        val width by capture(atLeast(1))
        val height by capture(atLeast(1))
        val row = int2("row")
        val col = int2("col")
        val (rows, cols) =
            sections(skipOther = true) {
                arm("width ", width)
                arm("height ", height)
                val rows = arm("rows") { clueLines(height, row) }
                val cols = arm("columns") { clueLines(width, col) }
                rows to cols
            }

        check(height le lit(MAX_FIELDS) / width, "the board must have at most a million fields")
        check(
            (size(rows.list()) eq height) and (size(cols.list()) eq width),
            "the numbers of rows and columns differ from the board size",
        )
        check(zeroStandsAlone(row), "a 0 must be the only clue of its row")
        check(zeroStandsAlone(col), "a 0 must be the only clue of its column")
        check(cluesFit(row, width), "the clues of a row do not fit into the width of the board")
        check(
            cluesFit(col, height),
            "the clues of a column do not fit into the height of the board",
        )
        Board(width, height, row, col)
    }

/** Reads [count] lines of comma-separated clues into [clues], and returns their records. */
private fun Scope0.clueLines(count: IntField<Long>, clues: IntField<List<LongArray>>): RecordName =
    repeat(count) {
        list(clues, atLeast(0).untilEol())
        record
    }

/** A 0 clue says the line is empty, so it must stand alone. */
private fun zeroStandsAlone(clues: IntField<List<LongArray>>): IntExpr =
    (count(clues) le 1) or (min(clues) gt 0)

/** The clues and one gap between each pair of them fit into [length]. */
private fun cluesFit(clues: IntField<List<LongArray>>, length: IntField<Long>): IntExpr =
    sum(clues) + count(clues) - 1 le length

fun main() {
    val (spec, board) = nonogram
    val data = spec.read(Path.of("formats/nonogram/cases/five-by-five/input.txt"))
    println("${data[board.width]}x${data[board.height]} board")
    println("rows: ${clues(data[board.row])}")
    println("columns: ${clues(data[board.col])}")
}

/** [lines] as `[[1, 2], [3]]`. */
private fun clues(lines: List<LongArray>): String =
    lines.joinToString(", ", "[", "]") { it.contentToString() }
