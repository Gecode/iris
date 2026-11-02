import { fileURLToPath } from 'node:url';

// Astro moves bundled modules for prerendering; config supplies the source root.
export const repository = import.meta.env?.IRIS_REPOSITORY_ROOT ?? fileURLToPath(new URL('../../', import.meta.url));
export const siteDirectory = fileURLToPath(new URL('../', import.meta.url));
export const github = 'https://github.com/Gecode/iris/blob/main/';

export function descendants(node) {
  return (node.childNodes ?? []).flatMap(child => [child, ...descendants(child)]);
}

export function nodeText(node) {
  return node.nodeName === '#text' ? node.value : (node.childNodes ?? []).map(nodeText).join('');
}

export function repositoryLink(href, sourcePath, route) {
  if (/^(?:[a-z][a-z\d+.-]*:|\/\/|#)/i.test(href)) return href;
  const url = new URL(href, `${github}${sourcePath}`);
  const path = decodeURIComponent(url.pathname).replace(/^\/(?:Gecode\/iris\/blob\/main\/)?/, '').replace(/\/$/, '');
  return route(path, url) ?? `${github}${path}${url.search}${url.hash}`;
}
