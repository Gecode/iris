import assert from 'node:assert/strict';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { isLexicalNegative } from './lexical-negative.mjs';

test('only conformance cases expecting lexical source errors are skipped', async t => {
  const root = await mkdtemp(join(tmpdir(), 'iris-textmate-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const path = 'conformance/cases/0.1/example/spec.iris';
  const errorPath = join(root, dirname(path), 'error.json');
  await mkdir(dirname(errorPath), { recursive: true });

  for (const code of ['E-SPEC-STRING', 'E-SPEC-NUMBER', 'E-SPEC-CHARACTER', 'E-SPEC-UTF8']) {
    await writeFile(errorPath, JSON.stringify({ code }));
    assert.equal(await isLexicalNegative(root, path), true, code);
  }
  for (const code of ['E-SPEC-SYNTAX', 'E-SPEC-PATTERN', 'E-SPEC-TYPE', 'E-UTF8']) {
    await writeFile(errorPath, JSON.stringify({ code }));
    assert.equal(await isLexicalNegative(root, path), false, code);
  }
  await writeFile(errorPath, JSON.stringify({ code: 'E-SPEC-STRING' }));
  assert.equal(await isLexicalNegative(root, 'editors/textmate/tests/corpus/lexical.iris'), false);
  await rm(errorPath);
  assert.equal(await isLexicalNegative(root, path), false, 'no expected error');
});
