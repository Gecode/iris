import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { deepStrictEqual, ok, strictEqual } from 'node:assert';
import { parse, parseFragment } from 'parse5';
import { buildDirectory } from './built-pages.mjs';
import { siteDirectory } from './format-sources.mjs';

// Checks the built Python API references, of the package and of its spec
// builder, against the output of the last prepare:python-api, and that they
// cover the public API the guide relies on.
const reference = JSON.parse(await readFile(resolve(siteDirectory, 'src/generated/python-api.json'), 'utf8'));
const required = [
  'Spec', 'Spec.parse', 'Spec.load', 'Spec.read', 'Spec.catalog',
  'Data', 'Data.entries', 'Data.ints', 'Data.array', 'Data.to_dict', 'Data.to_json', 'Data.to_dzn',
  'Entry', 'Value', 'Error', 'Error.code', 'Error.line', 'Error.column', 'Error.path', 'Error.origin', 'Error.to_json',
  'iris-command',
];
const builderRequired = [
  'SpecBuilder', 'SpecBuilder.format', 'SpecBuilder.instances', 'SpecBuilder.source', 'SpecBuilder.build',
  'Scope', 'Scope.set', 'Scope.check', 'Scope.line', 'Scope.define_type', 'Scope.forward',
  ...['Scope0', 'Scope1', 'Scope2', 'Scope3', 'ScopeN'].flatMap(scope => [scope, `${scope}.read`, `${scope}.list`, `${scope}.let`, `${scope}.param`, `${scope}.repeat`, `${scope}.sections`, `${scope}.if_`, `${scope}.capture`]),
  ...['Scope1', 'Scope2', 'Scope3', 'ScopeN'].flatMap(scope => [`${scope}.record`, `${scope}.as_name`]),
  'Block', 'SectionsGuard', 'Arms', 'Arms.arm', 'ArmGuard',
  'Field', 'Field.name', 'IntField', 'FloatField', 'NumField', 'StrField', 'DynField', 'declared',
  'Expr', 'IntExpr', 'IntLookup', 'IntLookup.field', 'DynExpr', 'Record', 'RecordList', 'RecordExpr', 'Required', 'Maybe',
  'Type', 'IntType', 'IntType.dims', 'IntType.until', 'Alphabet', 'ListReading',
  'int_', 'int_range', 'at_least', 'num', 'word', 'eol', 'alphabet',
  'lit', 'div', 'rem', 'pow', 'sum', 'count', 'over', 'all', 'any', 'defined',
  'Part', 'bind', 'ref', 'discard', 'rest_of_line',
  'BuildError', 'BuildError.kind', 'BuildError.call_site', 'BuildErrorKind', 'CallSite', 'track_caller',
  'gecode', 'gecode.INT_MAX', 'gecode.INT_MIN',
];

const fragment = await checkPage('using/python/api', reference, required);
// The compiled module's own signatures have no types; these come from the stub.
ok(signatures(fragment, 'Spec.read').some(signature => signature.includes('params: Mapping[str, str] | None = None')), 'using/python/api: Spec.read lacks its stub signature');
const builder = await checkPage('using/python/api/build', reference.build, builderRequired);
// A statement shows each overload of the stub, typed by the depth of its scope.
const reads = signatures(builder, 'Scope1.read');
ok(reads.some(signature => signature.includes('ty: IntType[_U]) -> IntField[list[_U] | _N_co]')), 'using/python/api/build: Scope1.read lacks its typed overloads');
ok(!reads.some(signature => signature.includes('*args')), 'using/python/api/build: Scope1.read shows the signature of the implementation');
ok(!/[~+]_[A-Z]/.test(normalize(text(builder))), 'using/python/api/build: a type variable keeps its variance prefix');
// A class shows its constructor under its own signature.
ok(signatures(builder, 'SpecBuilder').some(signature => signature === 'SpecBuilder(format: str | None = None)'), 'using/python/api/build: SpecBuilder lacks its constructor');

// The introductions of the two pages name each other.
for (const [url, phrase] of [['using/python/api/', "and the spec builder's reference covers gecode_iris.build."], ['using/python/api/build/', 'imported in the guide as ib. pdoc'], ['using/python/api/build/', 'The guide shows the builder in use, and the package reference covers the specs and data it builds.']]) {
  const intro = normalize(text(parse(await readFile(new URL(`${url}index.html`, buildDirectory), 'utf8'))));
  ok(intro.includes(phrase), `${url}: the introduction lacks "${phrase}"`);
}
const guide = parse(await readFile(new URL('using/python/index.html', buildDirectory), 'utf8'));
for (const href of ['/using/python/api/', '/using/python/api/build/', '/using/python/api/build/#declared']) {
  ok(descendants(guide).some(node => node.tagName === 'a' && attrs(node).href === href), `using/python: missing the link to ${href}`);
}
console.log(`Python API reference: ${reference.headings.length} headings of the package and ${reference.build.headings.length} of the spec builder agree with pdoc ${reference.pdoc} output.`);

async function checkPage(path, page, ids) {
  const document = parse(await readFile(new URL(`${path}/index.html`, buildDirectory), 'utf8'));
  const fragment = descendants(document).find(node => 'data-python-api' in attrs(node));
  ok(fragment, `${path}: missing the generated reference`);
  strictEqual(normalize(text(fragment)), normalize(text(parseFragment(page.html))), `${path}: stale reference; run npm run build`);
  const headings = descendants(fragment).filter(node => ['h2', 'h3'].includes(node.tagName)).map(node => attrs(node).id);
  deepStrictEqual(headings, page.headings.map(heading => heading.slug), `${path}: headings differ from the generated outline`);
  for (const id of ids) ok(headings.includes(id), `${path}: missing ${id}`);
  return fragment;
}

function signatures(fragment, id) {
  const heading = descendants(fragment).find(node => attrs(node).id === id);
  return heading.parentNode.childNodes.filter(node => (attrs(node).class ?? '') === 'api-signature').map(node => normalize(text(node)));
}

function attrs(node) { return Object.fromEntries((node.attrs ?? []).map(({ name, value }) => [name, value])); }
function descendants(node) { return (node.childNodes ?? []).flatMap(child => [child, ...descendants(child)]); }
function text(node) { return node.nodeName === '#text' ? node.value : (node.childNodes ?? []).map(text).join(''); }
function normalize(value) { return value.replace(/\s+/g, ' ').trim(); }
