import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { test } from 'node:test';
import oniguruma from 'vscode-oniguruma';
import textmate from 'vscode-textmate';

test('contextual names leave following pattern captures intact', async t => {
  const require = createRequire(import.meta.url);
  await oniguruma.loadWASM(await readFile(require.resolve('vscode-oniguruma/release/onig.wasm')));
  const registry = new textmate.Registry({
    onigLib: Promise.resolve(oniguruma),
    loadGrammar: async () => textmate.parseRawGrammar(
      await readFile(new URL('../iris.tmLanguage.json', import.meta.url), 'utf8'),
      'iris.tmLanguage.json',
    ),
  });
  t.after(() => registry.dispose());
  const grammar = await registry.loadGrammar('source.iris');
  const source = await readFile(new URL('corpus/containment.iris', import.meta.url), 'utf8');
  let stack = textmate.INITIAL;
  let patterns = 0;
  for (const [index, line] of source.split('\n').entries()) {
    const result = grammar.tokenizeLine(line, stack);
    stack = result.ruleStack;
    if (line !== 'line "{after:int}"') continue;
    patterns++;
    for (const [text, scope] of [
      ['after', 'variable.other.definition.iris'],
      ['int', 'storage.type.iris'],
    ]) {
      const start = line.indexOf(text);
      for (let column = start; column < start + text.length; column++) {
        const token = result.tokens.find(token => token.startIndex <= column && token.endIndex > column);
        const location = `line ${index + 1}, column ${column + 1}`;
        assert.ok(token.scopes.includes('meta.capture.iris'), location);
        assert.ok(token.scopes.includes(scope), location);
        assert.ok(!token.scopes.some(scope => /^meta\.(?:skip|sections|format|type)\b/.test(scope)), location);
      }
    }
  }
  assert.equal(patterns, 5);
});
