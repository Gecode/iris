import { readFile, readdir } from 'node:fs/promises';
import { resolve } from 'node:path';
import { parseFragment, serialize } from 'parse5';
import { createSatteriMarkdownProcessor } from '@astrojs/markdown-satteri';

import { repository, descendants, nodeText, repositoryLink } from './markdown-sources.mjs';
export { repository, siteDirectory } from './markdown-sources.mjs';
// Preserve the home page's editorial choices; new formats need no entry here.
const showcase = [
  { id: 'dimacs-cnf', homeCase: 'three-variables', description: 'Zero terminators end each clause and are discarded from the data.' },
  { id: 'tsplib', homeCase: 'euc-2d-five', description: 'Coordinates remain coordinates; distance construction belongs to the host.' },
  { id: 'nonogram', homeCase: 'five-by-five', description: 'Row and column clues become ragged arrays, with one array per line.' },
  { id: 'jobshop', slug: 'job-shop', homeCase: 'jsplib', description: 'Each job’s operations yield machine and duration arrays.' },
  { id: 'sudoku', homeCase: 'four-by-four', description: 'Blank cells become zero. Solving the puzzle belongs to the host.' },
  { id: 'tsptw', homeCase: 'three-nodes', description: 'Travel times form a matrix; checks require ordered time windows.' },
];

export async function discoverFormatSources() {
  const processor = await createSatteriMarkdownProcessor({ syntaxHighlight: false });
  const directories = (await readdir(resolve(repository, 'formats'), { withFileTypes: true }))
    .filter(entry => entry.isDirectory()).map(entry => entry.name).sort();
  const formats = [];
  for (const id of directories) {
    const path = `formats/${id}/README.md`;
    const text = await readFile(resolve(repository, path), 'utf8');
    const document = parseFragment((await processor.render(text)).code);
    const title = document.childNodes.find(node => node.tagName === 'h1');
    const paragraph = document.childNodes.find(node => node.tagName === 'p');
    if (!title || !paragraph) throw new Error(`${path}: require a title and description paragraph`);
    const source = descendants(document).flatMap(node => (node.attrs ?? []).filter(attr => attr.name === 'href'))
      .map(attr => attr.value).find(href => /^https:\/\/github.com\/Gecode\/gecode\/blob\/[a-f0-9]{40}\//.test(href));
    if (!source) throw new Error(`${path}: missing pinned Gecode source link`);
    const commit = source.split('/')[6];
    const editorial = showcase.find(format => format.id === id);
    formats.push({ id, slug: id, title: nodeText(title), description: nodeText(paragraph).split(/(?<=\.)\s/)[0], ...editorial,
      provenance: { url: source, commit }, readme: { path, text }, document });
  }
  formats.sort((a, b) => {
    const rank = id => { const index = showcase.findIndex(format => format.id === id); return index < 0 ? showcase.length : index; };
    return rank(a.id) - rank(b.id) || a.id.localeCompare(b.id);
  });
  return formats.map(({ document, ...format }) => ({ ...format, readme: { ...format.readme, html: renderReadme(document, format, formats) } }));
}

function renderReadme(document, format, formats) {
  // The page frame supplies the README's title as the single h1.
  document.childNodes = document.childNodes.filter(node => node.tagName !== 'h1');
  for (const node of descendants(document)) {
    for (const attr of node.attrs ?? []) {
      if (attr.name === 'href' || attr.name === 'src') attr.value = readmeLink(attr.value, format, formats);
    }
  }
  return serialize(document).trim();
}

function readmeLink(href, format, formats) {
  return repositoryLink(href, format.readme.path, (path, url) => {
    if (path === 'formats') return '/formats/';
    for (const target of formats) {
      const root = `formats/${target.id}`;
      const page = `/formats/${target.slug}/`;
      if (path === root || path === `${root}/README.md`) return `${page}${url.hash}`;
      if (path === `${root}/${target.id}.iris`) return `${page}#reader-spec`;
      const caseId = path.startsWith(`${root}/cases/`) ? path.slice(`${root}/cases/`.length) : null;
      if (caseId && !caseId.includes('/')) return `${page}#case-${caseId}`;
      if (caseId?.endsWith('/spec.iris')) return `${page}#reader-spec`;
      if (caseId && /\/(?:input\.txt|case\.toml|params\.json)$/.test(caseId)) return `/examples/${path}${url.hash}`;
    }
    if (path === 'formats/GECODE-LICENSE.md') return '/examples/GECODE-LICENSE.md';
    if (path === 'LICENSE') return '/examples/LICENSE';
  });
}
