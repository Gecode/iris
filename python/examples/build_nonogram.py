from __future__ import annotations

from typing import NamedTuple

import gecode_iris as iris
import gecode_iris.build as ib


class Nonogram(NamedTuple):
    """A nonogram reader: the board size and the clues of each row and column."""

    spec: iris.Spec
    width: ib.IntField[int]
    height: ib.IntField[int]
    row: ib.IntField[list[list[int]]]
    col: ib.IntField[list[list[int]]]


class Clues(NamedTuple):
    """The clue lines of one side of the board."""

    lines: ib.Record
    clues: ib.IntField[list[list[int]]]


@ib.track_caller
def clue_lines(s: ib.Scope0[ib.Required], count: ib.IntField[int], name: str) -> Clues:
    """Reads ``count`` lines of comma-separated clues into the entry ``name``."""
    with s.repeat(count) as line:
        clues = line.list(name, ib.at_least(0).until(ib.eol))
    return Clues(line.record, clues)


def zero_stands_alone(clues: ib.IntField[list[list[int]]]) -> ib.IntExpr:
    """A 0 clue says the line is empty, so it must stand alone."""
    return (ib.count(clues) <= 1) | (ib.min(clues) > 0)


def clues_fit(clues: ib.IntField[list[list[int]]], length: ib.IntField[int]) -> ib.IntExpr:
    """The clues and one gap between each pair of them fit into ``length``."""
    return ib.sum(clues) + ib.count(clues) - 1 <= length


def nonogram() -> Nonogram:
    b = ib.SpecBuilder("nonogram")
    b.separators(",")
    width = b.capture("width", ib.at_least(1))
    height = b.capture("height", ib.at_least(1))
    with b.sections(skip_other=True) as s:
        s.arm(("width ", width))
        s.arm(("height ", height))
        with s.arm("rows") as a:
            rows = clue_lines(a, height, "row")
        with s.arm("columns") as a:
            cols = clue_lines(a, width, "col")
    row, col = rows.clues, cols.clues

    b.check(height <= ib.div(1_000_000, width), "the board must have at most a million fields")
    b.check(
        (ib.size(rows.lines) == height) & (ib.size(cols.lines) == width),
        "the numbers of rows and columns differ from the board size",
    )
    b.check(zero_stands_alone(row), "a 0 must be the only clue of its row")
    b.check(zero_stands_alone(col), "a 0 must be the only clue of its column")
    b.check(clues_fit(row, width), "the clues of a row do not fit into the width of the board")
    b.check(clues_fit(col, height), "the clues of a column do not fit into the height of the board")
    return Nonogram(b.build(), width, height, row, col)


def main() -> None:
    puzzle = nonogram()
    data = puzzle.spec.read("formats/nonogram/cases/five-by-five/input.txt")
    print(f"{data[puzzle.width]}x{data[puzzle.height]} board")
    print(f"rows: {data[puzzle.row]}")
    print(f"columns: {data[puzzle.col]}")


if __name__ == "__main__":
    main()
