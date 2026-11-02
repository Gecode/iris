import { readFile, readdir, mkdtemp, writeFile, rm } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { tmpdir } from 'node:os';
import { pathToFileURL } from 'node:url';
import { spawnSync } from 'node:child_process';
import { gzipSync } from 'node:zlib';
import binaryen from 'binaryen';
import { strictEqual, deepStrictEqual, ok, rejects } from 'node:assert';
import { buildExampleTools, collectFormatExamples, siteDirectory, repository } from './format-examples.mjs';
import { encodeState, decodeState, validateState } from '../src/playground/share.ts';

const tools = buildExampleTools();
const { formats } = await collectFormatExamples(tools);
const directory = resolve(siteDirectory, 'dist/_astro');
const files = await readdir(directory);
const wasmFiles = files.filter(name => name.endsWith('.wasm'));
strictEqual(wasmFiles.length, 1, 'Build must emit exactly one WASM asset');
const bytes = await readFile(resolve(directory, wasmFiles[0]));
ok(bytes.equals(await readFile(resolve(siteDirectory, 'src/generated/wasm/iris_wasm_bg.wasm'))), 'Emitted WASM differs from the optimized generated binary');
const adapter = await readFile(resolve(siteDirectory, 'src/generated/playground-wasm.ts'), 'utf8');
ok(adapter.includes('module_or_path: wasmUrl'), 'Documentation-only fallback is not a complete playground build');
const rawLimit = Number(process.env.IRIS_WASM_RAW_LIMIT ?? 1048576);
const gzipLimit = Number(process.env.IRIS_WASM_GZIP_LIMIT ?? 393216);
const gzip = gzipSync(bytes, { level: 9 }).length;
const module = binaryen.readBinary(bytes);
try {
  const memory = module.getMemoryInfo();
  strictEqual(memory.max, 4096, 'Browser memory must be capped at 256 MiB');
  strictEqual(memory.shared, false, 'Browser memory must not require shared memory');
} finally { module.dispose(); }
ok(bytes.length <= rawLimit && gzip <= gzipLimit, `WASM size budget exceeded: ${bytes.length}/${rawLimit} raw, ${gzip}/${gzipLimit} gzip`);
const wasm = await import(pathToFileURL(resolve(siteDirectory, 'src/generated/wasm/iris_wasm.js')));
wasm.initSync({ module: bytes });
let successes = 0, failures = 0;
for (const format of formats) {
  for (const example of format.cases) {
    const request = { language: '0.1', spec: format.spec.text, input: example.input.text, specOrigin: format.spec.path, inputOrigin: example.input.path, params: example.params };
    const actual = JSON.parse(wasm.read(JSON.stringify(request)));
    const label = `${format.id}/${example.id}`;
    if (example.result.kind === 'success') {
      strictEqual(actual.kind, 'success', label);
      for (const field of ['json', 'dzn', 'summary']) strictEqual(actual[field], example.result[field], `${label}/${field}`);
      successes++;
    } else {
      strictEqual(actual.kind, 'language', label);
      strictEqual(actual.errorJson, example.result.json, `${label}/errorJson`);
      strictEqual(actual.diagnostic, example.result.diagnostic, `${label}/diagnostic`);
      strictEqual(actual.code, JSON.parse(example.result.json).code, `${label}/code`);
      ok(!('json' in actual), label); failures++;
    }
  }
}
ok(formats.length && successes && failures, 'No format cases selected');

