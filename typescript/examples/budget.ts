import { IrisError, Spec } from "gecode-iris";

const spec = Spec.parse("n:int repeat n { a[0]:int }", "empty-rows.iris");

try {
  spec.read("2000000000", "input.txt");
} catch (error) {
  if (!(error instanceof IrisError)) throw error;
  console.log(error.code, error.context);
  console.log(error.message);
}
