import { readdir } from 'node:fs/promises';

export const buildDirectory = new URL('../dist/', import.meta.url);

export async function builtHtmlFiles() {
  let paths;
  try {
    paths = await readdir(buildDirectory, { recursive: true });
  } catch (error) {
    if (error.code === 'ENOENT') throw new Error('No site build found. Run npm run build before npm run check.');
    throw error;
  }
  const htmlFiles = paths.filter(path => path.endsWith('.html')).sort();
  if (htmlFiles.length === 0) throw new Error('The site build contains no HTML pages.');
  return htmlFiles;
}
