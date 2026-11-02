import { copyFile, mkdir, readFile, readdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const brand = new URL('../../brand/', import.meta.url);
const publicDirectory = new URL('../public/', import.meta.url);

await copyBrand();
await renderSocialCard();
console.log('Prepared Iris brand assets and 1200×630 social card.');

async function copyBrand() {
  await mkdir(new URL('brand/', publicDirectory), { recursive: true });
  for (const name of [
    'iris-mark.svg', 'iris-logo-horizontal.svg', 'iris-logo-horizontal-dark.svg',
    'iris-logo-stacked.svg', 'iris-logo-stacked-dark.svg',
  ]) {
    await copyFile(new URL(name, brand), new URL(`brand/${name}`, publicDirectory));
  }
  for (const entry of await readdir(new URL('favicon/', brand), { withFileTypes: true })) {
    if (entry.isFile()) {
      await copyFile(new URL(`favicon/${entry.name}`, brand), new URL(entry.name, publicDirectory));
    }
  }
}

async function renderSocialCard() {
  const lockup = (await readFile(new URL('iris-logo-horizontal.svg', brand))).toString('base64');
  const petals = ['#EB891B', '#DA251D', '#005CA1', '#0B7646'];
  const image = `<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="630" viewBox="0 0 1200 630">
    <rect width="1200" height="630" fill="#EDF4FA"/>
    <image href="data:image/svg+xml;base64,${lockup}" x="240" y="118" width="720" height="394.5" preserveAspectRatio="xMidYMid meet"/>
    ${petals.map((colour, index) => `<rect x="${index * 300}" y="622" width="300" height="8" fill="${colour}"/>`).join('')}
  </svg>`;
  const result = spawnSync('rsvg-convert', [
    '--width', '1200', '--height', '630', '--output',
    fileURLToPath(new URL('og-image.png', publicDirectory)),
  ], { input: image, encoding: 'utf8' });
  if (result.error) throw new Error('Cannot run rsvg-convert; install librsvg (librsvg2-bin on Ubuntu).', { cause: result.error });
  if (result.status !== 0) throw new Error(`Social card rendering failed: ${result.stderr}`);
}