const temporary = await mkdtemp(join(tmpdir(), 'iris-wasm-parity-'));
try {
  const probes = [
    { spec: 'a: int\nb: int\nc: int\n', input: '9007199254740993 9223372036854775807 -9223372036854775808' },
    { spec: 'a: float\nb: float\n', input: '-0.0 0.000000000000001' },
    { spec: 'title: rest\nrepeat 3 { list x: int until 0 }\n', input: '🌸 blå <script>\n0 1 0 2 3 0' },
    { spec: '# 🌸\n\tx: missing\ny:\n', input: '' },
  ];
  for (const [index, probe] of probes.entries()) {
    const specOrigin = join(temporary, `probe-${index}.iris`), inputOrigin = join(temporary, `input-${index}.txt`);
    await writeFile(specOrigin, probe.spec); await writeFile(inputOrigin, probe.input);
    const actual = JSON.parse(wasm.read(JSON.stringify({ language: '0.1', ...probe, specOrigin, inputOrigin, params: [] })));
    const checked = JSON.parse(wasm.check(JSON.stringify({ language: '0.1', spec: probe.spec, specOrigin })));
    const cliCheck = run(tools.iris, ['check', specOrigin]);
    strictEqual(checked.diagnostic, cliCheck.status === 0 ? cliCheck.stdout : cliCheck.stderr, `probe ${index}/check`);
    if (actual.kind === 'success') {
      for (const field of ['json', 'dzn', 'summary']) strictEqual(actual[field], run(tools.iris, ['read', specOrigin, inputOrigin, `--${field}`]).stdout, `probe ${index}/${field}`);
    } else {
      strictEqual(actual.errorJson, run(tools.driver, ['read', specOrigin, inputOrigin]).stdout);
      strictEqual(actual.diagnostic, run(tools.iris, ['read', specOrigin, inputOrigin]).stderr);
    }
  }
  for (const spec of ['x:int', 'x:', 'x:missing', `let total=${Array(32).fill('12345').join('+')}`]) {
    const formatted = JSON.parse(wasm.format(JSON.stringify({ language: '0.1', spec, specOrigin: '<stdin>' })));
    const native = run(tools.iris, ['fmt', '-'], spec);
    strictEqual(formatted.kind === 'formatted' ? formatted.spec : formatted.diagnostic, native.status === 0 ? native.stdout : native.stderr, 'formatter parity');
  }
} finally { await rm(temporary, { recursive: true, force: true }); }

const state = validateState({ language: '0.1', spec: '# 🌸\nx: int', input: '9007199254740993', specOrigin: 'spec.iris', inputOrigin: 'input.txt', params: [{ name: 'x', value: '9007199254740993' }, { name: 'instance', value: 'blå' }], view: 'json' });
const encoded = await encodeState(state);
deepStrictEqual(await decodeState(encoded), state);
const zipped = value => '#v1=' + gzipSync(Buffer.from(value)).toString('base64url');
for (const hash of ['#v2=abc', '#v1=!', '#v1=abcd', '#v1=' + 'a'.repeat(16384), zipped('a'.repeat(262145)), zipped(JSON.stringify({ ...state, language: '1.0' })), zipped(JSON.stringify({ ...state, params: [{ name: 'x', value: 9 }] }))]) await rejects(() => decodeState(hash));
await rejects(() => encodeState({ ...state, input: 'a'.repeat(262144) }));
const workers = files.filter(name => name.startsWith('worker-') && name.endsWith('.js'));
strictEqual(workers.length, 1, 'Missing module worker');
console.log(`WASM parity: ${formats.length} formats, ${successes + failures} cases (${successes} successes, ${failures} failures), exact integer/float/Unicode/check/format probes and share codec passed.`);
console.log(`Emitted WASM: ${bytes.length} bytes raw / ${gzip} bytes gzip; worker ${(await readFile(resolve(directory, workers[0]))).length} bytes; glue ${(await readFile(resolve(siteDirectory, 'src/generated/wasm/iris_wasm.js'))).length} bytes.`);
function run(executable, args, input) {
  const result = spawnSync(executable, args, { cwd: repository, input, encoding: 'utf8', maxBuffer: 16 << 20 });
  if (result.error || result.status === null || result.status > 1) throw new Error(`${executable}: ${result.error ?? result.stderr}`);
  return result;
}
