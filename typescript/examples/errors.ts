import { readFileSync } from "node:fs";
import { IrisError, Spec } from "gecode-iris";

const specPath = "formats/dimacs-cnf/dimacs-cnf.iris";
const inputPath = "formats/dimacs-cnf/cases/literal-out-of-range/input.txt";
const spec = Spec.parse(readFileSync(specPath), specPath);

try {
  spec.read(readFileSync(inputPath), inputPath);
} catch (error) {
  if (!(error instanceof IrisError)) throw error;
  console.log(error.code, error.message);
  console.log(error.inputLocation);
  console.log(error.specLocation);
  console.log(error.context, error.recordPath);
  console.log(error.diagnostic);
}
