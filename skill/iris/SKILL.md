---
name: iris
description: Iris reader specs (`.iris` files) that describe text input formats for Gecode and other solvers. Use when writing a reader spec from a format description or sample input, when debugging or explaining an existing spec, or when fixing an `iris check` or `iris read` error.
---

# Iris reader specs

A reader spec (or *spec*) is a short program in the Iris language that
reads a text input format into named values: integers, floats, strings, and
arrays of them. The `iris` CLI checks specs and reads files with them.
Gecode 6.5.0 embeds specs to read instance files. This skill covers
**Iris 0.1**, the Gecode 6.5.0 language; write nothing from other versions.

```sh
iris check SPEC                    # validate the spec: "SPEC: ok"
iris read SPEC FILE                # summary of the data (same as --summary)
iris read SPEC FILE --json         # the data as JSON
iris read SPEC FILE --param n=5    # set a parameter or select an instance
iris list SPEC FILE                # list the instances of a multi-instance file
iris fmt SPEC                      # format in place (--check, --diff)
```

## Workflow

Keep scratch specs and test inputs in a temporary directory unless the
task says where they belong. Do each step until its completion criterion
holds.

1. **Inspect the sample.** Read the format description and at least one
   sample file. Note every count and what it counts (values, records, or
   lines), whether identifiers start at 0 or 1, separators and punctuation,
   comment lines, sentinels and terminators, floats, strings, and what ends
   the input. *Done when* you can say which statement reads each token of
   the sample.
2. **Look for a close match** in [references/formats.md](references/formats.md)
   (DIMACS CNF, TSPLIB, TSPTW, job shop, Sudoku, nonogram). If one fits,
   start from it. *Done when* you have a starting spec or know none fits.
3. **Draft the spec.** Find each input shape in
   [references/patterns.md](references/patterns.md) and adapt its spec.
   Name values as the consuming model will use them. Put a range on every
   count and identifier (`n: 1..`, `u: 1..n`), and use `offset -1` to
   make 1-based identifiers 0-based when the model wants that.
4. **Run `iris check`.** *Done when* it prints `ok`.
5. **Read the sample.** Run `iris read`, then `iris read --json`. Compare
   each entry with the sample by hand: the counts, the first and last
   record, the shape of every array (depth and lengths), and the type of
   each value. *Done when* every entry is what the model expects.
6. **Fix errors** with [references/errors.md](references/errors.md). The
   reported position is where reading *noticed* the problem; the cause is
   often a wrong count or a line-aware statement earlier.
7. **Add checks** for every invariant the model relies on that ranges do
   not already enforce: permutations (`distinct`), sums against
   capacities, ordering of two fields of one record, Gecode limits
   (`gecode.int_max`). *Done when* every assumption the model makes about
   the data is a range or a `check`.
8. **Test invalid and edge-case input.** Make copies of the sample: one per
   check and range you rely on that violates it, one with a count too
   large (input ends early), one with trailing junk, and the smallest valid
   input (zero records, if allowed). *Done when* every invalid copy fails
   with a message that names the problem and every valid one reads.
9. **Format.** Run `iris fmt SPEC`, then `iris check` and `iris read` once
   more. *Done when* both still succeed on the formatted spec.

## Cheat sheet

```iris
format example # names the format
comment "#" # lines starting with "#" are skipped
n: 1.. # an integer >= 1
m: 0..
cost[n]: float # n floats, across any number of lines
repeat m as edges {
  each edge
  u: 1..n offset -1 # read 1..n, store 0..n-1
  v: 1..n offset -1
}
check edge.u != edge.v "self-loops are not allowed"
list targets: 1..n until -1 offset -1 # values up to the sentinel -1
title: rest # the rest of the line, as a string
```

```input
# a small graph
3 2
1.5 2 0.25
1 2
2 3
3 1 -1
Small graph
```

```data
n = 3
m = 2
cost = [1.5, 2.0, 0.25]
u = [0, 1]
v = [1, 2]
targets = [2, 0]
title = "Small graph"
```

| Statement | Meaning |
|---|---|
| `x: T`, `a[e][e]: T` | Read a value, or a fixed-size array in row-major order. `_` reads and discards. |
| Types `T` | `int`, `float`, `num` (int if possible), `word` (one token), `rest` (rest of line, scalar only), ranges `lo..hi`, `lo..<hi`, `lo..`, `..hi`, `alphabet "chars"` (character grids), and names from `type t = 1..n`. No `string` type. |
| `list x: T until S` | Values up to the sentinel token `S` (a number or a string such as `"END"`). |
| `list x: T until eol` | The values on the rest of the line. |
| `repeat e { }`, `repeat until "END" { }`, `repeat until eof { }` | Loops. Each value read inside becomes an array with one element per iteration (a *column*). |
| `repeat e as recs { each rec ... }` | Names the records (`recs[0].u`, `size(recs)`) and the current record (`rec.u`). |
| `index 1` | Inside a repeat: read the 1-based row number and verify it. |
| `line "p cnf {vars:int} {clauses:0..}"` | Match a whole line, capturing values. `{x}` is a word, `{_}` discards, `...` at the end accepts any remaining text. |
| `find "pattern"`, `skip line`, `skip e lines`, `expect "tok"` | Seek a line, discard lines, require literal tokens. |
| `sections until "EOF" skip other { "KEY : {k:int}" optional ... }` | Header lines in any order, each at most once (TSPLIB style). |
| `if c { } else if c { } else { }` | Conditional reading. Branches do not create a scope. |
| `let x = e`, `check c "message"` | Derived value; invariant. |
| `param p = 3` | Value from `--param p=...`. |
| `instances { }` | Several instances per file; one is read (last statement). |
| Settings, top level only | `format NAME`, `comment "c"`, `stop "%"`, `separators ","`, `tokens "()"`, `ignore trailing`. Each takes effect where it appears and stays on. |

