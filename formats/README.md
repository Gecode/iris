# Format library

Ready-made Iris 0.1 reader specs for six benchmark formats from Gecode's
examples. This is the format library in [PLAN.md §8.9](../PLAN.md#89-format-library-formats),
part of milestone M2; its layout follows [§5](../PLAN.md#5-repository-layout).

| Format | Samples |
|---|---|
| [DIMACS CNF](dimacs-cnf/) | Three variables; an out-of-range literal. |
| [TSPLIB](tsplib/) | Five-city EXPLICIT and EUC_2D files; missing weights. |
| [TSPTW](tsptw/) | Three nodes; a reversed time window. |
| [Job shop](jobshop/) | Three jobs and machines in OR-Library and JSPLIB layouts; a repeated machine. |
| [Sudoku](sudoku/) | A 4×4 grid with 2×2 boxes; incompatible box dimensions. |
| [Nonogram](nonogram/) | A 5×5 X; a clue wider than the board. |

Each `<format>/<format>.iris` contains `instance_spec` from the corresponding
Gecode example at commit `599f977b7855dba2a1a5f67db1892722372067d3`, formatted
with `iris fmt` according to the [formatting guide](../docs/formatting.md).
Formatting changes only whitespace and line breaks. All six were extracted
from C++ raw string literals without escape decoding; extraction was checked
against bytes emitted by compiling the original literal. The
[Gecode notices](GECODE-LICENSE.md) accompany these specs.

## Samples and cases

Samples live directly in `cases/<name>/input.txt`, alongside their
`expected.json` or `error.json` and optional `params.json`. There is no
second sample or expectation directory. Each case follows the
[conformance contract](../conformance/README.md#case-files-and-discovery).
Its `spec.iris` is a relative file symlink to `../../<format>.iris`.
This keeps a single reader spec per format, so case copies cannot drift.
The runner supports file symlinks and discovers these cases from `formats/`.

All samples were written by hand for this repository and are freely
redistributable under its [MIT license](../LICENSE). No benchmark instances
were copied. Each format README cites its format documentation and describes
the subset read by the pinned spec.

Expected results come from running the pinned `gecode-instance` tool on
every sample. Successful dzn output was compared byte for byte with the
adapter's dzn output before capturing canonical JSON through the adapter.
Invalid samples' reference diagnostics were checked against the adapter's
canonical errors, including locations and record paths. These cases need
no known-failure annotations.

## Run the cases

From the repository root:

```sh
eval "$(scripts/gecode-reference.sh --print-env)"
adapter=$(conformance/adapters/gecode/build.sh)
(cd rust && cargo run --locked -p iris-conformance -- run --driver "\"$adapter\"" \
  --driver-id gecode-6.5.0-599f977b7855 ../formats)
```

For a direct read with the reference tool:

```sh
"$GECODE_REFERENCE_INSTANCE_TOOL" -spec formats/tsplib/tsplib.iris -dzn \
  formats/tsplib/cases/euc-2d-five/input.txt
```

Changes to the meaning of these specs belong in a later language version. Format
limitations and awkward constructions feed into [Iris 1.0 ideas](../docs/1.0-ideas.md).
