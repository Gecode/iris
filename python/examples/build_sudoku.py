from __future__ import annotations

from typing import NamedTuple

import gecode_iris as iris
import gecode_iris.build as ib


class Sudoku(NamedTuple):
    """A Sudoku reader: the grid size, the box size, and the cells, with 0 for a
    blank."""

    spec: iris.Spec
    n: ib.IntField[int]
    box_width: ib.IntField[int]
    box_height: ib.IntField[int]
    cells: ib.IntField[list[list[int]]]


SYMBOLS = "123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ0"
"""Cell values from 1 to 36, so ``1`` reads as 1 and ``0`` as 36."""


def sudoku() -> Sudoku:
    b = ib.SpecBuilder("sudoku")
    with b.instances() as i:
        n = i.capture("n", ib.at_least(1))
        box_width = i.capture("box_width", ib.at_least(1))
        box_height = i.capture("box_height", ib.at_least(1))
        i.line(("# size=", n, " box=", box_width, "x", box_height))
        cells = i.read("cells", ib.alphabet(SYMBOLS, blank=".").dims(n, n))
        i.check(box_width * box_height == n, "boxes must tile the grid")
        i.check(ib.max(cells) <= n, "a value is larger than the size of the grid")
    return Sudoku(b.build(), n, box_width, box_height, cells)


def main() -> None:
    puzzle = sudoku()
    data = puzzle.spec.read("formats/sudoku/cases/four-by-four/input.txt")
    n, width, height = data[puzzle.n], data[puzzle.box_width], data[puzzle.box_height]
    print(f"{n}x{n} grid with {width}x{height} boxes")
    for row in data[puzzle.cells]:
        print(row)


if __name__ == "__main__":
    main()
