import { spawnSync } from 'node:child_process';
import { mkdir, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { parse, serialize } from 'parse5';
import { repository, siteDirectory } from './format-sources.mjs';

// Runs Doxygen over the public C++ headers with cpp/Doxyfile, and keeps the
// content of its namespace, class and main pages for the site's own shell.
const apiPath = '/using/cpp/api/';
const obsoleteAttributes = new Set(['align', 'border', 'cellpadding', 'cellspacing', 'valign']);
const output = resolve(siteDirectory, 'src/generated/cpp-api.json');
const scratch = await mkdtemp(resolve(tmpdir(), 'iris-cpp-api-'));
try {
  const version = run(['--version'], process.env).trim().split(/\s+/)[0];
  run(['Doxyfile'], { ...process.env, IRIS_DOXYGEN_OUTPUT: scratch });
  const html = resolve(scratch, 'html');
  const published = (await readdir(html)).filter(file => slug(file) !== undefined).sort();
  const pages = await Promise.all(published.map(async file => extractPage(file, await readFile(resolve(html, file), 'utf8'), published)));
  await mkdir(resolve(siteDirectory, 'src/generated'), { recursive: true });
  await writeFile(output, JSON.stringify({ doxygen: version, pages }));
  console.log(`Prepared ${pages.length} pages of the C++ API reference with Doxygen ${version}.`);
} finally {
  await rm(scratch, { recursive: true, force: true });
}

function run(args, env) {
  const result = spawnSync('doxygen', args, { cwd: resolve(repository, 'cpp'), env, encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'] });
  if (result.error?.code === 'ENOENT') throw new Error('The C++ API reference needs doxygen: https://www.doxygen.nl/');
  if (result.error || result.status !== 0) throw new Error(`doxygen ${args.join(' ')} failed. ${result.error ?? ''}`);
  return result.stdout;
}

// The URL below the reference of a page to publish; other pages are indexes
// that the site's navigation replaces.
function slug(file) {
  if (file === 'index.html') return '';
  if (file === 'namespaceiris.html') return 'namespace';
  const type = /^(?:class|struct)iris_1_1_(.+)\.html$/.exec(file);
  return type && !type[1].endsWith('-members') ? type[1].replaceAll('_', '-') : undefined;
}

function extractPage(file, source, published) {
  const document = parse(source);
  const title = text(find(document, node => hasClass(node, 'title'))).trim();
  const contents = find(document, node => hasClass(node, 'contents'));
  dropGenerationNote(contents);
  for (const node of descendants(contents)) {
    // Older versions of Doxygen lay out tables with attributes HTML has dropped.
    node.attrs = node.attrs?.filter(({ name }) => !obsoleteAttributes.has(name));
    if (hasClass(node, 'permalink')) detach(node);
    else if (node.tagName === 'a' && attribute(node, 'href') === undefined) moveAnchor(node);
    else if (node.tagName === 'a') setAttribute(node, 'href', siteHref(attribute(node, 'href'), file, published));
  }
  const headings = [];
  for (const node of descendants(contents)) {
    if (hasClass(node, 'memtitle')) node.tagName = node.nodeName = 'h3';
    const id = attribute(node, 'id');
    if (id && ['h2', 'h3'].includes(node.tagName)) headings.push({ depth: Number(node.tagName[1]), slug: id, text: text(node).trim() });
  }
  return { slug: slug(file), title, headings, html: serialize(contents) };
}

// Everything after the last rule names the header files.
function dropGenerationNote(contents) {
  const siblings = contents.childNodes;
  const rule = siblings.findLastIndex(node => node.tagName === 'hr');
  if (rule >= 0) siblings.splice(rule);
}

// Doxygen marks a target with an empty <a>. A heading takes its id itself, so
// that the outline can link it; any other target becomes a span.
function moveAnchor(anchor) {
  const id = attribute(anchor, 'id') ?? attribute(anchor, 'name');
  const siblings = anchor.parentNode.childNodes;
  const next = siblings.slice(siblings.indexOf(anchor) + 1).find(node => node.nodeName !== '#text' || node.value.trim());
  const heading = [anchor.parentNode, next].find(node => /^h[1-6]$/.test(node?.tagName));
  if (heading) {
    setAttribute(heading, 'id', id);
    detach(anchor);
    return;
  }
  anchor.tagName = anchor.nodeName = 'span';
  anchor.attrs = [{ name: 'id', value: id }];
}

function siteHref(href, file, published) {
  if (/^[a-z]+:/.test(href)) return href;
  const [target, fragment] = href.split('#');
  if (target === '') return href;
  if (!published.includes(target)) throw new Error(`${file}: link to the unpublished ${href}`);
  return `${apiPath}${slug(target) === '' ? '' : `${slug(target)}/`}${fragment === undefined ? '' : `#${fragment}`}`;
}

function find(root, test) {
  const node = descendants(root).find(test);
  if (!node) throw new Error('Doxygen page without the expected structure');
  return node;
}
function descendants(node) { return (node.childNodes ?? []).flatMap(child => [child, ...descendants(child)]); }
function text(node) { return node.nodeName === '#text' ? node.value : (node.childNodes ?? []).map(text).join(''); }
function attribute(node, name) { return node.attrs?.find(attr => attr.name === name)?.value; }
function hasClass(node, name) { return (attribute(node, 'class') ?? '').split(/\s+/).includes(name); }
function detach(node) { node.parentNode.childNodes.splice(node.parentNode.childNodes.indexOf(node), 1); }
function setAttribute(node, name, value) {
  node.attrs = [...node.attrs.filter(attr => attr.name !== name), { name, value }];
}
