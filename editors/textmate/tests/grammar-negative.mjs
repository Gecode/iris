import { readFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';

// conformance/README.md, "Checking the formal grammar": primary rejection codes.
const rejectionCodes = new Set([
  'E-SPEC-STRING', 'E-SPEC-NUMBER', 'E-SPEC-CHARACTER', 'E-SPEC-UTF8',
  'E-SPEC-SYNTAX', 'E-SPEC-PLACEMENT', 'E-SPEC-PATTERN',
  'E-SPEC-UNKNOWN-TYPE', 'E-SPEC-NESTING',
]);

export async function isGrammarNegative(root, path) {
  if (!path.startsWith('conformance/cases/') && !path.startsWith('formats/')) return false;
  let expected;
  try {
    expected = await readFile(join(root, dirname(path), 'error.json'), 'utf8');
  } catch (error) {
    if (error.code === 'ENOENT') return false;
    throw error;
  }
  return rejectionCodes.has(JSON.parse(expected).code);
}
