import { Spec } from "gecode-iris";

const spec = Spec.parse("n:int row[n]:int", "example.iris");
const data = spec.read("3 10 20 30", "input.txt");

console.log(data.int("n"));
console.log(data.ints("row"));
console.log(data.value("row"));
