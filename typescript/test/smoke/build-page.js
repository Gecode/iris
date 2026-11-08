// Builds a reader spec at module scope in a browser page. The page's import
// map resolves gecode-iris/build to the builder entry point under test.
import { int, IrisError, spec } from "gecode-iris/build";

const built = spec((b) => {
  const n = b.read("n", int);
  b.check(n.gt(0), "n is positive");
  return n;
});
let site = null;
try {
  built.read("0");
} catch (error) {
  if (error instanceof IrisError) {
    site = error.callSite;
  }
}
document.body.dataset.result = JSON.stringify({ n: built.read("4"), site });
