// Smoke tests of the Node entry points. Each fixture runs in its own process,
// so it loads the module itself.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { expected, inputPath, specPath } from "./smoke/sudoku.ts";

/** Runs `test/smoke/<fixture>` on the format and returns its parsed JSON output. */
function readWith(fixture: string, nodeOptions: readonly string[] = []): unknown {
  const script = fileURLToPath(new URL(`smoke/${fixture}`, import.meta.url));
  const result = spawnSync(process.execPath, [...nodeOptions, script, specPath, inputPath], {
    encoding: "utf8",
  });
  assert.equal(result.status, 0, result.stderr);
  return JSON.parse(result.stdout);
}

test("the node entry parses at module scope when imported from ESM", () => {
  assert.deepEqual(readWith("esm.ts"), expected);
});

test("the node entry parses at module scope when loaded with require()", () => {
  assert.deepEqual(readWith("require.cjs"), expected);
});

test("the inline entry parses at module scope", () => {
  assert.deepEqual(readWith("inline.ts"), expected);
});

test("the manual entry reads once initialize() has loaded the module", () => {
  assert.deepEqual(readWith("manual.ts"), expected);
});

test("initialize() resolves promised sources and may be retried after a failure", () => {
  assert.deepEqual(readWith("promised-source.ts"), expected);
});

test("a pending initialize() never replaces a module that an entry installed", () => {
  assert.deepEqual(readWith("initialize-race.ts", ["--expose-gc"]), expected);
});
