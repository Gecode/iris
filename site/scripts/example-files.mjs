import { readFile, readdir } from 'node:fs/promises';
import { resolve } from 'node:path';
import { deepStrictEqual, ok } from 'node:assert';
import { repository } from './markdown-sources.mjs';

// A package guide's examples live as files in a package directory, where the
// package's own tests run them and compare the recorded .out output. Each
// source becomes the example `${prefix}-${stem}`; the guide must show every
// file except the named support files, such as the examples' compiler settings.
export async function collectExampleFiles({ directory, prefix, sources, support = [] }) {
  const root = resolve(repository, directory);
  const files = (await readdir(root)).filter(file => !file.startsWith('.')).sort();
  const expected = [...support, ...sources.flatMap(({ file, output }) => (output ? [file, recording(file)] : [file]))];
  deepStrictEqual(files, expected.sort(), `${directory}: every file must appear in the guide`);
  const collected = {};
  for (const { file, language, output } of sources) {
    const code = (await readFile(resolve(root, file), 'utf8')).replace(/\n$/, '');
    const example = { code, language };
    if (output) example.output = await readFile(resolve(root, recording(file)), 'utf8');
    collected[`${prefix}-${file.replace(/\.\w+$/, '')}`] = example;
  }
  return collected;
}

// Commands shown on a guide that their owning README documents verbatim.
export async function collectReadmeCommands(readmePath, commands, language = 'sh') {
  const readme = await readFile(resolve(repository, readmePath), 'utf8');
  const collected = {};
  for (const [id, code] of Object.entries(commands)) {
    ok(readme.includes(`\n${code}\n`), `${readmePath}: documented command changed: ${code}`);
    collected[id] = { code, language };
  }
  return collected;
}

function recording(file) { return file.replace(/\.\w+$/, '.out'); }
