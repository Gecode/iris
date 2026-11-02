import { HtmlValidate, formatterFactory } from 'html-validate';
import { fileURLToPath } from 'node:url';
import { readFile } from 'node:fs/promises';
import { buildDirectory, builtHtmlFiles } from './built-pages.mjs';

const config = JSON.parse(await readFile(new URL('../.htmlvalidate.json', import.meta.url), 'utf8'));
const validator = new HtmlValidate({
  ...config,
  elements: ['html5', {
    // Expressive Code uses divs for code lines and the copy button icon.
    // Permit these generated nodes only; ordinary div nesting stays checked.
    div: { phrasing: node => Boolean(node.closest('.expressive-code .ec-line, .expressive-code .copy button > div')) },
  }],
});
const format = formatterFactory('stylish');
let errors = 0;
const files = await builtHtmlFiles();
for (const path of files) {
  const report = await validator.validateFile(fileURLToPath(new URL(path, buildDirectory)));
  if (!report.valid) console.error(format(report.results));
  errors += report.errorCount;
}
console.log(`Validated ${files.length} HTML pages; ${errors} errors.`);
if (errors > 0) process.exitCode = 1;
