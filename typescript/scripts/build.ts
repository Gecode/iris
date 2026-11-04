// Builds the package: rust/iris-js to WebAssembly, its JavaScript glue, and dist/.
import { spawnSync, type SpawnSyncOptions } from "node:child_process";
import { readFileSync, renameSync, rmSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";

const packageDirectory = fileURLToPath(new URL("..", import.meta.url));
const rustDirectory = fileURLToPath(new URL("../../rust", import.meta.url));
const wasmDirectory = fileURLToPath(new URL("../wasm", import.meta.url));
const crateManifest = new URL("../../rust/iris-js/Cargo.toml", import.meta.url);

requireToolchain();
generateGlue(buildCrate());
optimize(`${wasmDirectory}/iris_js_bg.wasm`);
compileTypeScript();

function requireToolchain(): void {
  const version = /wasm-bindgen = "=([\d.]+)"/.exec(readFileSync(crateManifest, "utf8"))?.[1];
  if (version === undefined) {
    throw new Error("rust/iris-js/Cargo.toml must pin wasm-bindgen exactly");
  }
  const install = `install it with: cargo install wasm-bindgen-cli --locked --version ${version}`;
  let actual: string;
  try {
    actual = run("wasm-bindgen", ["--version"]).trim();
  } catch {
    throw new Error(`wasm-bindgen is missing; ${install}`);
  }
  if (actual !== `wasm-bindgen ${version}`) {
    throw new Error(`expected wasm-bindgen ${version}, found ${actual}; ${install}`);
  }
  const targets = run("rustup", ["target", "list", "--installed"]).split("\n");
  if (!targets.includes("wasm32-unknown-unknown")) {
    throw new Error("install the target with: rustup target add wasm32-unknown-unknown");
  }
}

/** Builds the crate with the workspace's wasm profile and returns the .wasm path. */
function buildCrate(): string {
  const messages = run(
    "cargo",
    [
      "build",
      "--locked",
      "-p",
      "iris-js",
      "--lib",
      "--target",
      "wasm32-unknown-unknown",
      "--profile",
      "wasm",
      "--message-format=json",
    ],
    { cwd: rustDirectory },
  );
  for (const line of messages.trim().split("\n")) {
    const message: unknown = JSON.parse(line);
    const artifact = wasmArtifact(message);
    if (artifact !== undefined) {
      return artifact;
    }
  }
  throw new Error("cargo did not report iris_js.wasm");
}

function wasmArtifact(message: unknown): string | undefined {
  if (
    typeof message !== "object" ||
    message === null ||
    !("reason" in message) ||
    message.reason !== "compiler-artifact" ||
    !("target" in message) ||
    typeof message.target !== "object" ||
    message.target === null ||
    !("name" in message.target) ||
    message.target.name !== "iris_js" ||
    !("filenames" in message) ||
    !Array.isArray(message.filenames)
  ) {
    return undefined;
  }
  return message.filenames.find(
    (path: unknown): path is string => typeof path === "string" && path.endsWith(".wasm"),
  );
}

/** The glue takes no default module path, so it never refers to import.meta.url. */
function generateGlue(artifact: string): void {
  rmSync(wasmDirectory, { recursive: true, force: true });
  run("wasm-bindgen", [
    artifact,
    "--target",
    "web",
    "--omit-default-module-path",
    "--out-dir",
    wasmDirectory,
    "--out-name",
    "iris_js",
  ]);
}

function optimize(binary: string): void {
  const wasmOpt = `${packageDirectory}/node_modules/binaryen/bin/wasm-opt`;
  const optimized = `${binary}.optimized`;
  run(process.execPath, [
    wasmOpt,
    binary,
    "-Oz",
    "--enable-bulk-memory",
    "--enable-nontrapping-float-to-int",
    "--enable-sign-ext",
    "-o",
    optimized,
  ]);
  const bytes = readFileSync(optimized);
  if (!WebAssembly.validate(bytes)) {
    throw new Error("the optimized WebAssembly module is not valid");
  }
  renameSync(optimized, binary);
  const gzip = gzipSync(bytes, { level: 9 }).length;
  console.log(`WebAssembly: ${bytes.length} bytes raw, ${gzip} bytes gzip`);
}

function compileTypeScript(): void {
  rmSync(`${packageDirectory}/dist`, { recursive: true, force: true });
  run(process.execPath, [
    `${packageDirectory}/node_modules/typescript/bin/tsc`,
    "-p",
    `${packageDirectory}/tsconfig.build.json`,
  ]);
}

function run(command: string, args: readonly string[], options: SpawnSyncOptions = {}): string {
  const result = spawnSync(command, args, {
    encoding: "utf8",
    maxBuffer: 64 << 20,
    stdio: ["ignore", "pipe", "inherit"],
    ...options,
  });
  const output = String(result.stdout);
  if (result.error !== undefined || result.status !== 0) {
    throw new Error(
      `${command} failed${result.error ? `: ${result.error.message}` : ""}\n${output}`,
    );
  }
  return output;
}
