import { readFileSync } from "node:fs";
import { alphabet, atLeast, max, p, spec } from "gecode-iris/build";

/** Cell values from 1 to 36, so `1` reads as 1 and `0` as 36. */
const SYMBOLS = "123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ0";

/** Reads the grid size, the box size, and the cells, with 0 for a blank. */
export function sudoku() {
  return spec("sudoku", (b) =>
    b.instances((i) => {
      const n = i.capture("n", atLeast(1));
      const boxWidth = i.capture("box_width", atLeast(1));
      const boxHeight = i.capture("box_height", atLeast(1));
      i.line(p`# size=${n} box=${boxWidth}x${boxHeight}`);
      const cells = i.read("cells", alphabet(SYMBOLS, { blank: "." }).dims(n, n));
      i.check(boxWidth.times(boxHeight).eq(n), "boxes must tile the grid");
      i.check(max(cells).le(n), "a value is larger than the size of the grid");
      return { n, boxWidth, boxHeight, cells };
    }),
  );
}

if (import.meta.main) {
  const data = sudoku().read(readFileSync("formats/sudoku/cases/four-by-four/input.txt"));
  console.log(`${data.n}x${data.n} grid with ${data.boxWidth}x${data.boxHeight} boxes`);
  for (const row of data.cells) {
    console.log(row.join(" "));
  }
}
