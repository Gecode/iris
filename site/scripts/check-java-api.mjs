import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { deepStrictEqual, ok, strictEqual } from 'node:assert';
import { parse, parseFragment } from 'parse5';
import { buildDirectory } from './built-pages.mjs';
import { siteDirectory } from './format-sources.mjs';

// Checks the built Java and Kotlin API references against the output of the
// last prepare:java-api, and that they cover the public API the guide relies on.
const javaRequired = {
  spec: ['parse(byte[], String)', 'parse(String, String)', 'format()', 'read(String, String)', 'read(Path)', 'read(Path, Params)', 'catalog(Path)'],
  data: ['entries()', 'find(String)', 'get(String)', 'toJson()', 'toDzn()'],
  'data-entry': [],
  value: ['type()', 'depth()', 'size()', 'get(int)', 'at(int...)', 'asLong()', 'asDouble()', 'asString()', 'longs()', 'ints()', 'doubles()', 'strings()'],
  'value-int-value': [],
  'value-float-value': [],
  'value-string-value': [],
  'value-array-value': [],
  'value-type': ['text()'],
  params: ['of()', 'of(String, String)', 'of(Map)', 'with(String, String)', 'values()'],
  'iris-exception': ['error()', 'code()', 'origin()', 'line()', 'column()', 'path()'],
  'iris-error': ['code()', 'message()', 'inputLocation()', 'specLocation()', 'context()', 'recordPath()', 'toJson()'],
  'iris-error-spec-error': [],
  'iris-error-read-error': [],
  'iris-error-check-error': [],
  location: [],
  'error-code': ['code()'],
};
const kotlinRequired = ['get', 'to-int-array', 'to-long-array', 'to-double-array', 'as-sequence', 'leaves', 'to-params'];

const java = await checkPages('java-api', 'using/java/api', 'data-java-api', Object.keys(javaRequired));
const kotlin = await checkPages('kotlin-api', 'using/java/kotlin-api', 'data-kotlin-api', kotlinRequired);

let members = 0;
for (const page of java.pages) {
  const names = new Set(page.headings.map(heading => heading.text));
  for (const name of javaRequired[page.slug] ?? []) {
    ok(names.has(name), `using/java/api/${page.slug}/: missing ${name}`);
    members++;
  }
}

const linksTo = href => node => node.tagName === 'a' && attrs(node).href?.startsWith(href);
checkIndex(java, 'using/java/api', Object.keys(javaRequired));
checkIndex(kotlin, 'using/java/kotlin-api', kotlinRequired);
const extensions = parseFragment(kotlin.pages.find(page => page.slug === 'get').html);
ok(descendants(extensions).some(linksTo('/using/java/api/value/')), 'using/java/kotlin-api/get: Value does not link the Java reference');
const guide = parse(await readFile(new URL('using/java/index.html', buildDirectory), 'utf8'));
for (const href of ['/using/java/api/', '/using/java/kotlin-api/']) ok(descendants(guide).some(linksTo(href)), `using/java: missing the link to ${href}`);
console.log(`Java API reference: ${java.pages.length} pages and ${members} members agree with Javadoc ${java.javadoc} output; ${kotlin.pages.length} Kotlin pages agree with Dokka ${kotlin.dokka} output.`);

async function checkPages(name, path, marker, slugs) {
  const reference = JSON.parse(await readFile(resolve(siteDirectory, `src/generated/${name}.json`), 'utf8'));
  deepStrictEqual(reference.pages.map(page => page.slug).sort(), ['', ...slugs].sort(), `${path}: unexpected pages`);
  for (const page of reference.pages) {
    const url = `${path}/${page.slug && `${page.slug}/`}`;
    const document = parse(await readFile(new URL(`${url}index.html`, buildDirectory), 'utf8'));
    const fragment = descendants(document).find(node => marker in attrs(node));
    ok(fragment, `${url}: missing the generated reference`);
    strictEqual(normalize(text(fragment)), normalize(text(parseFragment(page.html))), `${url}: stale reference; run npm run build`);
    const ids = new Set(descendants(fragment).map(node => attrs(node).id));
    for (const heading of page.headings) ok(ids.has(heading.slug), `${url}: the outline's ${heading.slug} has no target`);
  }
  return reference;
}

function checkIndex(reference, path, slugs) {
  const index = parseFragment(reference.pages.find(page => page.slug === '').html);
  for (const slug of slugs) ok(descendants(index).some(linksTo(`/${path}/${slug}/`)), `${path}: the index does not link ${slug}`);
}

function attrs(node) { return Object.fromEntries((node.attrs ?? []).map(({ name, value }) => [name, value])); }
function descendants(node) { return (node.childNodes ?? []).flatMap(child => [child, ...descendants(child)]); }
function text(node) { return node.nodeName === '#text' ? node.value : (node.childNodes ?? []).map(text).join(''); }
function normalize(value) { return value.replace(/\s+/g, ' ').trim(); }
