# Sudoku

[sudoku.iris](sudoku.iris) reads a `# size=N box=WxH` header followed by
an N×N grid. The alphabet `123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ0` maps to
values 1–36; `.` maps to 0 for blank cells. Data contains `n`,
`box_width`, `box_height`, and `cells`. Checks require the box area to
equal N and every given value to be at most N; solving the puzzle and
checking its uniqueness belong to the host.

The format source is the
[scaled sudoku corpus README](https://github.com/zayenz/scaled-sudoku-instances#puzzle-example).
The reader spec is copied verbatim from
[Gecode's sudoku-instances.hh](https://github.com/Gecode/gecode/blob/599f977b7855dba2a1a5f67db1892722372067d3/examples/sudoku-instances.hh#L1890).

- [four-by-four](cases/four-by-four/) uses a 4×4 grid with 2×2 boxes
  and blank cells.
- [bad-box-size](cases/bad-box-size/) changes the box dimensions to
  3×2. The reference reports `E-CHECK-FAILED` with an empty record path.

The `instances` statement permits several puzzles in one file; these
samples each contain one. These are hand-made samples under the
repository's MIT license. See the [library README](../README.md) for case
layout, validation, and Gecode notices.