Expressions: integer arithmetic `+ - * / % ^`; comparisons; `&& || !`;
`a[i]` (from 0); `size`, `count`, `min`, `max`, `sum`, `distinct`,
`defined`; comprehensions `all(x in a: e)`, `any`, `count`, `sum`;
`gecode.int_max` (2147483646) and `gecode.int_min`. Strings compare only
with `==` and `!=`. Comments in a spec start with `#`.

The data is a list of entries in definition order. A value read inside a
repeat becomes a column: a scalar field gives an array, an array field
gives an array of rows (which may be ragged), and nested repeats add
levels. There is no record type in the output.

## Traps

These cause most wrong specs. The rules behind them are in
[references/language.md](references/language.md).

- **Line-aware statements move on.** `line`, `find`, `skip`, `rest`,
  `list ... until eol`, and `sections` work on the rest of the current
  line, or on the *next* line with data when the current one is used up.
  `list args: int until eol` on a line with no arguments reads the next
  line; it never returns an empty list.
- **Blank lines are invisible**, and comment prefixes match only at the
  start of a line: in `5 # note`, `#` and `note` are data.
- **Every scalar read in a repeat must be read in every iteration.**
  Otherwise reading fails with `'x' is defined in only some repetitions`.
  Give it a default with `let x = 0` in the other branches, or read it as
  an array (`x[1]: int` / `x[0]: int`); an empty array becomes an empty
  row.
- **A column replaces any earlier entry of the same name**, including a
  top-level count: `n: int repeat n { n: int }` leaves only the column.
  Use distinct names everywhere.
- **In a check, a repeat field means each record.** `check sum(demand) <=
  cap` after the repeat checks each demand separately. Aggregate first:
  `let total = sum(demand)` then `check total <= cap`.
- **No float arithmetic.** `x * 2` fails for a float `x`; floats can only
  be compared.
- **Patterns match literally.** A space in a pattern accepts any run of
  blanks, including none, so `"N = {n:int}"` accepts `N=5` and `N = 5`,
  while `"N={n:int}"` rejects `N = 5`.
- **Trailing input is an error** (`unexpected input after the end of the
  spec`) unless the spec says `ignore trailing`. Usually it means a count
  is wrong.
- **Quadratic checks exhaust the work budget** on a few thousand values.
  Use `distinct(a)` instead of nested comprehensions.
- **`iris check` does not resolve names in expressions.** A typo such as
  `1..nn` passes the check and fails at read time; always read a sample.

## What Iris 0.1 cannot express

Before reading a packed token as one string, try splitting it:
`separators` for punctuation between values, `tokens` for letters between
numbers, an alphabet for tokens of fixed length, and a line pattern for
fixed punctuation ([patterns.md](references/patterns.md#compact-tokens)).
A number too big for 64 bits splits too: `tokens "0123456789"` and
`list digit: 0..9 until eol` read it as an exact digit array.

Several common needs have no direct form. The full table, with
workarounds and Specification sections, is at the end of
[references/patterns.md](references/patterns.md#what-iris-01-cannot-express).
In brief: blank lines as separators or empty records; comments after data
on a line; stopping a repeat on a value, or on a multi-token terminator
whose first token can also start a record (`0 0` after records that
never start with `0` works: `repeat until "0" { ... }` then
`expect "0"`); grids without given dimensions; string lengths and ordering; float
arithmetic and bounded floats; position-dependent checks such as sorted
order; integers beyond 64 bits or in hexadecimal; spaces as grid cells;
reading every instance of a multi-instance file through `instances` (use
`repeat`). When a format needs one of these, say so, read what Iris can,
and leave the rest to the host program.

## References

Load a reference when its condition holds.

- [references/patterns.md](references/patterns.md): writing a spec for a
  given input shape (counts then records, test-case counts, input until
  end of file, sentinels, varying row lengths, strings, real numbers,
  query lines with an operation keyword, character grids, headers and
  punctuation), and the table of what cannot be expressed.
- [references/language.md](references/language.md): the exact rules of a
  statement, type, expression, or check; settings; sections; parameters
  and instances; limits.
- [references/errors.md](references/errors.md): a failing `iris check` or
  `iris read`, or a message you do not understand.
- [references/formats.md](references/formats.md): the format library
  (DIMACS CNF, TSPLIB, TSPTW, job shop, Sudoku, nonogram), as models of
  idiomatic specs.
- [references/embedding.md](references/embedding.md): using a spec from
  Gecode (`InstanceOptions`, `Instance::Spec`), Rust, or another language.

The normative definition is the Iris Language Specification for Iris 0.1
(`spec/0.1/` in the Iris repository). Where this skill and the
Specification differ, the Specification is right.
