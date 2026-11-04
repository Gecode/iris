// Reads the format at module scope in workerd. Wrangler bundles the package
// with the workerd condition and imports the files as text modules.
import { Spec } from "gecode-iris";
import specText from "../../../../formats/sudoku/sudoku.iris";
import input from "../../../../formats/sudoku/cases/four-by-four/input.txt";

const spec = Spec.parse(specText, "sudoku.iris");
const json = spec.read(input, "input.txt").toJSON();

export default {
  fetch(): Response {
    return new Response(json, { headers: { "Content-Type": "application/json" } });
  },
};
