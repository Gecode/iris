import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { strictEqual, deepStrictEqual, ok } from 'node:assert';
import { parse, parseFragment } from 'parse5';
import { createSatteriMarkdownProcessor } from '@astrojs/markdown-satteri';
import { discoverSpecificationSources } from './specification-sources.mjs';
import { repository, descendants, nodeText } from './markdown-sources.mjs';
import { builtHtmlFiles, buildDirectory } from './built-pages.mjs';

const versions = await discoverSpecificationSources();
const pages = versions.flatMap(item => item.pages);
const routes = new Map(pages.map(page => [page.path, page.url]));
const processor = await createSatteriMarkdownProcessor({ syntaxHighlight: false, smartypants: false });
const paths = await builtHtmlFiles();
deepStrictEqual(paths.filter(path => path.startsWith('specification/')).sort(),
  ['specification/index.html', ...pages.map(page => `${page.url.slice(1)}index.html`)].sort(), 'Missing or extra Specification pages');
let codeBlocks = 0;
let chapterLinks = 0;
const registries = new Map();
let errorAnchors = 0;
for (const page of pages) {
  const document = parse(await readFile(new URL(`${page.url.slice(1)}index.html`, buildDirectory), 'utf8'));
  const nodes = descendants(document);
  const body = nodes.find(node => attrs(node)['data-specification-source'] === page.path);
  ok(body, `${page.path}: missing source content`);
  strictEqual(nodeText(nodes.find(node => node.tagName === 'h1')), page.title, `${page.path}: title`);
  const markdown = page.file.endsWith('.ebnf') ? `\`\`\`ebnf\n${page.text}\`\`\`\n` : page.text;
  const rendered = await processor.render(markdown);
  const source = parseFragment(rendered.code);
  source.childNodes = source.childNodes.filter(node => node.tagName !== 'h1');
  strictEqual(prose(body), prose(source), `${page.path}: changed Specification text`);
  const sourceCodes = descendants(source).filter(node => node.tagName === 'pre').map(node => nodeText(node).replace(/\n$/, ''));
  const codeWrappers = descendants(body).filter(node => 'data-specification-code' in attrs(node));
  const actualCodes = codeWrappers.map(node => {
    const code = descendants(node).find(child => child.tagName === 'code');
    ok(code, `${page.path}: missing Expressive Code block`);
    if (['iris', 'ebnf'].includes(attrs(node)['data-specification-code'])) {
      ok(descendants(code).some(child => child.tagName === 'span' && attrs(child).style?.includes('--0:')),
        `${page.path}: missing syntax highlighting`);
    }
    return descendants(code).filter(child => (attrs(child).class ?? '').split(' ').includes('ec-line'))
      .map(line => nodeText(line).replace(/\n$/, '')).join('\n');
  });
  deepStrictEqual(actualCodes, sourceCodes, `${page.path}: changed example or grammar bytes`);
  codeBlocks += sourceCodes.length;
  for (const heading of rendered.metadata.headings) {
    ok(descendants(body).some(node => attrs(node).id === heading.slug), `${page.path}: missing heading ${heading.slug}`);
  }
  const expectedLinks = descendants(source).filter(node => node.tagName === 'a').map(node => {
    const href = attrs(node).href;
    if (/^(?:[a-z][a-z\d+.-]*:|\/\/|#)/i.test(href)) return href;
    const url = new URL(href, `https://github.com/Gecode/iris/blob/main/${page.path}`);
    const path = decodeURIComponent(url.pathname).replace('/Gecode/iris/blob/main/', '');
    if (routes.has(path)) chapterLinks++;
    return routes.has(path) ? `${routes.get(path)}${url.search}${url.hash}` : url.href;
  });
  const actualLinks = descendants(body).filter(node => node.tagName === 'a')
    .filter(node => !(attrs(node).id?.startsWith('e-') && attrs(node).href === `#${attrs(node).id}`))
    .map(node => attrs(node).href);
  deepStrictEqual(actualLinks, expectedLinks, `${page.path}: rewritten source links`);
  const versionNav = nodes.find(node => attrs(node)['aria-label'] === 'Specification language version');
  ok(versionNav, `${page.path}: missing version navigation`);
  deepStrictEqual(descendants(versionNav).filter(node => node.tagName === 'a').slice(0, versions.length)
    .map(node => nodeText(node)), versions.map(item => item.version), `${page.path}: version list`);
  const sidebar = nodes.find(node => (attrs(node).class ?? '').split(' ').includes('iris-rail-navigation'));
  ok(sidebar, `${page.path}: missing sidebar`);
  deepStrictEqual(descendants(sidebar).filter(node => node.tagName === 'a' && attrs(node).href?.startsWith('/specification/'))
    .map(node => attrs(node).href), pages.map(page => page.url), `${page.path}: sidebar order and membership`);
  if (page.file === 'errors.md') {
    const anchors = new Set(descendants(body).map(node => attrs(node).id));
    const codes = [...page.text.matchAll(/^\| `(E-[A-Z0-9-]+)` \|/gm)].map(match => match[1]);
    for (const code of codes) ok(anchors.has(code.toLowerCase()), `${page.path}: ${code}: missing registered error anchor`);
    errorAnchors += codes.length;
    registries.set(page.url, { anchors, codes });
  }
}

const lspSource = await readFile(resolve(repository, 'rust/iris-lsp/src/diagnostic.rs'), 'utf8');
const lspBase = new URL(lspSource.match(/const DIAGNOSTIC_DOCS_BASE_URL: &str = "([^"]+)";/)?.[1]);
strictEqual(lspBase.origin, 'https://iris.gecode.dev', 'LSP documentation origin');
const lspRegistry = registries.get(lspBase.pathname);
ok(lspRegistry, 'LSP documentation URL must target a rendered error registry');
const linkableCodes = new Set();
for (const path of ['iris-syntax/src/diagnostic.rs', 'iris-sema/src/diagnostic.rs', 'iris-core/src/error.rs']) {
  const source = await readFile(resolve(repository, 'rust', path), 'utf8');
  for (const [, code] of source.matchAll(/=> "(E-[A-Z0-9-]+)"/g)) linkableCodes.add(code);
}
ok(linkableCodes.size > 0, 'No LSP error codes found');
for (const code of linkableCodes) {
  ok(lspRegistry.codes.includes(code), `${code}: LSP code absent from registry`);
  ok(lspRegistry.anchors.has(code.toLowerCase()), `${code}: missing LSP destination`);
}
const redirect = await readFile(new URL('specification/index.html', buildDirectory), 'utf8');
ok(redirect.includes(`/specification/${versions[0].version}/`), 'Specification entry must lead to the latest language version');
console.log(`Specification: ${pages.length} pages, ${chapterLinks} rewritten chapter links, ${codeBlocks} unchanged code blocks, ${errorAnchors} error anchors, ${linkableCodes.size} LSP destinations.`);

function attrs(node) { return Object.fromEntries((node.attrs ?? []).map(({ name, value }) => [name, value])); }
function prose(node) {
  function visit(child) {
    if (child.tagName === 'pre' || 'data-specification-code' in attrs(child)) return '';
    return child.nodeName === '#text' ? child.value : (child.childNodes ?? []).map(visit).join('');
  }
  return visit(node).replace(/\s+/g, ' ').trim();
}
