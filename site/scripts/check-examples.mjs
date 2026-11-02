import { readFile, readdir } from 'node:fs/promises';
import { resolve } from 'node:path';
import { strictEqual, deepStrictEqual, ok } from 'node:assert';
import { parse } from 'parse5';
import { buildExampleTools, collectFormatExamples, generatedModule, siteDirectory } from './format-examples.mjs';
import { builtHtmlFiles, buildDirectory } from './built-pages.mjs';
import { examplePreview } from '../src/data/example-preview.mjs';

const { formats, files } = await collectFormatExamples(buildExampleTools());
strictEqual(await readFile(resolve(siteDirectory, 'src/generated/formats.ts'), 'utf8'), generatedModule(formats), 'Stale src/generated/formats.ts; run npm run build');
for (const directory of ['public/examples', 'dist/examples']) await checkDownloads(directory, files);
const examples = new Map(formats.flatMap(format => format.cases.map(example => [`${format.id}/${example.id}`, { format, example }])));
const required = new Set(formats.flatMap(format => ['input', 'spec', 'json', 'dzn', 'command'].map(representation => `${format.id}/${format.homeCase}/${representation}`)));
for (const representation of ['input', 'diagnostic', 'command', 'error-code']) required.add(`dimacs-cnf/literal-out-of-range/${representation}`);
const seen = new Set();
let markedCount = 0;
for (const path of await builtHtmlFiles()) {
  const document = parse(await readFile(new URL(path, buildDirectory), 'utf8'));
  for (const node of descendants(document)) {
    const attributes = attrs(node);
    const key = attributes['data-example'] ?? attributes['data-example-highlighted'];
    if (!key) continue;
    const record = examples.get(key);
    ok(record, `${path}: unknown example ${key}`);
    const representation = attributes['data-representation'];
    const source = representationText(record, representation);
    const pane = closestPane(node);
    const expected = pane ? examplePreview(source, '').text : source;
    let actual;
    if (attributes['data-example-highlighted']) {
      const code = descendants(node).find(child => child.tagName === 'code');
      ok(code, `${path}: missing highlighted code for ${key}/${representation}`);
      actual = descendants(code).filter(child => (attrs(child).class ?? '').split(' ').includes('ec-line')).map(line => text(line).replace(/\n$/, '')).join('\n');
    } else {
      strictEqual(node.tagName, 'code', `${path}: text marker belongs on code`);
      actual = text(node);
    }
    const expectedText = attributes['data-final-lf'] === 'omitted' ? expected.replace(/\n$/, '') : expected;
    strictEqual(actual, expectedText, `${path}: stale rendered ${key}/${representation}`);
    if (pane) {
      const truncated = examplePreview(source, '').truncated;
      strictEqual(attrs(pane)['data-truncated'], String(truncated), `${key}/${representation}: excerpt flag`);
      const note = descendants(pane).find(child => 'data-excerpt-note' in attrs(child));
      strictEqual(Boolean(note), truncated, `${key}/${representation}: missing or unexpected excerpt note`);
      const href = representationHref(record.example, representation);
      ok(descendants(pane).some(child => child.tagName === 'a' && attrs(child).href === href), `${key}/${representation}: missing full download link`);
      if (note) {
        ok(text(note).includes('…') && text(note).includes('Excerpt'), `${key}/${representation}: missing visible ellipsis`);
        ok(descendants(note).some(child => child.tagName === 'a' && attrs(child).href === href), `${key}/${representation}: excerpt link`);
      }
    }
    if (path === 'index.html') seen.add(`${key}/${representation}`);
    markedCount++;
  }
}
for (const marker of required) ok(seen.has(marker), `Home page is missing ${marker}`);
console.log(`Example freshness: ${formats.length} formats, ${examples.size} cases, ${files.size} downloads in public and dist, ${markedCount} rendered texts agree with the current CLI.`);

async function checkDownloads(directory, files) {
  const root = resolve(siteDirectory, directory);
  const actualPaths = (await readdir(root, { recursive: true, withFileTypes: true })).filter(entry => entry.isFile()).map(entry => resolve(entry.parentPath, entry.name).slice(root.length + 1)).sort();
  deepStrictEqual(actualPaths, [...files.keys()].sort(), `${directory}: missing or extra downloads`);
  for (const [path, expected] of files) {
    const actual = await readFile(resolve(root, path));
    ok(actual.equals(expected), `${directory}/${path}: stale download`);
  }
}
function representationText({ format, example }, representation) {
  switch (representation) {
    case 'input': return example.input.text;
    case 'spec': return format.spec.text;
    case 'command': return example.commands.readDzn;
    case 'json': return example.result.json;
    case 'dzn': ok(example.result.kind === 'success'); return example.result.dzn;
    case 'error-code': ok(example.result.kind === 'error'); return JSON.parse(example.result.json).code;
    case 'diagnostic': ok(example.result.kind === 'error'); return example.result.diagnostic;
    case 'summary': ok(example.result.kind === 'success'); return example.result.summary;
    default: throw new Error(`Unknown representation ${representation}`);
  }
}
function representationHref(example, representation) {
  return representation === 'input' || representation === 'spec' ? example.downloads[representation] : example.downloads[representation === 'diagnostic' ? 'error.txt' : representation === 'summary' ? 'summary.txt' : `data.${representation}`];
}
function attrs(node) { return Object.fromEntries((node.attrs ?? []).map(({ name, value }) => [name, value])); }
function descendants(node) { return (node.childNodes ?? []).flatMap(child => [child, ...descendants(child)]); }
function text(node) { return node.nodeName === '#text' ? node.value : (node.childNodes ?? []).map(text).join(''); }
function closestPane(node) {
  for (let parent = node.parentNode; parent; parent = parent.parentNode) if ('data-example-pane' in attrs(parent)) return parent;
  return null;
}
