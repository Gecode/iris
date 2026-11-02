import { readFile } from 'node:fs/promises';
import { syntaxColours } from '../src/code-themes.mjs';

const css = await readFile(new URL('../src/styles/custom.css', import.meta.url), 'utf8');
const colours = tokens(css);
const shared = Object.fromEntries(Object.entries(colours).map(([name, value]) => [name.replace('color-', ''), value]));
const rows = [];
for (const theme of ['light', 'dark']) {
  const palette = Object.fromEntries(Object.entries(colours)
    .filter(([name]) => name.startsWith(`color-iris-${theme}-`))
    .map(([name, value]) => [name.replace(`color-iris-${theme}-`, 'iris-'), value]));
  for (const background of ['canvas', 'surface', 'hero']) {
    for (const role of ['ink', 'muted', 'link', 'orange', 'red', 'blue', 'green', 'border', 'focus']) {
      check(`${theme} ${role} / ${background}`, palette[`iris-${role}`], palette[`iris-${background}`], ['border', 'focus'].includes(role) ? 3 : 4.5);
    }
  }
  check(`${theme} primary action`, palette['iris-canvas'], palette['iris-link'], 4.5);
  for (const [role, colour] of Object.entries(syntaxColours[theme])) {
    if (role !== 'background') check(`${theme} syntax ${role}`, colour, syntaxColours[theme].background, role === 'border' ? 3 : 4.5);
  }
}
for (const background of ['rail', 'rail-active']) {
  for (const role of ['rail-text', 'rail-muted', 'rail-edge', 'focus-dark']) {
    check(`${role} / ${background}`, shared[`iris-${role}`], shared[`iris-${background}`], ['rail-edge', 'focus-dark'].includes(role) ? 3 : 4.5);
  }
}
for (const role of ['ink', 'muted', 'green', 'red', 'border']) {
  check(`terminal ${role} / dark canvas in both themes`, shared[`iris-dark-${role}`], shared['iris-dark-canvas'], role === 'border' ? 3 : 4.5);
}
check('terminal focus / dark canvas in both themes', shared['iris-focus-dark'], shared['iris-dark-canvas'], 3);
for (const [name, colour] of Object.entries({ orange: '#EB891B', red: '#DA251D', blue: '#005CA1', green: '#0B7646', ink: '#27393E' })) {
  for (const background of ['#FFFFFF', shared['iris-rail'], '#081E2D']) check(`asset ${name} / ${background}`, colour, background, 0);
}
console.log('| Role | Foreground | Background | Ratio | Minimum |');
console.log('|---|---|---|---:|---:|');
for (const row of rows) console.log(`| ${row.name} | ${row.foreground} | ${row.background} | ${row.ratio.toFixed(2)}:1 | ${row.minimum || 'decorative'} |`);
const failures = rows.filter(row => row.ratio < row.minimum);
console.log(`\nChecked ${rows.filter(row => row.minimum > 0).length} text/control pairs; ${failures.length} failures. Asset rows are informational.`);
if (failures.length > 0) process.exitCode = 1;

function tokens(block) {
  return Object.fromEntries([...block.matchAll(/--([\w-]+):\s*(#[\da-f]{6})\s*;/gi)].map(([, name, value]) => [name, value]));
}

function check(name, foreground, background, minimum) {
  if (!foreground || !background) throw new Error(`Missing colour token for ${name}`);
  const a = luminance(foreground);
  const b = luminance(background);
  rows.push({ name, foreground, background, minimum, ratio: (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05) });
}

function luminance(hex) {
  const [red, green, blue] = hex.slice(1).match(/../g).map(channel => {
    const value = parseInt(channel, 16) / 255;
    return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * red + 0.7152 * green + 0.0722 * blue;
}
