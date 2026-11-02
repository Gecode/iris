import { readFile, stat } from 'node:fs/promises';
import { join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parse } from 'parse5';
import { buildDirectory, builtHtmlFiles } from './built-pages.mjs';

const origin = 'https://iris.gecode.dev';
const buildRoot = fileURLToPath(buildDirectory);
const failures = [];
const pages = new Map();
let checkedLinks = 0;

for (const path of await builtHtmlFiles()) {
  pages.set(path, collectPage(parse(await readFile(new URL(path, buildDirectory), 'utf8')), path));
}
for (const [path, page] of pages) {
  const pathname = path === 'index.html' ? '/' : `/${path.replace(/index\.html$/, '')}`;
  for (const link of page.links) await checkTarget(link, new URL(pathname, origin), path);
}
for (const failure of failures) console.error(failure);
console.log(`Checked ${pages.size} HTML pages and ${checkedLinks} local links/assets/fragments; ${failures.length} failures.`);
if (failures.length > 0) process.exitCode = 1;

function collectPage(document, path) {
  const ids = new Set();
  const links = [];
  function visit(node) {
    const attrs = Object.fromEntries((node.attrs ?? []).map(({ name, value }) => [name, value]));
    if (attrs.id) {
      if (ids.has(attrs.id)) failures.push(`${path}: duplicate id ${JSON.stringify(attrs.id)}`);
      ids.add(attrs.id);
    }
    if (node.tagName === 'a' && attrs.name) ids.add(attrs.name);
    if (['a', 'area', 'link'].includes(node.tagName) && attrs.href) links.push(attrs.href);
    if (['script', 'img', 'source'].includes(node.tagName) && attrs.src) links.push(attrs.src);
    if (['img', 'source'].includes(node.tagName) && attrs.srcset) {
      // Data URLs contain commas; they are self-contained and need no file check.
      if (!attrs.srcset.startsWith('data:')) {
        for (const candidate of attrs.srcset.split(',')) links.push(candidate.trim().split(/\s+/)[0]);
      }
    }
    if (node.tagName === 'meta' && ['og:image', 'twitter:image'].includes(attrs.property ?? attrs.name)) links.push(attrs.content);
    for (const child of node.childNodes ?? []) visit(child);
    if (node.content) visit(node.content);
  }
  visit(document);
  return { ids, links };
}

async function checkTarget(href, base, source) {
  try {
    const url = new URL(href, base);
    if (url.origin !== origin) return;
    checkedLinks++;
    const pathname = decodeURIComponent(url.pathname);
    const target = resolve(buildRoot, `.${pathname}`);
    if (relative(buildRoot, target).split(sep).includes('..')) throw new Error('path escapes the build directory');
    let targetPath = target;
    if ((await stat(targetPath)).isDirectory()) targetPath = join(targetPath, 'index.html');
    if (!(await stat(targetPath)).isFile()) throw new Error('target is not a file');
    const page = pages.get(relative(buildRoot, targetPath));
    if (page && url.hash) {
      const fragment = decodeURIComponent(url.hash.slice(1)).split(':~:')[0];
      if (fragment && !page.ids.has(fragment)) throw new Error(`missing anchor ${JSON.stringify(fragment)}`);
    }
    if (targetPath.endsWith('.webmanifest')) {
      const manifest = JSON.parse(await readFile(targetPath, 'utf8'));
      for (const icon of manifest.icons ?? []) await checkTarget(icon.src, url, source);
    }
  } catch (error) {
    failures.push(`${source}: ${JSON.stringify(href)}: ${error.code === 'ENOENT' ? 'missing file' : error.message}`);
  }
}
