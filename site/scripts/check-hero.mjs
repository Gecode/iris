import { strictEqual, deepStrictEqual, ok } from 'node:assert';
import { readFile } from 'node:fs/promises';
import { parse } from 'parse5';
import { buildExampleTools, collectFormatExamples, siteDirectory } from './format-examples.mjs';
import { heroRelations, headerCaptures, byteSlice, heroEntries } from '../src/data/hero.ts';
import { resolve } from 'node:path';

const { formats } = await collectFormatExamples(buildExampleTools());
const format = formats.find(format => format.id === 'dimacs-cnf');
const example = format.cases.find(example => example.id === format.homeCase);
const entries = heroEntries(example.result.json);
const entryByName = name => {
  const entry = entries.find(entry => entry.name === name);
  ok(entry, `Missing hero entry ${name}`);
  return entry;
};
const covered = new Uint8Array(Buffer.byteLength(example.input.text));
const addressed = new Set();
for (const relation of heroRelations) {
  byteSlice(format.spec.text, relation.spec);
  if (!relation.input) {
    strictEqual(relation.id, 'stop');
    strictEqual(relation.data.length, 0);
    ok(!example.input.text.includes('%'), 'Stop must be inactive');
    continue;
  }
  const input = byteSlice(example.input.text, relation.input);
  for (let i = relation.input.start; i < relation.input.end; i++) strictEqual(covered[i]++, 0, `Overlapping hero input byte ${i}`);
  for (const target of relation.data) {
    const entry = entryByName(target.entry);
    let value = entry.value;
    for (const index of target.path) value = value[index];
    ok(value !== undefined, `${relation.id}: missing data target`);
    if (relation.statement === 'clause') {
      const literals = input.split(/\s+/).map(Number);
      strictEqual(literals.pop(), 0, `${relation.id}: sentinel`);
      deepStrictEqual(value, literals, `${relation.id}: wrong clause row`);
      strictEqual(entry.depth, 2);
      ok(literals.every(literal => Math.abs(literal) <= entryByName('vars').value));
    }
    addressed.add(`${target.entry}/${target.path.join('/')}`);
  }
}
for (const [index, byte] of Buffer.from(example.input.text).entries()) if (!/\s/.test(String.fromCharCode(byte))) strictEqual(covered[index], 1, `Unmapped input byte ${index}`);
for (const capture of headerCaptures) {
  byteSlice(format.spec.text, capture.spec);
  const entry = entryByName(capture.entry);
  strictEqual(entry.depth, 0);
  strictEqual(Number(byteSlice(example.input.text, capture.input)), entry.value, `${capture.entry}: capture value`);
}
strictEqual(entryByName('clause').value.length, entryByName('clauses').value);
for (const entry of entries) {
  if (entry.depth === 0) ok(addressed.has(`${entry.name}/`), `Unmapped scalar ${entry.name}`);
  else for (const index of entry.value.keys()) ok(addressed.has(`${entry.name}/${index}`), `Unmapped row ${entry.name}/${index}`);
}
const html = parse(await readFile(resolve(siteDirectory, 'dist/index.html'), 'utf8'));
const buttons = new Map();
const renderedControls = new Map();
const renderedCaptures = new Map();
const sources = new Map();
function visit(node) {
  const attrs = Object.fromEntries((node.attrs ?? []).map(({ name, value }) => [name, value]));
  if (attrs['data-hero-value']) sources.set(attrs['data-hero-value'], text(node));
  if (attrs['data-hero-source']) sources.set(attrs['data-hero-source'], text(node));
  if (attrs['data-relations']) {
    buttons.set(attrs.id, attrs['data-relations']);
    renderedControls.set(attrs.id, text(node));
  }
  if (attrs['data-hero-capture']) renderedCaptures.set(attrs.id, text(node));
  for (const child of node.childNodes ?? []) visit(child);
}
function text(node) { return node.nodeName === '#text' ? node.value : (node.childNodes ?? []).map(text).join(''); }
visit(html);
strictEqual(sources.get('input'), example.input.text, 'Rendered hero input');
strictEqual(sources.get('spec'), format.spec.text, 'Rendered hero reader spec');
for (const entry of entries) {
  if (entry.depth === 0) strictEqual(sources.get(entry.name), String(entry.value), `Rendered ${entry.name}`);
  else entry.value.forEach((row, index) => strictEqual(sources.get(`${entry.name}/${index}`), `[${row.join(', ')}]`, `Rendered ${entry.name}/${index}`));
}
for (const capture of headerCaptures) {
  for (const side of ['input', 'spec']) strictEqual(renderedCaptures.get(`hero-${side}-capture-${capture.entry}`), capture[side].text, `Rendered capture ${side}/${capture.entry}`);
}
for (const relation of heroRelations) {
  if (relation.input) {
    const id = `hero-input-${relation.input.start}`;
    strictEqual(buttons.get(id), relation.id, `Rendered input relation ${relation.id}`);
    strictEqual(renderedControls.get(id), relation.input.text);
  }
  const specId = `hero-spec-${relation.spec.start}`;
  ok(buttons.get(specId)?.split(' ').includes(relation.id), `Rendered reader-spec relation ${relation.id}`);
  strictEqual(renderedControls.get(specId), relation.spec.text);
  for (const target of relation.data) {
    const id = `hero-data-${target.entry}${target.path.length ? `-${target.path.join('-')}` : ''}`;
    strictEqual(buttons.get(id), relation.id, `Rendered data relation ${relation.id}`);
  }
  const matches = [...buttons.values()].filter(value => value.split(' ').includes(relation.id));
  ok(matches.length >= (relation.input ? 2 : 1), `Missing rendered relation ${relation.id}`);
}
console.log('Hero mapping: UTF-8 ranges, input coverage, captures, every stored leaf, and rendered values agree with the current CLI.');
