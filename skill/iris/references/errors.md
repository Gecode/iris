# Reading Iris errors

Load this when `iris check` or `iris read` fails and the cause is not
obvious from the message.

## The two kinds of error

**Spec errors** come from `iris check` (and from `iris read` before any input
is read). They name the reader-spec line:

```text
spec.iris:2: unknown type 'wrd'
2 | repeat n {x:int y:wrd}
  |                      ^
```

**Read errors** come from `iris read`. They name the input position, show
the input line with a caret, the execution path, and the spec line that was
running:

```text
input.txt:4:1: expected int, found 'query'
     4 | query 4
       | ^
while reading: repeat [2 of 3] > args
spec: spec.iris:4
```

- `input.txt:4:1` is line 4, column 1 (columns count bytes).
- `while reading:` lists the active structure, outermost first:
  `repeat [2 of 3]` is the second of three iterations, `repeat [5]` an
  iteration of `repeat until`, `section "PATTERN"` a sections arm, and a
  name is the value being read.
- `spec: spec.iris:4` is the statement that failed.

A check failure points at the start of the failing record and names it:
`check failed for record 2 of 'edges': self-loops are not allowed`.

The CLI prints messages, not error codes, and never prints partial data.
Both kinds exit with status 1; usage errors exit with 2.

## Where the real cause is

The position is where reading *noticed* the problem. When the message
makes no sense for that spot, the spec usually got out of step earlier: a
count was wrong, a record read one token too many or too few, or a
line-aware statement moved to the next line. To find where the spec and the
input diverge:

1. Read the `while reading:` path: which iteration failed? Compare it with
   the sample's record of that number.
2. Truncate the input before the failure, or delete statements after the
   suspect one and add `ignore trailing`, then run
   `iris read --json` to see what was read so far. Remove `ignore trailing`
   afterwards.
3. Check every count: is it the number of records, of lines, or of values?
   Does the file count from 0 or 1?
4. Look for the line traps: `list ... until eol`, `rest`, `line`, and
   `skip line` move to the next line when the current one is used up.

`iris check` validates syntax, types, placement, and record scoping, but
not names used in expressions: a misspelled name such as `a: 1..nn` passes
`iris check` and fails at read time with `'nn' is not defined yet`.

## Spec errors

