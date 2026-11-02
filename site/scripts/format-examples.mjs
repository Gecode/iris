import { spawnSync } from 'node:child_process';
import { readFile, readdir, realpath } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';
import { deepStrictEqual, strictEqual } from 'node:assert';

export const repository = fileURLToPath(new URL('../../', import.meta.url));
export const siteDirectory = fileURLToPath(new URL('../', import.meta.url));
const metadata = [
  { id: 'dimacs-cnf', slug: 'dimacs-cnf', title: 'DIMACS CNF', homeCase: 'three-variables', description: 'Zero terminators end each clause and are discarded from the data.' },
  { id: 'tsplib', slug: 'tsplib', title: 'TSPLIB', homeCase: 'euc-2d-five', description: 'Coordinates remain coordinates; distance construction belongs to the host.' },
  { id: 'nonogram', slug: 'nonogram', title: 'Nonogram', homeCase: 'five-by-five', description: 'Row and column clues become ragged arrays, with one array per line.' },
  { id: 'jobshop', slug: 'job-shop', title: 'Job shop', homeCase: 'jsplib', description: 'Each job’s operations yield machine and duration arrays.' },
  { id: 'sudoku', slug: 'sudoku', title: 'Sudoku', homeCase: 'four-by-four', description: 'Blank cells become zero. Solving the puzzle belongs to the host.' },
  { id: 'tsptw', slug: 'tsptw', title: 'TSPTW', homeCase: 'three-nodes', description: 'Travel times form a matrix; checks require ordered time windows.' },
];

export function buildExampleTools() {
  const build = run('cargo', ['build', '--locked', '--manifest-path', resolve(repository, 'rust/Cargo.toml'), '-p', 'iris-cli', '--bin', 'iris', '--bin', 'iris-driver', '--message-format=json'], resolve(repository, 'rust'));
  if (build.status !== 0) throw new Error(`Cannot build Iris examples:\n${build.stderr}`);
  const artifacts = build.stdout.trim().split('\n').map(line => JSON.parse(line));
  const binary = name => {
    const path = artifacts.find(item => item.reason === 'compiler-artifact' && item.target.name === name && item.executable)?.executable;
    if (!path) throw new Error(`Cargo did not report executable ${name}`);
    return path;
  };
  return { iris: binary('iris'), driver: binary('iris-driver') };
}

export async function collectFormatExamples(tools) {
  const directories = (await readdir(resolve(repository, 'formats'), { withFileTypes: true })).filter(entry => entry.isDirectory()).map(entry => entry.name).sort();
  deepStrictEqual(directories, metadata.map(format => format.id).sort(), 'Add format metadata for new formats');
  execute(tools.iris, ['fmt', '--check', 'formats/'], 0);
  const files = new Map();
  const formats = [];
  async function copy(source, destination) {
    const bytes = await readFile(resolve(repository, source));
    files.set(destination, bytes);
    return { path: source, text: bytes.toString('utf8') };
  }
  for (const format of metadata) {
    const source = `formats/${format.id}`;
    const spec = await copy(`${source}/${format.id}.iris`, `${source}/${format.id}.iris`);
    const readme = await copy(`${source}/README.md`, `${source}/README.md`);
    const cases = [];
    const caseIds = (await readdir(resolve(repository, source, 'cases'), { withFileTypes: true })).filter(entry => entry.isDirectory()).map(entry => entry.name).sort();
    for (const id of caseIds) {
      cases.push(await collectCase(format, spec, id, source));
    }
    if (!cases.some(example => example.id === format.homeCase && example.result.kind === 'success')) throw new Error(`${source}: home case must succeed`);
    formats.push({ ...format, spec, readme, cases });
  }
  await copy('formats/GECODE-LICENSE.md', 'GECODE-LICENSE.md');
  await copy('LICENSE', 'LICENSE');
  return { formats, files };

  async function collectCase(format, spec, id, source) {
    const path = `${source}/cases/${id}`;
    const names = await readdir(resolve(repository, path));
    if (!names.includes('case.toml')) throw new Error(`${path}: missing case.toml`);
    strictEqual(await realpath(resolve(repository, path, 'spec.iris')), await realpath(resolve(repository, spec.path)), `${path}: reader spec must link to the format spec`);
    strictEqual(Number(names.includes('expected.json')) + Number(names.includes('error.json')), 1, `${path}: require one expectation`);
    const input = await copy(`${path}/input.txt`, `${path}/input.txt`);
    await copy(`${path}/case.toml`, `${path}/case.toml`);
    const paramsText = names.includes('params.json') ? (await copy(`${path}/params.json`, `${path}/params.json`)).text : '{}';
    const parameters = JSON.parse(paramsText);
    if (!parameters || Array.isArray(parameters) || typeof parameters !== 'object' || Object.values(parameters).some(value => typeof value !== 'string')) throw new Error(`${path}: parameters must be strings`);
    const params = Object.entries(parameters).map(([name, value]) => ({ name, value }));
    const paramArgs = params.flatMap(({ name, value }) => ['--param', `${name}=${value}`]);
    const args = mode => ['read', spec.path, input.path, `--${mode}`, ...paramArgs];
    const commands = { readJson: command(args('json')), readDzn: command(args('dzn')), readSummary: command(args('summary')), errorJson: names.includes('error.json') ? command(['read', spec.path, input.path, '--params', JSON.stringify(parameters)], 'iris-driver') : null };
    const result = await readCaseResult(tools, { path, spec, input, parameters, args, names });
    let catalog = null;
    if (['jobshop', 'sudoku'].includes(format.id)) {
      const listArgs = ['list', spec.path, input.path];
      catalog = { command: command(listArgs), text: execute(tools.iris, listArgs, 0).stdout, dznCommand: command([...listArgs, '--dzn']), dzn: execute(tools.iris, [...listArgs, '--dzn'], 0).stdout };
    }
    const downloads = {};
    const save = (name, content) => {
      const destination = `${path}/${name}`;
      files.set(destination, Buffer.from(content));
      downloads[name] = `/examples/${destination}`;
    };
    downloads.input = `/examples/${path}/input.txt`;
    downloads.spec = `/examples/${spec.path}`;
    if (result.kind === 'success') {
      save('data.json', result.json);
      save('data.dzn', result.dzn);
      save('summary.txt', result.summary);
    } else {
      save('error.json', result.json);
      save('error.txt', result.diagnostic);
    }
    if (catalog) { save('catalog.txt', catalog.text); save('catalog.dzn', catalog.dzn); }
    downloads.case = `/examples/${path}/case.json`;
    const example = { id, input, params, commands, result, catalog, downloads };
    files.set(`${path}/case.json`, Buffer.from(`${JSON.stringify({ ...example, spec }, null, 2)}\n`));
    return example;
  }
}

