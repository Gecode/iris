import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { deepStrictEqual, ok, strictEqual } from 'node:assert';
import { parse, parseFragment } from 'parse5';
import { buildDirectory } from './built-pages.mjs';
import { siteDirectory } from './format-sources.mjs';

// Checks the built Python API reference against the output of the last
// prepare:python-api, and that it covers the public API the guide relies on.
const reference = JSON.parse(await readFile(resolve(siteDirectory, 'src/generated/python-api.json'), 'utf8'));
const page = parse(await readFile(new URL('using/python/api/index.html', buildDirectory), 'utf8'));
const fragment = descendants(page).find(node => 'data-python-api' in attrs(node));
ok(fragment, 'using/python/api: missing the generated reference');
strictEqual(normalize(text(fragment)), normalize(text(parseFragment(reference.html))), 'using/python/api: stale reference; run npm run build');

const headings = descendants(fragment).filter(node => ['h2', 'h3'].includes(node.tagName)).map(node => attrs(node).id);
deepStrictEqual(headings, reference.headings.map(heading => heading.slug), 'using/python/api: headings differ from the generated outline');
const required = [
  'Spec', 'Spec.parse', 'Spec.load', 'Spec.read', 'Spec.catalog',
  'Data', 'Data.entries', 'Data.ints', 'Data.array', 'Data.to_dict', 'Data.to_json', 'Data.to_dzn',
  'Entry', 'Value', 'Error', 'Error.code', 'Error.line', 'Error.column', 'Error.path', 'Error.origin', 'Error.to_json',
  'iris-command',
];
for (const id of required) ok(headings.includes(id), `using/python/api: missing ${id}`);
// The compiled module's own signatures have no types; these come from the stub.
const read = descendants(fragment).find(node => attrs(node).id === 'Spec.read');
const signature = read.parentNode.childNodes.find(node => (attrs(node).class ?? '') === 'api-signature');
ok(normalize(text(signature)).includes('params: Mapping[str, str] | None = None'), 'using/python/api: Spec.read lacks its stub signature');

const guide = parse(await readFile(new URL('using/python/index.html', buildDirectory), 'utf8'));
ok(descendants(guide).some(node => node.tagName === 'a' && attrs(node).href === '/using/python/api/'), 'using/python: missing the API reference link');
console.log(`Python API reference: ${headings.length} headings agree with pdoc ${reference.pdoc} output.`);

function attrs(node) { return Object.fromEntries((node.attrs ?? []).map(({ name, value }) => [name, value])); }
function descendants(node) { return (node.childNodes ?? []).flatMap(child => [child, ...descendants(child)]); }
function text(node) { return node.nodeName === '#text' ? node.value : (node.childNodes ?? []).map(text).join(''); }
function normalize(value) { return value.replace(/\s+/g, ' ').trim(); }
