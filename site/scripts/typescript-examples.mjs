import { strictEqual } from 'node:assert';
import { collectExampleFiles, collectReadmeCommands } from './example-files.mjs';

// typescript/examples holds the guide's examples and their recorded output;
// typescript/test/examples.test.ts type-checks them all and runs the ones with
// a recording against the built package, and
// typescript/test/format-builders.test.ts reads every format case through
// each builder.
const formats = ['jobshop', 'nonogram', 'dimacs-cnf', 'sudoku', 'tsptw', 'tsplib'];
const commands = ['view'];
const sources = [
  { file: 'read.ts', output: true },
  { file: 'require.cjs', output: true },
  { file: 'browser.ts', output: false },
  { file: 'worker.ts', output: false },
  { file: 'inline.ts', output: true },
  { file: 'manual.ts', output: true },
  { file: 'files.ts', output: true },
  { file: 'access.ts', output: true },
  { file: 'integers.ts', output: true },
  { file: 'output.ts', output: true },
  { file: 'errors.ts', output: true },
  { file: 'budget.ts', output: true },
  { file: 'deadline-worker.ts', output: false },
  { file: 'deadline.ts', output: true },
  ...formats.map(name => ({ file: `build-${name}.ts`, output: true })),
].map(source => ({ ...source, language: source.file.endsWith('.cjs') ? 'js' : 'ts' }));
const installCommands = {
  'typescript-install': 'npm install gecode-iris',
  'typescript-install-checkout': '(cd ../iris/typescript && npm ci && npm run build)\nnpm install ../iris/typescript',
};
const contentSecurityPolicy = { 'typescript-csp': "Content-Security-Policy: script-src 'self' 'wasm-unsafe-eval'" };
export const typescriptPages = {
  'using/typescript/index.html': [
    ...Object.keys(installCommands),
    ...Object.keys(contentSecurityPolicy),
    ...sources.map(({ file }) => `typescript-${file.replace(/\.\w+$/, '')}`),
    ...commands.map(name => `typescript-${name}`),
  ],
};

export async function collectTypescriptExamples({ tools, execute }) {
  const shown = [...sources, ...commands.map(name => ({ file: `${name}.sh`, language: 'sh', output: true }))];
  const collected = {
    ...(await collectReadmeCommands('typescript/README.md', installCommands)),
    ...(await collectReadmeCommands('typescript/README.md', contentSecurityPolicy, 'text')),
    ...(await collectExampleFiles({ directory: 'typescript/examples', prefix: 'typescript', sources: shown, support: ['tsconfig.json'] })),
  };
  // The recorded view is what the iris command prints.
  for (const name of commands) {
    const { code, output } = collected[`typescript-${name}`];
    const [program, ...args] = code.trim().split(/\s+/);
    strictEqual(program, 'iris', `${name}.sh: one iris command line`);
    strictEqual(execute(tools.iris, args, 0).stdout, output, `${name}.sh: recorded output differs from the CLI`);
  }
  return collected;
}
