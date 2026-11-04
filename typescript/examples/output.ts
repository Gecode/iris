import { readFileSync } from "node:fs";
import { Spec } from "gecode-iris";

const spec = Spec.parse(readFileSync("formats/dimacs-cnf/dimacs-cnf.iris"));
const data = spec.read(readFileSync("formats/dimacs-cnf/cases/three-variables/input.txt"));

process.stdout.write(data.toDzn());
process.stdout.write(data.toJSON());