| Message (abbreviated) | Cause and fix |
|---|---|
| `unknown type 'string'` | There is no `string` type: use `word` (one token) or `rest` (rest of line). Also reported when a named type is used before its `type` statement or outside its block. |
| `unknown statement 'for'` / `'and'` / `'or'` | No such statement. Loops are `repeat`; logic is `&&`, `\|\|`, `!`. |
| `expected a statement, found '='` | `check x = 1`: comparison is `==`. |
| `expected a statement, found '/'` | `//` is not a comment; use `#`. |
| `unexpected character ';'` or `'''` | No semicolons; strings use double quotes. |
| `missing '}'` | Unbalanced braces. |
| `the setting 'comment' must be at the top level of the spec` | `comment`, `stop`, `tokens`, `separators`, `ignore trailing` cannot be inside a block. |
| `'instances' must be the last statement of the spec` | Put all statements inside or before `instances`. |
| `'each' must be the first statement of a repeat` | Move `each` to the start of the body. |
| `a list cannot be of 'rest' or of an alphabet type` | Use `repeat n { s: rest }`. |
| `'rest' values cannot be arrays` | Same. |
| `'offset' needs numbers` / `'layout' needs a two-dimensional array of numbers` | Options only apply to numeric reads (layout: 2-D). |
| `an alphabet type needs a two-dimensional array` | Declare `g[rows][cols]: alphabet "..."`. |
| `unterminated capture in pattern` / `capture name 'bad name' in pattern ... is not a name` | Fix the `{...}` in the pattern string. |
| `unknown function 'len'` | Functions are `size`, `count`, `min`, `max`, `sum`, `distinct`, `defined`, and the comprehensions `all`, `any`, `count`, `sum`. |
| `'x' names a type, so it cannot also name a value` | Type names and value names share one namespace. Rename one. |
| `'x' is bound by a comprehension but is also a name in the spec` | Comprehension variables must be fresh names. |
| `'_' cannot be defined here` | `_` discards only in reads and captures. |
| `a check can only range over the records of one repeat, but 'x' and 'y' are defined in different repeats` | Aggregate one side with `let` first (see [language.md §10](language.md#10-checks)). |
| `'n' is defined in several repeats, or inside and outside a repeat, so a check here cannot range over its records` | Use distinct names for top-level values and repeat fields. |
| `'rows' is a list of records, not a value` | Use `size(rows)`, `rows[i].field`, or a field. |
| `only records have fields such as '.a'` | Field access needs a record name from `as`/`each`. |
| `'edges' cannot name both the list and each record` | `as` names the list and `each` names one record, so they need different names: `repeat m as edges { each edge ... }`. |
| `'items' is a list in each record of the repeat around it; use 'row.items'` | Reach nested lists through the outer record. |

## Read errors

| Message (abbreviated) | Cause and fix |
|---|---|
| `expected int, found 'x'` | A token is not an integer. Often the spec is out of step: a previous read took too many or too few tokens. |
| `expected an int in 1..3, found '5'` | Range violation: the value is invalid, or the range or count is wrong. |
| `expected a value, found end of input` / `expected int, found end of input` | The input ended early: a count is too large, a record reads too many values, or a line-aware statement moved past the data. |
| `expected a line of values, found end of input` | `list ... until eol` found no further line with data. |
| `unexpected input after the end of the spec` | Data remains after the last statement: a count is too small, the format has a trailer, or the spec misses a part. Add `ignore trailing` only if the trailer is meaningless. |
| `expected a line matching "..."` | A `line` pattern failed. Compare spacing (`N={n}` rejects `N = 5`), case, and literal text. Also reported for a required sections arm that never matched (`... before the end of the input`). |
| `no line matching "..." found` | `find` reached the end of the input. |
| `unexpected line; expected one of ...` | A line matched no sections arm. Add an arm, or `skip other`. |
| `a second line matching "..." (the first is on line 3)` | A sections arm matched twice. One sections statement cannot read repeated records; use `repeat`, with the sections inside it when each record is one of several line forms. |
| `expected 'END' before end of input` | A `repeat until "END"` never met its terminator. |
| `expected list terminator, found end of input` | A sentinel list never met its sentinel. |
| `the body of a repeat until read no input` | A `repeat until` body consumed nothing. |
| `expected a symbol at row 2, column 1` | An alphabet met a character outside the alphabet, or the input ended. Add the character to the alphabet or to `blank` or `strip`. |
| `'x' is defined in only some repetitions` / `in no repetition` | A scalar field was read in some iterations only. Define it in every branch (`let x = 0`) or make it an array. |
| `'x' has rows of different depths` | One branch reads `x` as a scalar, another as an array. Make the shapes agree. |
| `'x' mixes strings and numbers` | A column has both. Use separate names for string and numeric values. |
| `'z' is not defined yet` | The name is misspelled, defined later, or defined on a branch that did not run. Use `defined(z)` to test. |
| `check failed: MESSAGE` / `check failed for record 2 of 'edges'` | The data violates a check, or the check is wrong. Remember that a repeat field in a check means each record. |
| `cannot index a scalar` / `'count' needs an array` in a check | A repeat field in a check is one record's scalar. Compute the whole-column value with `let` before the check. |
| `'*' needs integers` | Float or string in arithmetic. Iris has no float arithmetic. |
| `'<' compares numbers` | Strings can only be compared with `==` and `!=`. |
| `array index out of range` | An index outside 0..size-1. |
| `array size is negative (-1)` / `repeat count is negative` | A count expression is negative. Bound counts with ranges such as `n: 0..`. |
| `expected index 2, found 3` | An `index` statement found the wrong row number. |
| `cannot skip a line at end of input` | `skip` ran past the data. |
| `expected 'cnf', found 'sat'` | An `expect` token differs. |
| `the file has 2 instances; select one with the parameter instance` | Use `--param instance=N`, or read all cases with `repeat` instead of `instances`. |
| `parameter 'p' must be given` / `must be an integer, not 'x'` / `the spec has no parameter 'q'` | Supply or fix `--param NAME=VALUE`. |
| `the expressions do too much work for the size of the input` | A check or `let` visits too many values (quadratic comprehension). Use `distinct`, `sum`, or `let` aggregates. |
| `too many repetitions or empty rows without reading input` | A large counted repeat or many empty rows read no input. |
| `input is not valid UTF-8` | The input file must be UTF-8. |
