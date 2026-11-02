import { mkdir, rm, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { buildExampleTools, collectFormatExamples, generatedModule, documentationModule, siteDirectory } from './format-examples.mjs';

const { formats, files, documentation } = await collectFormatExamples(buildExampleTools());
const output = resolve(siteDirectory, 'public/examples');
await rm(output, { recursive: true, force: true });
for (const [path, bytes] of files) {
  const destination = resolve(output, path);
  await mkdir(dirname(destination), { recursive: true });
  await writeFile(destination, bytes);
}
await mkdir(resolve(siteDirectory, 'src/generated'), { recursive: true });
await writeFile(resolve(siteDirectory, 'src/generated/formats.ts'), generatedModule(formats));
await writeFile(resolve(siteDirectory, 'src/generated/documentation.ts'), documentationModule(documentation));
console.log(`Prepared ${formats.length} formats, ${formats.reduce((sum, format) => sum + format.cases.length, 0)} cases, and ${files.size} downloads from the local Iris CLI.`);
