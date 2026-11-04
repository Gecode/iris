import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { deepStrictEqual, ok, strictEqual } from 'node:assert';
import { parse, parseFragment } from 'parse5';
import { buildDirectory } from './built-pages.mjs';
import { siteDirectory } from './format-sources.mjs';

// Checks the built C++ API reference against the output of the last
// prepare:cpp-api, and that it covers the public API the guide relies on.
const reference = JSON.parse(await readFile(resolve(siteDirectory, 'src/generated/cpp-api.json'), 'utf8'));
const required = {
  namespace: ['ErrorCode', 'ValueType', 'code_name()', 'type_name()'],
  spec: ['parse()', 'try_parse()', 'load()', 'try_load()', 'name()', 'read()', 'read_text()', 'try_read()', 'try_read_text()', 'catalog()', 'catalog_text()', 'try_catalog()', 'try_catalog_text()'],
  data: ['size()', 'name()', 'value()', 'find()', 'dzn()', 'at()', 'integer()', 'number()', 'text()', 'ints()', 'floats()'],
  'value-ref': ['type()', 'depth()', 'size()', 'operator[]()', 'at()', 'integer()', 'number()', 'text()', 'ints()', 'floats()'],
  params: ['set()', 'size()', 'name()', 'get()'],
  error: ['code()', 'message()', 'details()', 'input_location()', 'spec_location()', 'excerpt()', 'context()', 'record_path()', 'earlier_rejection()'],
  'error-details': ['input_location', 'spec_location', 'excerpt', 'context', 'record_path', 'earlier_rejection'],
  location: ['origin', 'line', 'column'],
  'access-error': [],
};
deepStrictEqual(reference.pages.map(page => page.slug).sort(), ['', ...Object.keys(required)].sort(), 'using/cpp/api: unexpected pages');

let members = 0;
for (const page of reference.pages) {
  const url = `using/cpp/api/${page.slug && `${page.slug}/`}`;
  const document = parse(await readFile(new URL(`${url}index.html`, buildDirectory), 'utf8'));
  const fragment = descendants(document).find(node => 'data-cpp-api' in attrs(node));
  ok(fragment, `${url}: missing the generated reference`);
  strictEqual(normalize(text(fragment)), normalize(text(parseFragment(page.html))), `${url}: stale reference; run npm run build`);
  const headings = descendants(fragment).filter(node => ['h2', 'h3'].includes(node.tagName) && attrs(node).id);
  deepStrictEqual(headings.map(node => attrs(node).id), page.headings.map(heading => heading.slug), `${url}: headings differ from the generated outline`);
  // An overloaded member's heading ends in its number, such as [1/2].
  const names = new Set(page.headings.map(heading => heading.text.replace(/\s*\[.*\]$|^enum\s+/g, '').replace(/^.*::/, '')));
  for (const name of required[page.slug] ?? []) {
    ok(names.has(name), `${url}: missing ${name}`);
    members++;
  }
}

const linksTo = href => node => node.tagName === 'a' && attrs(node).href?.startsWith(href);
const index = parseFragment(reference.pages.find(page => page.slug === '').html);
for (const slug of Object.keys(required)) {
  ok(slug === 'namespace' || descendants(index).some(linksTo(`/using/cpp/api/${slug}/`)), `using/cpp/api: the index does not link ${slug}`);
}
const guide = parse(await readFile(new URL('using/cpp/index.html', buildDirectory), 'utf8'));
ok(descendants(guide).some(linksTo('/using/cpp/api/')), 'using/cpp: missing the API reference link');
console.log(`C++ API reference: ${reference.pages.length} pages and ${members} members agree with Doxygen ${reference.doxygen} output.`);

function attrs(node) { return Object.fromEntries((node.attrs ?? []).map(({ name, value }) => [name, value])); }
function descendants(node) { return (node.childNodes ?? []).flatMap(child => [child, ...descendants(child)]); }
function text(node) { return node.nodeName === '#text' ? node.value : (node.childNodes ?? []).map(text).join(''); }
function normalize(value) { return value.replace(/\s+/g, ' ').trim(); }
