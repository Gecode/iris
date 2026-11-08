// Browser smoke test: serves the package and loads the browser and inline
// entry points, and the builder's, in Chromium under a strict
// Content-Security-Policy.
//
//   npm run test:browser
//
// Set BROWSER_CHANNEL=chrome to use an installed Chrome instead of
// Playwright's Chromium.
import assert from "node:assert/strict";
import { randomBytes } from "node:crypto";
import { readFileSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { after, before, test } from "node:test";
import { fileURLToPath } from "node:url";
import { chromium, type Browser } from "playwright";
import { expected, inputPath, specPath } from "./smoke/sudoku.ts";

const packageDirectory = fileURLToPath(new URL("..", import.meta.url));
const SERVED_DIRECTORIES = ["/dist/", "/wasm/", "/test/smoke/"];
const MEDIA_TYPES: Readonly<Record<string, string>> = {
  js: "text/javascript",
  wasm: "application/wasm",
};

interface Page {
  /** The package path that the import map gives for `specifier`. */
  readonly entry: string;
  readonly allowWasm: boolean;
  /** The import that the page's script resolves to `entry`. */
  readonly specifier: string;
  readonly script: string;
}

const READ_PAGE = { specifier: "gecode-iris", script: "/test/smoke/page.js" };
const BUILD_PAGE = { specifier: "gecode-iris/build", script: "/test/smoke/build-page.js" };

const PAGES: Readonly<Record<string, Page>> = {
  "/browser.html": { ...READ_PAGE, entry: exportTarget(".", "browser"), allowWasm: true },
  "/inline.html": { ...READ_PAGE, entry: exportTarget("./inline", "default"), allowWasm: true },
  "/without-wasm-unsafe-eval.html": {
    ...READ_PAGE,
    entry: exportTarget(".", "browser"),
    allowWasm: false,
  },
  "/build.html": { ...BUILD_PAGE, entry: exportTarget("./build", "browser"), allowWasm: true },
  "/build-inline.html": {
    ...BUILD_PAGE,
    entry: exportTarget("./build/inline", "default"),
    allowWasm: true,
  },
};

const server = createServer((request, response) => {
  serve(request, response).catch(() => response.writeHead(404).end());
});
let browser: Browser;
let origin: string;

before(async () => {
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  if (address === null || typeof address === "string") {
    throw new Error("the server has no TCP address");
  }
  origin = `http://127.0.0.1:${address.port}`;
  const channel = process.env["BROWSER_CHANNEL"];
  browser = await chromium.launch(channel === undefined ? {} : { channel });
});

after(async () => {
  await browser?.close();
  server.close();
});

test("the browser entry parses at module scope after top-level await", async () => {
  assert.deepEqual(JSON.parse(await resultOf("/browser.html")), expected);
});

test("the inline entry parses at module scope in a page", async () => {
  assert.deepEqual(JSON.parse(await resultOf("/inline.html")), expected);
});

for (const path of ["/build.html", "/build-inline.html"]) {
  test(`${path} builds a reader spec whose check names the page's line`, async () => {
    const result: unknown = JSON.parse(await resultOf(path));
    assert.equal(Reflect.get(Object(result), "n"), 4);
    const site: unknown = Reflect.get(Object(result), "site");
    assert.equal(Reflect.get(Object(site), "file"), `${origin}/test/smoke/build-page.js`);
    assert.equal(Reflect.get(Object(site), "line"), 7);
  });
}

test("a policy without 'wasm-unsafe-eval' blocks the module", async () => {
  await assert.rejects(resultOf("/without-wasm-unsafe-eval.html"), /Content Security Policy/);
});

/** Opens a page and returns the JSON it reads, or rejects with its first error. */
async function resultOf(path: string): Promise<string> {
  const page = await browser.newPage();
  try {
    const failure = new Promise<never>((_, reject) => page.on("pageerror", reject));
    await page.goto(origin + path);
    const result = page
      .waitForFunction(() => document.body.dataset["result"])
      .then((handle) => handle.jsonValue());
    const json: unknown = await Promise.race([result, failure]);
    assert.equal(typeof json, "string");
    return String(json);
  } finally {
    await page.close();
  }
}

async function serve(request: IncomingMessage, response: ServerResponse): Promise<void> {
  const path = new URL(request.url ?? "/", "http://localhost").pathname;
  const page = PAGES[path];
  if (page !== undefined) {
    const nonce = randomBytes(16).toString("base64");
    const scripts = `'self' 'nonce-${nonce}'${page.allowWasm ? " 'wasm-unsafe-eval'" : ""}`;
    response.writeHead(200, {
      "Content-Type": "text/html; charset=utf-8",
      "Content-Security-Policy": `default-src 'self'; script-src ${scripts}`,
    });
    response.end(pageHtml(page, nonce));
  } else if (path === "/test/smoke/sudoku.json") {
    response.writeHead(200, { "Content-Type": "application/json" });
    response.end(
      JSON.stringify({
        spec: await readFile(specPath, "utf8"),
        input: await readFile(inputPath, "utf8"),
      }),
    );
  } else if (SERVED_DIRECTORIES.some((directory) => path.startsWith(directory))) {
    const file = new URL(`.${path}`, `file://${packageDirectory}`);
    const mediaType = MEDIA_TYPES[path.slice(path.lastIndexOf(".") + 1)];
    if (mediaType === undefined || path.includes("..")) {
      throw new Error(`not served: ${path}`);
    }
    response.writeHead(200, { "Content-Type": mediaType });
    response.end(await readFile(file));
  } else {
    throw new Error(`not found: ${path}`);
  }
}

function pageHtml(page: Page, nonce: string): string {
  const importMap = JSON.stringify({ imports: { [page.specifier]: page.entry.slice(1) } });
  return `<!doctype html>
<meta charset="utf-8">
<title>gecode-iris smoke test</title>
<script type="importmap" nonce="${nonce}">${importMap}</script>
<script type="module" src="${page.script}"></script>
`;
}

/** Resolves a package export as a bundler would, for one condition. */
function exportTarget(subpath: string, condition: string): string {
  const manifest: unknown = JSON.parse(
    readFileSync(new URL("../package.json", import.meta.url), "utf8"),
  );
  const conditions: unknown = Reflect.get(Reflect.get(Object(manifest), "exports"), subpath);
  const target: unknown = Reflect.get(Object(conditions), condition);
  if (typeof target !== "string") {
    throw new Error(`package.json exports no ${condition} target for ${subpath}`);
  }
  return target;
}
