import { readFile, mkdtemp, mkdir, copyFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve, dirname } from 'node:path';
import { strictEqual, ok } from 'node:assert';
import { repository } from './markdown-sources.mjs';
import { collectCppExamples, cppPages } from './cpp-examples.mjs';
import { collectJavaExamples, javaPages } from './java-examples.mjs';
import { collectLearningExamples, learningPages } from './learning-examples.mjs';
import { collectPythonExamples, pythonPages } from './python-examples.mjs';
import { collectTypescriptExamples, typescriptPages } from './typescript-examples.mjs';

export const documentationPages = {
  ...learningPages,
  ...pythonPages,
  ...typescriptPages,
  ...cppPages,
  ...javaPages,
  'tools/cli/index.html': ['build-cli', 'check', 'read-default', 'read-summary', 'read-json', 'read-dzn', 'read-param', 'list', 'list-dzn', 'fmt', 'fmt-check', 'fmt-diff', 'build-driver', 'conformance'],
  'tools/formatter/index.html': ['format-before', 'format-after', 'format-stdin'],
  'tools/lsp/index.html': ['sample-directive'],
  'tools/editors/index.html': ['build-jetbrains'],
  'using/rust/index.html': ['rust-dependency', 'rust-example', 'rust-doctest'],
};

// Called by the format collector: one build, one set of generated files, one check.
export async function collectDocumentationExamples({ tools, formats, execute, command }) {
  const examples = {};
  const add = (id, code, language = 'sh', output) => {
    examples[id] = { code: code.replace(/\n$/, ''), language, ...(output === undefined ? {} : { output }) };
  };
  const addCliExample = (id, args) => add(id, command(args), 'sh', execute(tools.iris, args, 0).stdout);
  const dimacs = formats.find(format => format.id === 'dimacs-cnf');
  const spec = dimacs.spec.path;
  const input = dimacs.cases.find(example => example.id === 'three-variables').input.path;
  addCliExample('check', ['check', spec]);
  for (const mode of ['default', 'summary', 'json', 'dzn']) {
    addCliExample(`read-${mode}`, ['read', spec, input, ...(mode === 'default' ? [] : [`--${mode}`])]);
  }
  const jobshop = formats.find(format => format.id === 'jobshop');
  const job = jobshop.cases.find(example => example.result.kind === 'success' && example.params.some(param => param.name === 'instance'));
  ok(job, 'Need a real selected job shop instance');
  const args = [jobshop.spec.path, job.input.path];
  addCliExample('read-param', ['read', ...args, '--summary', ...job.params.flatMap(({ name, value }) => ['--param', `${name}=${value}`])]);
  addCliExample('list', ['list', ...args]);
  addCliExample('list-dzn', ['list', ...args, '--dzn']);
  addCliExample('fmt-check', ['fmt', '--check', 'formats/']);
  addCliExample('fmt-diff', ['fmt', '--check', '--diff', 'formats/']);
  // Invoke through a portable repo-root path; the built driver can live elsewhere.
  const driverPath = 'rust/target/debug/iris-driver';
  const conformanceArgs = ['conformance', driverPath, 'conformance/cases/0.1/smoke'];
  add('conformance', command(conformanceArgs));
  execute(tools.iris, ['conformance', tools.driver, conformanceArgs[2]], 0);

  const pair = await formatterPair(tools.iris, execute);
  add('format-before', pair.before, 'iris');
  add('format-after', pair.after, 'iris');
  add('format-stdin', command(['fmt', '-']));
  await formatFileCopy(tools.iris, spec, execute);
  add('fmt', command(['fmt', spec]));

  // Source-derived setup commands are checked against their owning READMEs.
  await addReadmeCommand('build-cli', 'rust/iris-lsp/README.md', 'cargo build --locked -p gecode-iris-cli --bin iris');
  await addReadmeCommand('build-jetbrains', 'editors/jetbrains/README.md', './gradlew build verifyPlugin');
  await addReadmeCommand('rust-doctest', 'rust/gecode-iris/README.md', 'cargo test --locked -p gecode-iris --doc');
  add('build-driver', 'cargo build --locked -p iris-driver');
  // buildExampleTools above executes this build (with both binaries selected).
  const lsp = await readSource('rust/iris-lsp/README.md');
  const directive = '# iris: sample samples/small.txt';
  ok(lsp.includes(directive));
  add('sample-directive', directive, 'iris');

  add('rust-example', await rustExample(), 'rust');
  // rustdoc compiles and executes this exact body, including its hidden Ok line.
  execute('cargo', ['test', '--locked', '-p', 'gecode-iris', '--doc', '--quiet'], 0, true, { cwd: resolve(repository, 'rust') });
  add('rust-dependency', '[dependencies]\ngecode-iris = { path = "../iris/rust/gecode-iris" }', 'toml');
  Object.assign(examples, await collectLearningExamples({ tools, execute, command }));
  Object.assign(examples, await collectPythonExamples({ tools, execute }));
  Object.assign(examples, await collectTypescriptExamples());
  Object.assign(examples, await collectCppExamples());
  Object.assign(examples, await collectJavaExamples());
  return examples;

  async function addReadmeCommand(id, path, code) {
    ok((await readSource(path)).includes(`\n${code}\n`), `${path}: documented command changed`);
    add(id, code);
  }
}

function readSource(path) { return readFile(resolve(repository, path), 'utf8'); }

async function formatterPair(iris, execute) {
  const guide = await readSource('docs/formatting.md');
  const pair = guide.match(/Before:\n\n```iris\n([\s\S]*?)```\n\nAfter:\n\n```iris\n([\s\S]*?)```/);
  ok(pair, 'Formatting guide must contain a before/after pair');
  const after = execute(iris, ['fmt', '-'], 0, false, { input: pair[1] }).stdout;
  strictEqual(after, pair[2], 'Formatter differs from the style guide pair');
  return { before: pair[1], after };
}

async function formatFileCopy(iris, spec, execute) {
  const scratch = await mkdtemp(resolve(tmpdir(), 'iris-site-docs-'));
  try {
    const destination = resolve(scratch, spec);
    await mkdir(dirname(destination), { recursive: true });
    await copyFile(resolve(repository, spec), destination);
    execute(iris, ['fmt', spec], 0, false, { cwd: scratch });
  } finally {
    await rm(scratch, { recursive: true, force: true });
  }
}

async function rustExample() {
  const crate = await readSource('rust/gecode-iris/src/lib.rs');
  const doc = crate.split('\n').filter(line => line.startsWith('//!')).map(line => line.replace(/^\/\/! ?/, '')).join('\n');
  const block = doc.match(/```\n([\s\S]*?)```/);
  ok(block, 'Need the umbrella crate doctest');
  const body = block[1].replace(/^# /gm, '');
  return `fn main() -> Result<(), Box<dyn std::error::Error>> {\n${body.trimEnd().split('\n').map(line => line ? `    ${line}` : '').join('\n')}\n}\n`;
}
