import { strictEqual } from 'node:assert';
import { collectExampleFiles, collectReadmeCommands } from './example-files.mjs';

// python/examples holds the guide's examples and their recorded output;
// python/tests/test_examples.py runs each one against the built package.
const builders = ['build_jobshop', 'build_nonogram', 'build_dimacs_cnf', 'build_sudoku', 'build_tsptw', 'build_tsplib'];
const commands = ['view', 'cli'];
const examples = ['read', 'files', 'access', 'arrays', 'output', 'errors', ...builders, ...commands];
const installCommands = {
  'python-install': 'uv add gecode-iris\nuv pip install gecode-iris\nuvx --from gecode-iris iris --help',
  'python-install-checkout': 'uv add ../iris/python',
};
export const pythonPages = {
  'using/python/index.html': [...Object.keys(installCommands), ...examples.map(name => `python-${name}`)],
};

export async function collectPythonExamples({ tools, execute }) {
  const sources = examples.map(name => (commands.includes(name)
    ? { file: `${name}.sh`, language: 'sh', output: true }
    : { file: `${name}.py`, language: 'python', output: true }));
  const collected = {
    ...(await collectReadmeCommands('python/README.md', installCommands)),
    ...(await collectExampleFiles({ directory: 'python/examples', prefix: 'python', sources })),
  };
  // The Python iris command matches the Rust one; check the recordings with both.
  for (const name of commands) {
    const { code, output } = collected[`python-${name}`];
    const [program, ...args] = code.trim().split(/\s+/);
    strictEqual(program, 'iris', `${name}.sh: one iris command line`);
    strictEqual(execute(tools.iris, args, 0).stdout, output, `${name}.sh: recorded output differs from the Rust CLI`);
  }
  return collected;
}
