package examples

import dev.gecode.iris.build.IntField
import dev.gecode.iris.kotlin.build.IrisSpec
import dev.gecode.iris.kotlin.build.alphabet
import dev.gecode.iris.kotlin.build.atLeast
import dev.gecode.iris.kotlin.build.eq
import dev.gecode.iris.kotlin.build.int2
import dev.gecode.iris.kotlin.build.irisSpec
import dev.gecode.iris.kotlin.build.le
import dev.gecode.iris.kotlin.build.max
import java.nio.file.Path

/** Cell values from 1 to 36, so `1` reads as 1 and `0` as 36. */
private const val SYMBOLS = "123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ0"

/** The handles of a Sudoku: the grid size, the box size, and the cells, with 0 for a blank. */
class Grid(
    val n: IntField<Long>,
    val boxWidth: IntField<Long>,
    val boxHeight: IntField<Long>,
    val cells: IntField<List<LongArray>>,
)

/** A Sudoku reader: the grid size, the box size, and the cells, with 0 for a blank. */
val sudoku: IrisSpec<Grid> =
    irisSpec("sudoku") {
        instances {
            val n by capture(atLeast(1))
            val boxWidth by capture(atLeast(1), name = "box_width")
            val boxHeight by capture(atLeast(1), name = "box_height")
            line("# size=", n, " box=", boxWidth, "x", boxHeight)
            val cells = int2("cells")
            read(cells, alphabet(SYMBOLS).blank(".").dim(n).dim(n))
            check(boxWidth * boxHeight eq n, "boxes must tile the grid")
            check(max(cells) le n, "a value is larger than the size of the grid")
            Grid(n, boxWidth, boxHeight, cells)
        }
    }

fun main() {
    val (spec, grid) = sudoku
    val data = spec.read(Path.of("formats/sudoku/cases/four-by-four/input.txt"))
    val size = data[grid.n]
    println("${size}x$size grid with ${data[grid.boxWidth]}x${data[grid.boxHeight]} boxes")
    for (row in data[grid.cells]) {
        println(row.contentToString())
    }
}
