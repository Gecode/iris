import { readFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';

// Specification §§11.4 and 11.6: source lexing and source UTF-8 failures.
const lexicalCodes = new Set([
  'E-SPEC-STRING', 'E-SPEC-NUMBER', 'E-SPEC-CHARACTER', 'E-SPEC-UTF8',
]);

export async function isLexicalNegative(root, path) {
  if (!path.startsWith('conformance/cases/')) return false;
  let expected;
  try {
    expected = await readFile(join(root, dirname(path), 'error.json'), 'utf8');
  } catch (error) {
    if (error.code === 'ENOENT') return false;
    throw error;
  }
  return lexicalCodes.has(JSON.parse(expected).code);
}
