// Runs the builder programs of the TypeScript round trip, which
// rust/gecode-iris/tests/builder_round_trip_typescript.rs generates:
//
//   node test/round-trip-runner.ts SOURCES OUT
//
// Each program is a module SOURCES/NAME.ts whose `builder()` returns a
// `SpecBuilder`. The runner builds it, which runs the builder's checks, and
// writes the source that it prints to OUT/NAME.iris, or why it built none to
// OUT/NAME.error.
import { mkdirSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import type { SpecBuilder } from "gecode-iris/build";

interface Program {
  builder(): SpecBuilder;
}

function isProgram(module: unknown): module is Program {
  return (
    typeof module === "object" &&
    module !== null &&
    "builder" in module &&
    typeof module.builder === "function"
  );
}

/** The source that the program at `path` prints, once its builder builds. */
async function source(path: string): Promise<string> {
  const program: unknown = await import(pathToFileURL(path).href);
  if (!isProgram(program)) {
    throw new Error(`${path} does not export a function builder`);
  }
  const builder = program.builder();
  builder.build();
  return builder.source();
}

/** Write what the program `name` prints, or why it printed nothing. */
async function run(sources: string, out: string, name: string): Promise<void> {
  try {
    writeFileSync(join(out, `${name}.iris`), await source(join(sources, `${name}.ts`)));
  } catch (error) {
    writeFileSync(join(out, `${name}.error`), String(error));
  }
}

async function main(args: readonly string[]): Promise<void> {
  const [sources, out] = args;
  if (sources === undefined || out === undefined || args.length !== 2) {
    process.stderr.write("usage: node test/round-trip-runner.ts SOURCES OUT\n");
    process.exitCode = 2;
    return;
  }
  mkdirSync(out, { recursive: true });
  const names = readdirSync(sources)
    .filter((file) => file.endsWith(".ts"))
    .map((file) => file.slice(0, -".ts".length))
    .toSorted();
  await Promise.all(names.map((name) => run(sources, out, name)));
}

await main(process.argv.slice(2));
