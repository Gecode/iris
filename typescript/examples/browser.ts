import { IrisError, Spec } from "gecode-iris";

const spec = Spec.parse("n:int row[n]:int", "example.iris");
const input = document.querySelector("textarea");
const output = document.querySelector("output");
if (input === null || output === null) throw new Error("the page needs a textarea and an output");

input.addEventListener("input", () => {
  try {
    output.textContent = spec.read(input.value, "textarea").toJSON();
  } catch (error) {
    if (!(error instanceof IrisError)) throw error;
    output.textContent = error.diagnostic;
  }
});
