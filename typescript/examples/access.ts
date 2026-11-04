import { readFileSync } from "node:fs";
import { Spec } from "gecode-iris";

const spec = Spec.parse(readFileSync("formats/dimacs-cnf/dimacs-cnf.iris"));
const data = spec.read(readFileSync("formats/dimacs-cnf/cases/three-variables/input.txt"));

for (const entry of data.entries) {
  console.log(entry);
}
console.log(data.has("clause"), data.has("model"));
console.log(data.size("clause"), data.size("clause", [1]));
console.log(data.ints("clause", [1]));
console.log(data.value("clause"));
console.log(data.toObject());
