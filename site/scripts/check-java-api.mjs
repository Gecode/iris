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
  entry: [],
  field: [],
  'record-reader': [],
  // The spec builder of dev.gecode.iris.build.
  build: [],
  'build/iris': ['builder(Runnable)', 'spec(Runnable)', 'int0(String)', 'string1(String)', 'read(Field, Type)', 'line(Object...)', 'repeat(Expr, Runnable)', 'param(Field)', 'check(Expr)', 'sections(Runnable)', 'lit(long)'],
  'build/builder': ['source()', 'build()'],
  'build/build-exception': ['kind()', 'reason()', 'callSite()', 'text()', 'failure()', 'emittedLine()'],
  ...Object.fromEntries([
    'alphabet', 'arm-option', 'build-exception-kind', 'discard', 'domain', 'dyn', 'dyn-expr', 'dyn-field', 'expr', 'float-expr', 'float-field', 'float-type',
    'gecode', 'handle-kind', 'int', 'int-expr', 'int-field', 'int-type', 'integral-expr', 'list-reading', 'num', 'num-expr', 'num-field', 'num-type',
    'numeric-expr', 'over', 'part', 'pattern', 'range', 'real', 'record-expr', 'record-list', 'record-name', 'section-clause', 'str', 'string-expr',
    'string-field', 'string-type', 'textual-expr', 'track-caller', 'type', 'until',
  ].map(name => [`build/${name}`, []])),
  // The annotation processor of dev.gecode:iris-processor.
  processor: [],
  'processor/iris-spec': ['value'],
  'processor/iris-spec-processor': ['ROOT_OPTION', 'process(Set, RoundEnvironment)'],
};
const kotlinRequired = ['get', 'to-int-array', 'to-long-array', 'to-double-array', 'as-sequence', 'leaves', 'to-params'];
// The spec builder of dev.gecode.iris.kotlin.build, below build/. Its other
// pages, of the members of its classes and its smaller functions, need only be
// reachable from its summary.
const kotlinBuilderRequired = [
  'build', 'build/iris-spec', 'build/iris-builder', 'build/-iris-spec', 'build/-iris-builder', 'build/-iris-builder/build', 'build/-iris-builder/source',
  'build/-spec-scope', 'build/-spec-scope/format', 'build/-spec-scope/instances',
  'build/-scope', ...['read', 'list', 'let', 'param', 'set', 'check', 'line', 'repeat', 'repeat-until', 'sections', 'if-then', 'otherwise', 'capture', 'define-type', 'forward-int'].map(name => `build/-scope/${name}`),
  'build/-scope0', 'build/-scope1', 'build/-scope1/record', 'build/-scope2', 'build/-scope3', 'build/-deep-scope', 'build/-optional-scope0', 'build/-optional-scope1', 'build/-optional-deep-scope',
  'build/-sections', 'build/-sections/arm', 'build/-capture', 'build/-capture/named', 'build/-iris-dsl',
  'build/int0', 'build/int2', 'build/string1', 'build/float-n', 'build/lit', 'build/range', 'build/range-to', 'build/range-until', 'build/at-least',
  'build/eq', 'build/le', 'build/and', 'build/or', 'build/rem', 'build/unary-minus', 'build/sum', 'build/count', 'build/over', 'build/until', 'build/word',
];

const java = await checkPages('java-api', 'using/java/api', 'data-java-api', Object.keys(javaRequired));
const kotlin = await checkPages('kotlin-api', 'using/java/kotlin-api', 'data-kotlin-api', [...kotlinRequired, ...kotlinBuilderRequired], slug => slug.startsWith('build/'));
const kotlinBuilderPages = kotlin.pages.filter(page => page.slug === 'build' || page.slug.startsWith('build/'));

let members = 0;
for (const page of java.pages) {
  const names = new Set(page.headings.map(heading => heading.text));
  for (const name of javaRequired[page.slug] ?? []) {
    ok(names.has(name), `using/java/api/${page.slug}/: missing ${name}`);
    members++;
  }
}

