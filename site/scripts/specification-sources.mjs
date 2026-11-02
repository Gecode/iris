import { readFile, readdir } from 'node:fs/promises';
import { resolve } from 'node:path';
import { parseFragment, serialize, serializeOuter } from 'parse5';
import { createSatteriMarkdownProcessor } from '@astrojs/markdown-satteri';
import { repository, descendants, nodeText, repositoryLink } from './markdown-sources.mjs';

export async function discoverSpecificationSources() {
  const versions = (await readdir(resolve(repository, 'spec'), { withFileTypes: true }))
    .filter(entry => entry.isDirectory() && /^\d+\.\d+(?:\.\d+)?$/.test(entry.name))
    .map(entry => entry.name).sort((a, b) => b.localeCompare(a, 'en', { numeric: true }));
  if (!versions.length) throw new Error('No Language Specification versions found');
  return Promise.all(versions.map(async version => {
    const directory = `spec/${version}`;
    const index = await readFile(resolve(repository, directory, 'README.md'), 'utf8');
    // The chapter index owns the reading order, including the grammar beside appendix A.
    const files = ['README.md', ...new Set([...index.matchAll(/\]\(([^/#)]+\.(?:md|ebnf))\)/g)].map(match => match[1]))];
    const entries = await readdir(resolve(repository, directory));
    for (const file of entries) {
      if (!files.includes(file)) throw new Error(`${directory}/${file}: missing from the chapter index`);
    }
    const pages = await Promise.all(files.map(async file => {
      const path = `${directory}/${file}`;
      const text = await readFile(resolve(repository, path), 'utf8');
      const slug = file === 'README.md' ? '' : file.replace(/\.(?:md|ebnf)$/, '');
      const title = file.endsWith('.ebnf') ? 'EBNF grammar' : text.match(/^# (.+)$/m)?.[1];
      if (!title) throw new Error(`${path}: missing title`);
      return { file, path, text, slug, title, url: `/specification/${version}/${slug ? `${slug}/` : ''}` };
    }));
    return { version, pages };
  }));
}

export async function renderSpecificationPage(page, versions) {
  const processor = await createSatteriMarkdownProcessor({ syntaxHighlight: false, smartypants: false });
  const markdown = page.file.endsWith('.ebnf') ? `\`\`\`ebnf\n${page.text}\`\`\`\n` : page.text;
  const rendered = await processor.render(markdown);
  const document = parseFragment(rendered.code);
  const title = document.childNodes.find(node => node.tagName === 'h1');
  const titleAnchor = title?.attrs.find(attr => attr.name === 'id')?.value;
  document.childNodes = document.childNodes.filter(node => node !== title);
  for (const node of descendants(document)) {
    for (const attr of node.attrs ?? []) {
      if (attr.name === 'href' || attr.name === 'src') {
        attr.value = repositoryLink(attr.value, page.path, (path, url) => {
          const target = versions.flatMap(item => item.pages).find(item => item.path === path);
          return target ? `${target.url}${url.search}${url.hash}` : undefined;
        });
      }
    }
    if (page.file === 'errors.md' && node.tagName === 'tr') addErrorAnchor(node);
  }
  wrapTables(document, page.title);
  // Keep the Markdown parser's heading IDs and markup; Code supplies the site's
  // configured Expressive Code renderer for each complete, unchanged fence.
  const blocks = document.childNodes.map(node => {
    if (node.tagName !== 'pre') return { html: serializeOuter(node) };
    const code = node.childNodes.find(child => child.tagName === 'code');
    if (!code) throw new Error(`${page.path}: code block without code`);
    const lang = code.attrs.find(attr => attr.name === 'class')?.value.replace('language-', '') ?? 'text';
    return { code: nodeText(code).replace(/\n$/, ''), lang };
  });
  return { blocks, titleAnchor, headings: rendered.metadata.headings.filter(heading => heading.depth > 1) };
}

function wrapTables(document, title) {
  const tables = descendants(document).filter(node => node.tagName === 'table').map(table => {
    const labels = descendants(table).filter(node => node.tagName === 'th').map(node => nodeText(node).trim());
    const rows = descendants(table).filter(node => node.tagName === 'tr');
    return { table, labels, rows, header: labels.join('\u0000') };
  });
  // Tables sharing a header share a layout, so one registry never mixes both.
  const recordHeaders = new Set(tables.filter(isRecordRegistry).map(item => item.header));
  let number = 0;
  for (const { table, labels, rows, header } of tables) {
    for (const row of rows) {
      row.childNodes.filter(node => node.tagName === 'td')
        .forEach((cell, column) => cell.attrs.push({ name: 'data-label', value: labels[column] ?? '' }));
    }
    const layout = recordHeaders.has(header) ? 'records' : labels.length >= 4 ? 'wide' : 'table';
    if (layout !== 'table') addTableRoles(table);
    // A table that can scroll needs a focusable region; records wrap instead.
    const region = layout === 'records' ? ' role="group"' : ' tabindex="0" role="region"';
    const wrapper = parseFragment(`<div class="iris-specification-table"${region}></div>`).childNodes[0];
    wrapper.attrs.push(
      { name: 'aria-label', value: `${title}, table ${++number}` },
      { name: 'data-columns', value: String(labels.length) },
      { name: 'data-layout', value: layout },
    );
    const parent = table.parentNode;
    parent.childNodes[parent.childNodes.indexOf(table)] = wrapper;
    wrapper.parentNode = parent;
    wrapper.childNodes = [table];
    table.parentNode = wrapper;
  }
}

// Registries keyed by their first column, with prose in the second, read best
// as one labelled record per row at every width. Narrower wide tables become
// records only on phones.
function isRecordRegistry({ labels, rows }) {
  if (labels.length < 4) return false;
  return rows.some(row => {
    const prose = row.childNodes.filter(node => node.tagName === 'td')[1];
    return prose && nodeText(prose).length > 80;
  });
}

// Records restyle table elements as blocks, which drops their implicit roles.
function addTableRoles(table) {
  const roles = { table: 'table', thead: 'rowgroup', tbody: 'rowgroup', tr: 'row', th: 'columnheader', td: 'cell' };
  for (const node of [table, ...descendants(table)]) {
    if (roles[node.tagName]) node.attrs.push({ name: 'role', value: roles[node.tagName] });
  }
}

function addErrorAnchor(row) {
  const cell = row.childNodes.find(node => node.tagName === 'td');
  const code = cell && nodeText(cell).trim();
  if (!code || !/^E-[A-Z0-9]+(?:-[A-Z0-9]+)*$/.test(code)) return;
  const anchor = code.toLowerCase();
  const link = parseFragment(`<a id="${anchor}" href="#${anchor}">${serialize(cell)}</a>`).childNodes[0];
  cell.childNodes = [link];
  link.parentNode = cell;
}
