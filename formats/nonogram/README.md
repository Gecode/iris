# Nonogram

[nonogram.iris](nonogram.iris) reads the `.non` subset exported by
webpbn: `width`, `height`, `rows`, and `columns`, with comma-separated
clues on one line per row or column. Data contains the dimensions and
ragged `row` and `col` arrays. A single `0` denotes an empty row or
column and remains `[0]` in the data. Other keyword lines are skipped.

The format source is Steven Simpson's
[puzzle-file format](https://stevocity.me.uk/nonogram/fmt2), also available
through [webpbn's export tool](https://webpbn.com/export.cgi). The reader
spec is copied from
[Gecode's nonogram.cpp](https://github.com/Gecode/gecode/blob/5e7bbd5c26b0702c6fd652356b2289da0710ac3c/examples/nonogram.cpp#L61).
It requires explicit `width` and `height` lines rather than the full
format's dimension aliases `columns N` and `rows N`.

- [five-by-five](cases/five-by-five/) describes a 5×5 X.
- [row-too-wide](cases/row-too-wide/) gives its first row a clue of 6.
  The reference reports `E-CHECK-FAILED` with record path `[0]`.

These are hand-made samples under the repository's MIT license. See the
[library README](../README.md) for case layout, validation, and Gecode notices.
