import { collectExampleFiles, collectReadmeCommands } from './example-files.mjs';

// typescript/examples holds the guide's examples and their recorded output;
// typescript/test/examples.test.ts type-checks them all and runs the ones with
// a recording against the built package.
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
  ],
};

export async function collectTypescriptExamples() {
  return {
    ...(await collectReadmeCommands('typescript/README.md', installCommands)),
    ...(await collectReadmeCommands('typescript/README.md', contentSecurityPolicy, 'text')),
    ...(await collectExampleFiles({ directory: 'typescript/examples', prefix: 'typescript', sources, support: ['tsconfig.json'] })),
  };
}
