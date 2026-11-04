// The examples of the site's TypeScript guide, type-checked and run against the
// built package. The site renders each file in examples/ with its recorded
// output, so these tests keep the guide true. Examples run from the repository
// root, since they read files under formats/.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../..", import.meta.url));
const directory = fileURLToPath(new URL("../examples/", import.meta.url));

/** Examples that need another runtime, or that another example runs. */
const notRun = new Map([
  ["browser.ts", "a page loads it"],
  ["worker.ts", "workerd calls its fetch handler"],
  ["deadline-worker.ts", "deadline.ts runs it in a worker"],
]);

const files = readdirSync(directory);
const sources = files.filter((file) => /\.(ts|cjs)$/.test(file)).toSorted();
const runnable = sources.filter((source) => !notRun.has(source));

function recording(source: string): string {
  return source.replace(/\.\w+$/, ".out");
}

test("exactly the runnable examples have a recorded output", () => {
  assert.deepEqual(
    files.filter((file) => file.endsWith(".out")).toSorted(),
    runnable.map(recording).toSorted(),
  );
  for (const source of notRun.keys()) {
    assert.ok(sources.includes(source), `examples/${source} is missing`);
  }
});

test("every example type-checks against the built package", () => {
  const tsc = fileURLToPath(new URL("../node_modules/typescript/bin/tsc", import.meta.url));
  const result = spawnSync(process.execPath, [tsc, "-p", `${directory}tsconfig.json`], {
    encoding: "utf8",
  });
  assert.equal(result.status, 0, result.stdout + result.stderr);
});

for (const source of runnable) {
  test(`examples/${source} prints its recorded output`, () => {
    const result = spawnSync(process.execPath, [`${directory}${source}`], {
      cwd: root,
      encoding: "utf8",
    });
    assert.deepEqual({ status: result.status, stderr: result.stderr }, { status: 0, stderr: "" });
    assert.equal(result.stdout, readFileSync(`${directory}${recording(source)}`, "utf8"));
  });
}