export function generatedModule(formats) {
  return `// Generated by scripts/prepare-examples.mjs.\nimport type { Format } from '../data/formats';\nexport const formats: readonly Format[] = ${JSON.stringify(formats, null, 2)};\n`;
}

function run(executable, args, cwd = repository) {
  const result = spawnSync(executable, args, { cwd, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 });
  if (result.error) throw result.error;
  return result;
}
function execute(executable, args, status, allowStderr = false) {
  const result = run(executable, args);
  if (result.status !== status || (!allowStderr && result.stderr)) throw new Error(`${executable} ${args.join(' ')}: expected exit ${status}, received ${result.status}\n${result.stderr}\n${result.stdout}`);
  return result;
}
function command(args, executable = 'iris') {
  return [executable, ...args].map(arg => /^[\w./=:-]+$/.test(arg) ? arg : `'${arg.replaceAll("'", "'\\''")}'`).join(' ');
}

async function readCaseResult(tools, { path, spec, input, parameters, args, names }) {
  let result;
  if (names.includes('expected.json')) {
    const json = execute(tools.iris, args('json'), 0).stdout;
    strictEqual(json.replace(/\n$/, ''), (await readFile(resolve(repository, path, 'expected.json'), 'utf8')).replace(/\n$/, ''), `${path}: canonical JSON differs`);
    result = { kind: 'success', json, dzn: execute(tools.iris, args('dzn'), 0).stdout, summary: execute(tools.iris, args('summary'), 0).stdout };
  } else {
    const driver = execute(tools.driver, ['read', spec.path, input.path, '--params', JSON.stringify(parameters)], 1);
    const failure = execute(tools.iris, args('dzn'), 1, true);
    strictEqual(failure.stdout, '', `${path}: error printed stdout`);
    if (!failure.stderr) throw new Error(`${path}: missing diagnostic`);
    const error = JSON.parse(driver.stdout);
    const expected = JSON.parse(await readFile(resolve(repository, path, 'error.json'), 'utf8'));
    for (const field of ['code', 'record_path']) deepStrictEqual(error[field], expected[field], `${path}: ${field}`);
    for (const field of ['input_location', 'spec_location']) {
      const actualLocation = error[field];
      const expectedLocation = expected[field];
      deepStrictEqual({ ...actualLocation, origin: await realpath(resolve(repository, actualLocation.origin)) }, { ...expectedLocation, origin: await realpath(resolve(repository, expectedLocation.origin)) }, `${path}: ${field}`);
    }
    result = { kind: 'error', json: driver.stdout, diagnostic: failure.stderr };
  }
  return result;
}
