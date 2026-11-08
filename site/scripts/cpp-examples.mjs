import { strictEqual } from 'node:assert';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { collectExampleFiles, collectReadmeCommands } from './example-files.mjs';
import { repository } from './markdown-sources.mjs';

// cpp/examples holds the guide's examples and their recorded output; the
// example tests in cpp/tests/CMakeLists.txt compile and run each program, and
// the format-builders test reads every format case through each builder.
const formats = ['jobshop', 'nonogram', 'dimacs_cnf', 'sudoku', 'tsptw', 'tsplib'];
const programs = ['read', 'files', 'access', 'output', 'errors', 'expected', 'memory'];
const commands = ['view'];
const installCommands = {
  'cpp-install': 'cmake -S . -B build/release -G Ninja -DCMAKE_BUILD_TYPE=Release -DIRIS_BUILD_TESTS=OFF\ncmake --build build/release\ncmake --install build/release --prefix /opt/iris',
};
const cmakeSnippets = {
  'cpp-find-package': 'find_package(iris 0.1 REQUIRED CONFIG)\ntarget_link_libraries(app PRIVATE iris::iris)',
  'cpp-fetch-content': 'include(FetchContent)\nFetchContent_Declare(iris SOURCE_DIR "${PROJECT_SOURCE_DIR}/third_party/iris/cpp")\nFetchContent_MakeAvailable(iris)\ntarget_link_libraries(app PRIVATE iris::iris)',
};
// Each builder is a header, shown alone, and a program, shown with its output.
const builderIds = formats.flatMap(name => [`cpp-build_${name}-hpp`, `cpp-build_${name}`]);
export const cppPages = {
  'using/cpp/index.html': [
    ...Object.keys(installCommands), ...Object.keys(cmakeSnippets),
    ...programs.map(name => `cpp-${name}`), ...builderIds, ...commands.map(name => `cpp-${name}`),
  ],
};

export async function collectCppExamples({ tools, execute }) {
  const sources = [
    ...programs.map(name => ({ file: `${name}.cpp`, language: 'cpp', output: true })),
    ...formats.map(name => ({ file: `build_${name}.cpp`, language: 'cpp', output: true })),
    ...commands.map(name => ({ file: `${name}.sh`, language: 'sh', output: true })),
  ];
  const collected = {
    ...(await collectReadmeCommands('cpp/README.md', installCommands)),
    ...(await collectReadmeCommands('cpp/README.md', cmakeSnippets, 'cmake')),
    ...(await collectExampleFiles({ directory: 'cpp/examples', prefix: 'cpp', sources, support: formats.map(name => `build_${name}.hpp`) })),
  };
  // A header shares its program's stem, so its id ends in -hpp.
  for (const name of formats) {
    const code = await readFile(resolve(repository, `cpp/examples/build_${name}.hpp`), 'utf8');
    collected[`cpp-build_${name}-hpp`] = { code: code.replace(/\n$/, ''), language: 'cpp' };
  }
  // The recorded view is what the iris command prints.
  for (const name of commands) {
    const { code, output } = collected[`cpp-${name}`];
    const [program, ...args] = code.trim().split(/\s+/);
    strictEqual(program, 'iris', `${name}.sh: one iris command line`);
    strictEqual(execute(tools.iris, args, 0).stdout, output, `${name}.sh: recorded output differs from the CLI`);
  }
  return collected;
}
