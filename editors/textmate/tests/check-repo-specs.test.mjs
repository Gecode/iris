import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';

test('repository sweep fails when a grammar regression marks valid source invalid', async t => {
  const directory = await mkdtemp(join(tmpdir(), 'iris-textmate-'));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const grammar = JSON.parse(await readFile(new URL('../iris.tmLanguage.json', import.meta.url), 'utf8'));
  grammar.repository['declaration-name'].name = 'invalid.illegal.regression.iris';
  const grammarPath = join(directory, 'broken.tmLanguage.json');
  await writeFile(grammarPath, JSON.stringify(grammar));

  const result = spawnSync(process.execPath, [
    fileURLToPath(new URL('check-repo-specs.mjs', import.meta.url)), grammarPath,
  ], { encoding: 'utf8' });
  assert.ifError(result.error);
  assert.equal(result.status, 1, result.stderr);
  assert.match(result.stderr, /\.iris:\d+:\d+: invalid\.illegal\.regression\.iris/);
  assert.match(result.stdout, /Repository tokenization: .* [1-9]\d* failures/);
});
