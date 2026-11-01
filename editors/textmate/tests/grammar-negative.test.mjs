import assert from 'node:assert/strict';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { isGrammarNegative } from './grammar-negative.mjs';

test('only primary grammar rejection codes are skipped, regardless of operation', async t => {
  const root = await mkdtemp(join(tmpdir(), 'iris-textmate-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const path = 'conformance/cases/0.1/example/spec.iris';
  const errorPath = join(root, dirname(path), 'error.json');
  await mkdir(dirname(errorPath), { recursive: true });

  for (const code of [
    'E-SPEC-STRING', 'E-SPEC-NUMBER', 'E-SPEC-CHARACTER', 'E-SPEC-UTF8',
    'E-SPEC-SYNTAX', 'E-SPEC-PLACEMENT', 'E-SPEC-PATTERN',
    'E-SPEC-UNKNOWN-TYPE', 'E-SPEC-NESTING',
  ]) {
    await writeFile(errorPath, JSON.stringify({ code }));
    assert.equal(await isGrammarNegative(root, path), true, code);
  }
  for (const operation of ['parse', 'read', 'catalog', 'dzn']) {
    await writeFile(join(root, dirname(path), 'case.toml'), `operation = "${operation}"\n`);
    for (const code of ['E-SPEC-TYPE', 'E-SPEC-FUNCTION', 'E-RANGE', 'E-UTF8']) {
      await writeFile(errorPath, JSON.stringify({ code }));
      assert.equal(await isGrammarNegative(root, path), false, `${operation}: ${code}`);
    }
    await rm(errorPath);
    assert.equal(await isGrammarNegative(root, path), false, `${operation}: success`);
  }
  await writeFile(errorPath, JSON.stringify({ code: 'E-SPEC-STRING' }));
  assert.equal(await isGrammarNegative(root, 'editors/textmate/tests/corpus/lexical.iris'), false);
  await rm(errorPath);
  const alternate = join(root, dirname(path), 'known/example');
  await mkdir(alternate, { recursive: true });
  await writeFile(join(alternate, 'error.json'), JSON.stringify({ code: 'E-SPEC-SYNTAX' }));
  assert.equal(await isGrammarNegative(root, path), false, 'alternate error is not primary');

  const formatPath = 'formats/example/cases/rejected/spec.iris';
  const formatError = join(root, dirname(formatPath), 'error.json');
  await mkdir(dirname(formatError), { recursive: true });
  await writeFile(formatError, JSON.stringify({ code: 'E-SPEC-SYNTAX' }));
  assert.equal(await isGrammarNegative(root, formatPath), true, 'format case');
});
