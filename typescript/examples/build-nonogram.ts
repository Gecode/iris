import { readFileSync } from "node:fs";
import {
  atLeast,
  count,
  div,
  eol,
  type Field,
  min,
  p,
  type Scope,
  size,
  spec,
  sum,
  trackCaller,
} from "gecode-iris/build";

/**
 * Reads `n` lines of comma-separated clues into the entry `name`, and
 * returns the record of each line with the clues. `trackCaller` makes its
 * statements name the helper's caller.
 */
const clueLines = trackCaller((s: Scope, n: Field<"int", 0>, name: string) =>
  s.repeat(n, (line) => ({ lines: line.record, clues: line.list(name, atLeast(0).until(eol)) })),
);

/** A 0 clue says the line is empty, so it must stand alone. */
function zeroStandsAlone(clues: Field<"int", 2>) {
  return count(clues).le(1).or(min(clues).gt(0));
}

/** The clues and one gap between each pair of them fit into `length`. */
function cluesFit(clues: Field<"int", 2>, length: Field<"int", 0>) {
  return sum(clues).plus(count(clues)).minus(1).le(length);
}

/** Reads a board size and the clues of each row and column. */
export function nonogram() {
  return spec("nonogram", (b) => {
    b.separators(",");
    const width = b.capture("width", atLeast(1));
    const height = b.capture("height", atLeast(1));
    const { rows, cols } = b.sections({ skipOther: true }, (s) => {
      s.arm(p`width ${width}`);
      s.arm(p`height ${height}`);
      return {
        rows: s.arm("rows", (a) => clueLines(a, height, "row")),
        cols: s.arm("columns", (a) => clueLines(a, width, "col")),
      };
    });
    const [row, col] = [rows.clues, cols.clues];

    b.check(height.le(div(1_000_000, width)), "the board must have at most a million fields");
    b.check(
      size(rows.lines).eq(height).and(size(cols.lines).eq(width)),
      "the numbers of rows and columns differ from the board size",
    );
    b.check(zeroStandsAlone(row), "a 0 must be the only clue of its row");
    b.check(zeroStandsAlone(col), "a 0 must be the only clue of its column");
    b.check(cluesFit(row, width), "the clues of a row do not fit into the width of the board");
    b.check(cluesFit(col, height), "the clues of a column do not fit into the height of the board");
    return { width, height, row, col };
  });
}

if (import.meta.main) {
  const data = nonogram().read(readFileSync("formats/nonogram/cases/five-by-five/input.txt"));
  console.log(`${data.width}x${data.height} board`);
  console.log(`rows: ${JSON.stringify(data.row)}`);
  console.log(`columns: ${JSON.stringify(data.col)}`);
}
