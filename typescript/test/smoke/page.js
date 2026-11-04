// Reads the format at module scope in a browser page. The page's import map
// resolves gecode-iris to the entry point under test.
import { Spec } from "gecode-iris";
import sudoku from "./sudoku.json" with { type: "json" };

const spec = Spec.parse(sudoku.spec, "sudoku.iris");
document.body.dataset.result = spec.read(sudoku.input, "input.txt").toJSON();
