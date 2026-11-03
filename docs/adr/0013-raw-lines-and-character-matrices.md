# 0013. Raw lines and character matrices

## Status

Proposed, 2026-11-03. For the Iris 1.0 design
([PLAN.md §10](../../PLAN.md#10-toward-iris-10)).

## Context

Iris 0.1 reads tokens. Blank lines are invisible, `rest` trims edge blanks
([§4.2](../../spec/0.1/04-input.md#42-white-space-and-line-blanks)), and
an alphabet skips separators and ignores line ends
([§6.10](../../spec/0.1/06-types-and-values.md#610-alphabets)). So 0.1
cannot read a line whose spaces matter, an empty line, groups separated by
blank lines, or a grid whose spaces are cells.

In a survey of the 2500 most-solved Open Kattis problems
([idea entry](../1.0-ideas.md#read-lines-and-pictures-with-their-spaces)),
about 20 problems need spaces or blank lines kept, 6 separate groups with
blank lines, and 10 are ASCII art: digits drawn in `*`, a sum drawn in
5×7 glyphs, a chessboard of `+---+` frames, a peg board whose blank
squares are spaces. Authors trim trailing spaces inconsistently, so art
rows have uneven lengths. And 45 of 553 specs could not check a row width.

Iris reads ASCII art as a character matrix and leaves the meaning of the
drawing to the program: recognizing glyphs is solving the problem, not
reading its input.

## Decision

Add one reading type for whole lines, one for lines of cells, and one
terminator for blank lines.

**`raw`** reads one physical line as a `string`: every space and tab is
kept, and only the LF and a CR before it are removed, or a final CR when
the input ends without an LF. An empty line reads as `""`. Comment
prefixes ([§4.7](../../spec/0.1/04-input.md#47-comment-and-stop-prefixes))
do not apply, so a grid of `#` walls reads under `comment "#"`. A stop
line ends the input for raw reads as for every other read.

A raw read takes a whole line, so its position matters:

1. At the start of the input, or right after an LF, it reads the line
   that starts there.
2. Otherwise, if only line blanks remain on the current line, it consumes
   them and the LF, then reads the next line.
3. Otherwise, data remains on the current line, and it reports a new
   error, `E-RAW-POSITION`.

If no line remains, it reports `E-EXPECTED-VALUE`. An input that ends with
an LF has no empty line after it. The line these rules select is *the next
raw line*.

**`picture`** reads one line as `raw` does and splits it into cells, one
per character, giving one more depth than a scalar. So `row: picture` is a
depth-1 array, and `art[h]: picture` and `list art: picture until ...` are
depth 2. Cells are one-character strings by default, so any drawing reads.
Options:

| Option | Effect |
|---|---|
| `width w` | Every row has exactly w cells after padding; otherwise a new error, `E-ROW-WIDTH`. |
| `pad "c"` | Short rows are filled with the character c: up to `width` when given, else up to the longest row the statement reads. Without `width`, `pad` is allowed only in an array or list declaration; otherwise parsing reports `E-SPEC-TYPE`. |
| `alphabet "..."`, `base`, `blank` | Cells are integers, mapped after padding as in an alphabet; other characters are `E-SYMBOL`. A space is a character like any other. |

```iris
n: 1..10
board[n]: picture width 5 pad " " alphabet "o. " base 0
list art: picture pad " " until blank
```

```input
3
oo.o
o  o.
.o.oo
 *
***

```

```data
n = 3
board = [[0, 0, 1, 0, 2], [0, 2, 2, 0, 1], [1, 0, 1, 0, 0]]
art = [[" ", "*", " "], ["*", "*", "*"]]
```

The first board row is `oo.o`, padded to 5 cells. After `n`, only the LF
remains on the first line, so the first row starts on the second. A
rectangular result, which `pad` guarantees and rows of equal length give
by themselves, renders in dzn as a matrix literal under [appendix
B.5](../../spec/0.1/appendix-b-dzn.md#b5-rectangular-arrays-and-empty-dimensions).
Rows of different lengths are ragged under
[B.6](../../spec/0.1/appendix-b-dzn.md#b6-ragged-and-deeper-arrays).

**Characters.** A cell is one Unicode scalar value, so box-drawing art
reads one cell per drawn character; a combining mark is a cell of its own.
A tab is one cell. The input is valid UTF-8 ([ADR
0011](0011-utf8-strings.md)), so the split is always defined. The same
holds for the `alphabet` and `pad` strings of a picture, and for the
symbol strings of [ADR 0014](0014-integers-beyond-64-bits.md). 0.1
alphabets match bytes, which agree with characters for ASCII; [the
alphabet
idea](../1.0-ideas.md#decide-whether-alphabets-should-read-unicode-characters)
should adopt characters for them too.

**`until blank`** ends a list or a repeat at a blank line, a line holding
only line blanks, and consumes that line. End of input also ends it, so
the last group needs no trailing blank line.

For a list of raw or picture rows, the test looks without consuming at
the next raw line. For `until blank` and `until eof`, no line ends the
list, and a blank line also ends an `until blank` list. `list lines: raw
until eof` reads every remaining line, blank and comment lines included. A
string sentinel ends the list at a line equal to it, byte for byte, and a
missing sentinel is the error that 0.1 reports at the end of input. A
numeric sentinel is `E-SPEC-TYPE`.

For `repeat until blank` and a token list `until blank`, the test runs
before each iteration or element. While data remains on the current line, it
continues. Otherwise the test looks without consuming at the next raw
line, passing comment lines: no line or a blank line ends it, and any
other line continues it. When a blank line ends it, the blank line and the
comment lines before it are consumed. Reads inside a record still skip
blank lines, as in 0.1:

```iris
repeat until eof {
  stations: 1..100
  repeat until blank {
    a: int
    b: int
    length: int
  }
}
```

`repeat until eof` and `repeat until "TOKEN"` keep their 0.1 tests, which
skip blank lines and the blanks that start a line before they test. A raw
read in the body then starts in the middle of an indented line, so raw
rows are read with `list ... until eof` or `list ... until "TOKEN"`, and
`iris check` warns about a raw or picture read in a repeat with a token
test. A token list `until eof` ends when no data remains, as `repeat until
eof` does.

**Compatibility.** `raw` and `picture` are reading types only where 0.1
would report `E-SPEC-UNKNOWN-TYPE`: a bare name that is no visible named
type and starts no range. So `type raw = word` and `x: raw..10` keep their
0.1 meaning, and these names are not reserved like base types. `repeat
until blank` is new syntax. In a list, `until eof` and `until blank` are
sentinel expressions in 0.1. They become the new terminators unless some
statement, capture, `store`, or `param` anywhere in the spec defines a
value of that name; then they keep their 0.1 meaning, and `iris check`
warns. Value lookup is dynamic in 0.1
([§8.1](../../spec/0.1/08-expressions-and-checks.md#81-values-and-value-lookup)),
so the test covers the whole spec rather than the names visible at the
list. A spec without these reads exactly as in 0.1.

## Consequences

Every ASCII-art problem in the survey reads as a matrix, and text lines,
blank-line groups, and space cells become readable. `width` adds the
row-width check that 0.1 lacks for character grids.

`raw` and `picture` are the first reads that see blank lines and comment
prefixes. A raw read right after the line of a token read sees a blank
line that the token read would skip. `E-RAW-POSITION` reports the common
mistake of a raw read after a token on the same line.

A row of only spaces is a blank line to `until blank`, so art that
contains such rows needs a row count instead.

The Specification needs the position rules for raw reads, the cell
rules, the new options, the terminator tests, the compatibility rules,
`E-RAW-POSITION`, and `E-ROW-WIDTH`.

## Alternative considered

Recognizing glyphs in Iris, as variants whose constructors are pictures
split into tiles, was drafted from the survey and dropped: it covers 10 of
2500 problems, and the program reading the matrix can recognize the
glyphs itself.

An alphabet option for line-delimited rows would extend 0.1 alphabets, but
needs every cell symbol in advance, which arbitrary art does not have.

Cells as bytes, like 0.1 alphabets, would split a box-drawing character
into three cells that are not valid strings.
