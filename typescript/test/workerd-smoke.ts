// Workers smoke test: serves test/smoke/workerd in workerd with `wrangler dev`
// and fetches what the worker read at module scope, through the workerd entry
// and the builder's workerd entry.
//
//   npm run test:workerd
import assert from "node:assert/strict";
import { spawn, type ChildProcess } from "node:child_process";
import { once } from "node:events";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { expected } from "./smoke/sudoku.ts";

const wrangler = fileURLToPath(
  new URL("../node_modules/wrangler/bin/wrangler.js", import.meta.url),
);
const config = fileURLToPath(new URL("smoke/workerd/wrangler.jsonc", import.meta.url));

// A build error leaves the server never ready, so the test needs a deadline.
test(
  "the workerd entries parse and build at module scope in workerd",
  { timeout: 60_000 },
  async () => {
    const server = spawn(
      process.execPath,
      [wrangler, "dev", "--config", config, "--ip", "127.0.0.1", "--port", "0"],
      {
        stdio: ["ignore", "pipe", "inherit"],
        env: { ...process.env, WRANGLER_SEND_METRICS: "false" },
      },
    );
    try {
      const url = await readyUrl(server);
      const response = await fetch(url);
      assert.equal(response.status, 200);
      assert.deepEqual(await response.json(), expected);
      const built = await fetch(new URL("/build", url));
      assert.equal(built.status, 200);
      const answer: unknown = await built.json();
      assert.equal(Reflect.get(Object(answer), "n"), 4, "the built reader spec reads in workerd");
      const site: unknown = Reflect.get(Object(answer), "site");
      assert.ok(site !== null && typeof site === "object", "the failed check has a call site");
      assert.equal(typeof Reflect.get(site, "line"), "number");
    } finally {
      server.kill();
      await once(server, "exit");
    }
  },
);

/** Echoes the server's output and returns the URL it reports when ready. */
async function readyUrl(server: ChildProcess): Promise<string> {
  let output = "";
  for await (const chunk of server.stdout ?? []) {
    process.stdout.write(chunk);
    output += String(chunk);
    const url = /Ready on (http:\/\/\S+)/.exec(output)?.[1];
    if (url !== undefined) {
      return url;
    }
  }
  throw new Error("wrangler dev exited before it was ready");
}
