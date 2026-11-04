import { readFile, readdir } from 'node:fs/promises';
import { resolve } from 'node:path';
import { deepStrictEqual, ok, strictEqual } from 'node:assert';
import { repository } from './markdown-sources.mjs';

// python/examples holds the guide's examples and their recorded output;
// python/tests/test_examples.py runs each one against the built package.
const examples = ['read', 'files', 'access', 'arrays', 'output', 'errors', 'cli'];
const installCommands = {
  'python-install': 'uv add gecode-iris\nuv pip install gecode-iris\nuvx --from gecode-iris iris --help',
  'python-install-checkout': 'uv add ../iris/python',
};
export const pythonPages = {
  'using/python/index.html': [...Object.keys(installCommands), ...examples.map(name => `python-${name}`)],
};

export async function collectPythonExamples({ tools, execute }) {
  const directory = resolve(repository, 'python/examples');
  const files = (await readdir(directory)).filter(file => !file.startsWith('.')).sort();
  const sources = examples.map(name => `${name}.${name === 'cli' ? 'sh' : 'py'}`);
  deepStrictEqual(files, [...sources, ...examples.map(name => `${name}.out`)].sort(), 'python/examples: every file must appear in the Python guide');
  const readme = await readFile(resolve(repository, 'python/README.md'), 'utf8');
  const collected = {};
  for (const [id, code] of Object.entries(installCommands)) {
    ok(readme.includes(`\n${code}\n`), `python/README.md: documented command changed: ${code}`);
    collected[id] = { code, language: 'sh' };
  }
  for (const source of sources) {
    const code = await readFile(resolve(directory, source), 'utf8');
    const output = await readFile(resolve(directory, source.replace(/\.\w+$/, '.out')), 'utf8');
    if (source.endsWith('.sh')) {
      // The Python iris command matches the Rust one; check the recording with both.
      const [program, ...args] = code.trim().split(/\s+/);
      strictEqual(program, 'iris', `${source}: one iris command line`);
      strictEqual(execute(tools.iris, args, 0).stdout, output, `${source}: recorded output differs from the Rust CLI`);
    }
    collected[`python-${source.replace(/\.\w+$/, '')}`] = { code: code.replace(/\n$/, ''), language: source.endsWith('.sh') ? 'sh' : 'python', output };
  }
  return collected;
}
