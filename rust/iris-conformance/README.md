# iris-conformance

The shared conformance runner for Iris implementations, part of milestone M2
in [PLAN.md §7.2](../../PLAN.md#72-runner-and-driver-protocol). It has no
dependency on the implementation crates. The CLI also uses its library to
provide `iris conformance`.

The [conformance contract](../../conformance/README.md) defines the case format,
driver protocol, runner commands, and grammar checks. From `rust/`:

```sh
cargo test --locked -p iris-conformance
cargo run --locked -p iris-conformance -- --help
```

The runner embeds two Specification files in `embedded/0.1/`, so that the
crate builds outside the repository, as in the Python source distribution:
`grammar.ebnf` is a copy of `spec/0.1/grammar.ebnf`, and `error-codes.txt`
lists the codes in the first column of `spec/0.1/errors.md`, one per line.
`tests/embedded.rs` fails when either differs from the Specification; after a
change to those files, update the copies to match.

## Comparing reader specs

`compare` tells whether two reader specs are the same program. It is how the
[builder round trip](../README.md) checks what a generated builder prints:

```sh
cargo run --locked -p iris-conformance -- compare left.iris right.iris
cargo run --locked -p iris-conformance -- compare --manifest pairs.tsv
```

Both sides go through the recognizer of the embedded formal grammar, so the
comparison needs no implementation crate. It ignores only what cannot change a
spec's meaning: layout, comments, redundant parentheses, the spelling of numbers,
strings and format names, the names of records and comprehension variables, and
an `each` or `as` that nothing uses. A reference must still reach the same
repeat or variable on both sides, so renaming is ignored and retargeting is not.

A manifest has one `ORIGINAL TAB PRINTED` line per pair, or `ORIGINAL` alone for
a skip, and a summary reports the compared and skipped counts. The command exits
0 when the sides agree, 1 on a difference, and 2 when a spec cannot be read or
recognized.

## Differential runs

`differential` compares two or more drivers on generated cases. It needs
one limit, `--cases N` or `--duration SECONDS`:

```sh
cargo run --release --locked -p iris-conformance -- differential \
  --driver "\"$PWD/target/release/iris-driver\"" \
  --driver "\"$PWD/../cpp/build/asan/iris-driver\"" \
  --seed 7 --duration 600 --jobs 8 --keep /tmp/iris-differential \
  ../conformance/cases/0.1 ../formats
```

Case `I` of seed `S` depends on `(S, I)` alone, so `--seed S --start I
--cases 1` replays it. Without `--seed` the run picks one from the clock and
prints it. Each case is made by one of two strategies:

- `generate` writes a valid reader spec, one statement per line, with
  matching input and sometimes `params.json`. Input tokens are perturbed
  now and then with boundary numbers, wrong kinds, control bytes,
  truncation and invalid UTF-8, so reads also fail.
- `mutate` takes a case from the given roots, leaving out the work-budget
  cases of chapter 10, and changes one to three tokens, lines, settings or
  parameters. Without roots only `generate` runs.

Every driver gets `parse`, `read`, `catalog` as JSON and as dzn, and `dzn`
for each case; when all agree the spec fails to parse, the rest are
skipped. Outcomes are compared as the runner compares them with an
expectation: data and catalogs by value, errors by code, line,
column and record path, dzn byte for byte. A difference, or any exit status other than 0
or 1, a timeout (`--timeout`, 30 seconds) or undecodable output, stops the
run. The case is reduced by deleting spec lines, then input lines, while it
still fails. With `--keep DIR`, `DIR/original/` and `DIR/reduced/` hold it as
a conformance case without an expectation, and `drivers.txt` holds each
driver's command, status and output; without it the reduced case is
printed. The run then prints the replay command and exits 1. It ends with a
line such as

```text
Differential: seed=7 cases=12000 comparisons=48000 disagreements=0
```

and counts per strategy, request and outcome kind. Usage and I/O errors exit
2. `tests/differential.rs` checks the subcommand with the fake driver
`tests/differential-driver.py`, and `iris-cli`'s
`tests/differential_generator.rs` checks that most generated specs are valid
and read.

Publication is disabled until milestone M7, as for the other workspace crates.
