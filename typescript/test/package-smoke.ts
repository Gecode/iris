// Packed-package smoke test: packs the package with npm pack, installs the
// tarball into an isolated consumer, type-checks an import of every entry
// point with strict settings and skipLibCheck off, and imports each in Node.
// It catches declarations or files that the repository has but the tarball
// lacks.
//
//   npm run test:package
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, test } from "node:test";
import { fileURLToPath } from "node:url";

const packageDirectory = fileURLToPath(new URL("..", import.meta.url));
const tsc = join(packageDirectory, "node_modules/typescript/bin/tsc");
const ENTRY_POINTS = ["gecode-iris", "gecode-iris/inline", "gecode-iris/manual"];

const temporary = mkdtempSync(join(tmpdir(), "gecode-iris-package-"));
const consumer = join(temporary, "consumer");

before(() => {
  const packed: unknown = JSON.parse(
    run("npm", ["pack", "--json", "--pack-destination", temporary], packageDirectory),
  );
  const tarball = packedFilename(packed);
  mkdirSync(consumer);
  writeJson("package.json", { name: "consumer", private: true, type: "module" });
  writeJson("tsconfig.json", {
    compilerOptions: {
      strict: true,
      module: "nodenext",
      moduleResolution: "nodenext",
      target: "es2023",
      lib: ["es2023", "esnext.disposable", "dom"],
      types: [],
      noEmit: true,
      skipLibCheck: false,
    },
    include: ["*.ts"],
  });
  run(
    "npm",
    ["install", "--no-audit", "--no-fund", "--ignore-scripts", join(temporary, tarball)],
    consumer,
  );
});

after(() => rmSync(temporary, { recursive: true, force: true }));

test("every entry point type-checks in a strict consumer of the packed package", () => {
  const imports = ENTRY_POINTS.map(
    (entry, i) => `import { initialize as initialize${i}, Spec as Spec${i} } from "${entry}";`,
  );
  const uses = ENTRY_POINTS.map(
    (_, i) => `export const load${i}: Promise<void> = initialize${i}(new URL("file:///iris.wasm"));
export const parse${i}: (text: string) => Spec${i} = (text) => Spec${i}.parse(text);`,
  );
  writeFileSync(join(consumer, "entries.ts"), [...imports, ...uses].join("\n") + "\n");
  run(process.execPath, [tsc, "-p", join(consumer, "tsconfig.json")], consumer);
});

for (const entry of ENTRY_POINTS) {
  test(`${entry} imports in Node from the packed package`, () => {
    run(process.execPath, ["--input-type=module", "-e", `await import("${entry}");`], consumer);
  });
}

function packedFilename(packed: unknown): string {
  const filename: unknown = Array.isArray(packed)
    ? Reflect.get(Object(packed[0]), "filename")
    : undefined;
  if (typeof filename !== "string") {
    throw new Error("npm pack --json did not report a filename");
  }
  return filename;
}

function writeJson(name: string, value: unknown): void {
  writeFileSync(join(consumer, name), `${JSON.stringify(value, null, 2)}\n`);
}

function run(command: string, args: readonly string[], cwd: string): string {
  const result = spawnSync(command, args, { cwd, encoding: "utf8" });
  assert.equal(
    result.status,
    0,
    `${command} ${args.join(" ")} failed:\n${result.error?.message ?? ""}${result.stdout}${result.stderr}`,
  );
  return result.stdout;
}
