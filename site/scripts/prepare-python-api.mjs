import { spawnSync } from 'node:child_process';
import { mkdir, mkdtemp, readdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { repository, siteDirectory } from './format-sources.mjs';

// Builds the Python package from python/ and renders its API reference with the
// pinned pdoc from python/pyproject.toml's docs group (see python/README.md).
const python = resolve(repository, 'python');
const output = resolve(siteDirectory, 'src/generated/python-api.json');
const scratch = await mkdtemp(resolve(tmpdir(), 'iris-python-api-'));
try {
  const wheels = resolve(scratch, 'wheels');
  run('uvx', ['maturin@1.16.0', 'build', '--out', wheels], { cwd: python });
  const [wheel, ...others] = await readdir(wheels);
  if (!wheel?.endsWith('.whl') || others.length > 0) throw new Error('maturin must build exactly one wheel');
  const environment = resolve(scratch, 'venv');
  run('uv', ['venv', '--quiet', '--python', '3.15', environment], { cwd: python });
  const interpreter = resolve(environment, process.platform === 'win32' ? 'Scripts/python.exe' : 'bin/python');
  run('uv', ['pip', 'install', '--quiet', '--python', interpreter, resolve(wheels, wheel), '--group', 'docs'], { cwd: python });
  await mkdir(resolve(siteDirectory, 'src/generated'), { recursive: true });
  run(interpreter, ['-I', resolve(python, 'docs/reference.py'), output], { cwd: python });
  console.log('Prepared the Python API reference with pdoc.');
} finally {
  await rm(scratch, { recursive: true, force: true });
}

function run(command, args, options) {
  const result = spawnSync(command, args, { stdio: ['ignore', 'inherit', 'inherit'], ...options });
  if (result.error?.code === 'ENOENT' && ['uv', 'uvx'].includes(command)) throw new Error('The Python API reference needs uv: https://docs.astral.sh/uv/');
  if (result.error || result.status !== 0) throw new Error(`${command} ${args.join(' ')} failed. ${result.error ?? ''}`);
}