const linksTo = href => node => node.tagName === 'a' && attrs(node).href?.startsWith(href);
checkReachable(java, 'using/java/api', Object.keys(javaRequired), ['', 'processor']);
checkIndex(kotlin, 'using/java/kotlin-api', kotlinRequired);
checkReachable(kotlin, 'using/java/kotlin-api', kotlinBuilderPages.map(page => page.slug), ['build']);
// Dokka's own declaration and description of a class stay on its page, and a
// member's page is titled with its class.
const scope = kotlin.pages.find(page => page.slug === 'build/-scope');
ok(normalize(text(parseFragment(scope.html))).includes('sealed class Scope<I, F, N, S, B : Any, R : Any, C : Any>A block of a spec'), 'using/java/kotlin-api/build/-scope/: missing the declaration of Scope');
strictEqual(kotlin.pages.find(page => page.slug === 'build/-scope/read').title, 'Scope.read', 'using/java/kotlin-api/build/-scope/read/: unexpected title');
const builderRead = parseFragment(kotlin.pages.find(page => page.slug === 'build/-scope/read').html);
ok(descendants(builderRead).some(linksTo('/using/java/api/build/int-field/')), 'using/java/kotlin-api/build/-scope/read: IntField does not link the Java reference');
const extensions = parseFragment(kotlin.pages.find(page => page.slug === 'get').html);
ok(descendants(extensions).some(linksTo('/using/java/api/value/')), 'using/java/kotlin-api/get: Value does not link the Java reference');
// The introductions of the two Kotlin references name each other.
for (const [url, phrase] of [['using/java/kotlin-api/', "the spec builder's reference covers dev.gecode.iris.kotlin.build."], ['using/java/kotlin-api/build/', 'this reference of the Kotlin spec builder during each site build from the Kotlin sources, and its summary lists every class and function. The guide shows the builder in use, the Java builder\'s reference covers the handles, types and expressions it shares, and the extensions read the data.']]) {
  const intro = normalize(text(parse(await readFile(new URL(`${url}index.html`, buildDirectory), 'utf8'))));
  ok(intro.includes(phrase), `${url}: the introduction lacks "${phrase}"`);
}
const guide = parse(await readFile(new URL('using/java/index.html', buildDirectory), 'utf8'));
for (const href of ['/using/java/api/', '/using/java/api/processor/', '/using/java/kotlin-api/', '/using/java/kotlin-api/build/', '/using/java/kotlin-api/build/-scope/']) ok(descendants(guide).some(linksTo(href)), `using/java: missing the link to ${href}`);
console.log(`Java API reference: ${java.pages.length} pages and ${members} members agree with Javadoc ${java.javadoc} output; ${kotlin.pages.length} Kotlin pages, ${kotlinBuilderPages.length} of them of the spec builder, agree with Dokka ${kotlin.dokka} output.`);

// The reference has exactly the pages that slugs names, and any others that
// more allows.
async function checkPages(name, path, marker, slugs, more = () => false) {
  const reference = JSON.parse(await readFile(resolve(siteDirectory, `src/generated/${name}.json`), 'utf8'));
  const published = reference.pages.map(page => page.slug);
  deepStrictEqual(published.filter(slug => !more(slug) || slugs.includes(slug)).sort(), ['', ...slugs].sort(), `${path}: unexpected pages`);
  for (const slug of slugs) ok(published.includes(slug), `${path}: missing ${slug}`);
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

// Every page can be found from a root, an index that the guide links, through
// the links of published pages.
function checkReachable(reference, path, slugs, roots) {
  const pages = new Map(reference.pages.map(page => [page.slug, page]));
  const reached = new Set(roots);
  for (const slug of reached) {
    for (const node of descendants(parseFragment(pages.get(slug).html)).filter(linksTo(`/${path}/`))) {
      reached.add(attrs(node).href.slice(`/${path}/`.length).replace(/\/?(?:#.*)?$/, ''));
    }
  }
  for (const slug of slugs) ok(reached.has(slug), `${path}: no link from the index leads to ${slug}`);
}

function checkIndex(reference, path, slugs) {
  const index = parseFragment(reference.pages.find(page => page.slug === '').html);
  for (const slug of slugs) ok(descendants(index).some(linksTo(`/${path}/${slug}/`)), `${path}: the index does not link ${slug}`);
}

function attrs(node) { return Object.fromEntries((node.attrs ?? []).map(({ name, value }) => [name, value])); }
function descendants(node) { return (node.childNodes ?? []).flatMap(child => [child, ...descendants(child)]); }
function text(node) { return node.nodeName === '#text' ? node.value : (node.childNodes ?? []).map(text).join(''); }
function normalize(value) { return value.replace(/\s+/g, ' ').trim(); }
