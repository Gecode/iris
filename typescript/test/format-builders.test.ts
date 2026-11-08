// The hand-written builders in examples/, one per format in formats/.
//
// The Rust conformance runner judges every case of a format through
// test/format-builder-driver.ts, which answers with the spec that the
// format's builder builds instead of the case's own. So each case is judged
// as rust/gecode-iris/tests/format_builders.rs judges it: the same case
// loading, parameters, operation, and comparison of data or error. The
// runner comes from the Rust workspace through Cargo.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readdirSync } from "node:fs";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../..", import.meta.url));
const driver = fileURLToPath(new URL("format-builder-driver.ts", import.meta.url));
const builders = ["dimacs-cnf", "jobshop", "nonogram", "sudoku", "tsplib", "tsptw"];

test("every format has a builder", () => {
  const formats = readdirSync(`${root}formats`, { withFileTypes: true });
  assert.deepEqual(
    formats.filter((entry) => entry.isDirectory()).map((entry) => entry.name),
    builders,
  );
  const examples = readdirSync(`${root}typescript/examples`);
  assert.deepEqual(
    examples.filter((file) => /^build-.*\.ts$/.test(file)),
    builders.map((format) => `build-${format}.ts`),
  );
});

/** `word` in single quotes, which the runner's command line splits on. */
function quoted(word: string): string {
  return `'${word.replaceAll("'", "'\\''")}'`;
}

for (const format of builders) {
  test(`every ${format} case reads through its builder`, (t) => {
    const command = [process.execPath, driver, format].map(quoted).join(" ");
    const runner = ["run", "--quiet", "--locked", "-p", "gecode-iris-conformance", "--"];
    const result = spawnSync(
      "cargo",
      [...runner, "run", "--driver", command, "--version", "0.1", `${root}formats/${format}`],
      { cwd: `${root}rust`, encoding: "utf8" },
    );
    const report = result.stdout + result.stderr;
    const summary = /^Summary: total=(\d+) pass=(\d+) /m.exec(result.stdout);
    assert.ok(summary !== null, `no summary in the report:\n${report}`);
    const [total, passed] = [Number(summary[1]), Number(summary[2])];
    assert.ok(total >= 2, `${format} has too few cases:\n${report}`);
    assert.equal(passed, total, report);
    assert.equal(result.status, 0, report);
    t.diagnostic(`${passed} of ${total} cases pass`);
  });
}
