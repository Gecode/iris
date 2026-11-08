// Reads the format at module scope in workerd. Wrangler bundles the package
// with the workerd condition and imports the files as text modules. The
// `/build` path answers with a reader spec built through gecode-iris/build
// and the call site that its failed check names.
import { Spec } from "gecode-iris";
import { int, IrisError, spec as buildSpec, type CallSite } from "gecode-iris/build";
import specText from "../../../../formats/sudoku/sudoku.iris";
import input from "../../../../formats/sudoku/cases/four-by-four/input.txt";

const spec = Spec.parse(specText, "sudoku.iris");
const json = spec.read(input, "input.txt").toJSON();

const built = buildSpec((b) => {
  const n = b.read("n", int);
  b.check(n.gt(0), "n is positive");
  return n;
});
let site: CallSite | null = null;
try {
  built.read("0");
} catch (error) {
  if (error instanceof IrisError) {
    site = error.callSite;
  }
}
const builtJson = JSON.stringify({ n: built.read("4"), site });

export default {
  fetch(request: Request): Response {
    const body = new URL(request.url).pathname === "/build" ? builtJson : json;
    return new Response(body, { headers: { "Content-Type": "application/json" } });
  },
};
