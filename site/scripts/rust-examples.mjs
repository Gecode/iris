import { collectExampleFiles } from './example-files.mjs';

// rust/gecode-iris/examples holds a spec builder for each format and its
// recorded output; the format_builders test in rust/gecode-iris runs each
// program and reads every case of its format through the builder.
const builders = ['jobshop', 'nonogram', 'dimacs_cnf', 'sudoku', 'tsplib', 'tsptw'];
export const rustBuilderExamples = builders.map(name => `rust-build-${name}`);

export async function collectRustExamples() {
  const sources = builders.map(name => ({ file: `${name}.rs`, language: 'rust', output: true }));
  return collectExampleFiles({ directory: 'rust/gecode-iris/examples', prefix: 'rust-build', sources });
}
