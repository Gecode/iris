// The format that every smoke test reads: formats/sudoku and its four-by-four case.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const formatDirectory = new URL("../../../formats/sudoku/", import.meta.url);
const caseDirectory = new URL("cases/four-by-four/", formatDirectory);

export const specPath: string = fileURLToPath(new URL("sudoku.iris", formatDirectory));
export const inputPath: string = fileURLToPath(new URL("input.txt", caseDirectory));
/** The case's canonical JSON document, parsed for comparison. */
export const expected: unknown = JSON.parse(
  readFileSync(new URL("expected.json", caseDirectory), "utf8"),
);
