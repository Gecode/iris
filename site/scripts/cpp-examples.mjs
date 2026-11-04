import { collectExampleFiles, collectReadmeCommands } from './example-files.mjs';

// cpp/examples holds the guide's examples and their recorded output; the
// example tests in cpp/tests/CMakeLists.txt compile and run each one.
const examples = ['read', 'files', 'access', 'output', 'errors', 'expected', 'memory'];
const installCommands = {
  'cpp-install': 'cmake -S . -B build/release -G Ninja -DCMAKE_BUILD_TYPE=Release -DIRIS_BUILD_TESTS=OFF\ncmake --build build/release\ncmake --install build/release --prefix /opt/iris',
};
const cmakeSnippets = {
  'cpp-find-package': 'find_package(iris 0.1 REQUIRED CONFIG)\ntarget_link_libraries(app PRIVATE iris::iris)',
  'cpp-fetch-content': 'include(FetchContent)\nFetchContent_Declare(iris SOURCE_DIR "${PROJECT_SOURCE_DIR}/third_party/iris/cpp")\nFetchContent_MakeAvailable(iris)\ntarget_link_libraries(app PRIVATE iris::iris)',
};
export const cppPages = {
  'using/cpp/index.html': [...Object.keys(installCommands), ...Object.keys(cmakeSnippets), ...examples.map(name => `cpp-${name}`)],
};

export async function collectCppExamples() {
  const sources = examples.map(name => ({ file: `${name}.cpp`, language: 'cpp', output: true }));
  return {
    ...(await collectReadmeCommands('cpp/README.md', installCommands)),
    ...(await collectReadmeCommands('cpp/README.md', cmakeSnippets, 'cmake')),
    ...(await collectExampleFiles({ directory: 'cpp/examples', prefix: 'cpp', sources })),
  };
}
