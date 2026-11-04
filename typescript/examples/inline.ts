import { Spec } from "gecode-iris/inline";

const spec = Spec.parse("n:int row[n]:int", "example.iris");
console.log(spec.read("3 10 20 30", "input.txt").ints("row"));
