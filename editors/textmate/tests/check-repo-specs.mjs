import { execFileSync } from 'node:child_process';
import { readFile, readdir, realpath } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import oniguruma from 'vscode-oniguruma';
import textmate from 'vscode-textmate';
import { isGrammarNegative } from './grammar-negative.mjs';

const root = fileURLToPath(new URL('../../../', import.meta.url));
const trackedPaths = execFileSync('git', ['ls-files', '-z', '--', '*.iris'], {
  cwd: root,
  encoding: 'utf8',
}).split('\0').filter(Boolean);
// Include the local corpus before it is staged, too.
const localPaths = (await readdir(new URL('.', import.meta.url), { recursive: true }))
  .filter(path => path.endsWith('.iris'))
  .map(path => join('editors/textmate/tests', path));
const paths = [...new Set([...trackedPaths, ...localPaths])].sort();
const grammarPath = process.argv[2] ?? new URL('../iris.tmLanguage.json', import.meta.url);
const require = createRequire(import.meta.url);
await oniguruma.loadWASM(await readFile(require.resolve('vscode-oniguruma/release/onig.wasm')));
const registry = new textmate.Registry({
  onigLib: Promise.resolve(oniguruma),
  loadGrammar: async () => textmate.parseRawGrammar(
    await readFile(grammarPath, 'utf8'),
    'iris.tmLanguage.json',
  ),
});
const grammar = await registry.loadGrammar('source.iris');
const decoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });
let failures = 0;
let lines = 0;
let tokenized = 0;
let skipped = 0;
let duplicates = 0;
const realPaths = new Set();
for (const path of paths) {
  if (await isGrammarNegative(root, path)) {
    skipped++;
    continue;
  }
  let source;
  try {
    const realPath = await realpath(join(root, path));
    if (realPaths.has(realPath)) {
      duplicates++;
      continue;
    }
    realPaths.add(realPath);
    source = decoder.decode(await readFile(realPath));
  } catch (error) {
    console.error(`${path}: ${error.message}`);
    failures++;
    continue;
  }
  tokenized++;
  let stack = textmate.INITIAL;
  for (const [index, line] of source.split('\n').entries()) {
    const result = grammar.tokenizeLine(line, stack);
    stack = result.ruleStack;
    lines++;
    for (const token of result.tokens) {
      const invalid = token.scopes.filter(scope => scope === 'invalid' || scope.startsWith('invalid.'));
      if (invalid.length > 0) {
        console.error(`${path}:${index + 1}:${token.startIndex + 1}: ${invalid.join(' ')} ${JSON.stringify(line.slice(token.startIndex, token.endIndex))}`);
        failures++;
      }
    }
  }
}
registry.dispose();
console.log(`Repository tokenization: ${trackedPaths.length} tracked reader specs, ${paths.length} including local corpus; ${tokenized} tokenized, ${skipped} grammar negatives skipped, ${duplicates} duplicate paths skipped, ${lines} lines, ${failures} failures`);
if (failures > 0) process.exitCode = 1;
