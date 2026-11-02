# Iris 0.1 language reference

A condensed guide to writing reader specs in Iris 0.1, the language of
Gecode 6.5.0. The normative text is the Iris Language Specification in
`spec/0.1/` of the Iris repository; section numbers below (§) point into it.
Every example here passes `iris check`, and those with input were read with
`iris read --json`.

## Contents

1. [Execution model and data](#1-execution-model-and-data)
2. [How input is scanned](#2-how-input-is-scanned)
3. [Spec syntax](#3-spec-syntax)
4. [Settings](#4-settings)
5. [Reading values](#5-reading-values)
6. [Line statements and patterns](#6-line-statements-and-patterns)
7. [Repeats, records, and columns](#7-repeats-records-and-columns)
8. [Conditionals, let, and sections](#8-conditionals-let-and-sections)
9. [Expressions](#9-expressions)
10. [Checks](#10-checks)
11. [Parameters and instances](#11-parameters-and-instances)
12. [Limits and work budgets](#12-limits-and-work-budgets)

## 1 Execution model and data

A spec is a sequence of statements executed from top to bottom against the
input. Statements read values, match lines, structure the reading, compute
values, or check them. There are no loops other than `repeat`, no
functions, and no way to go back in the input.

The result, the *data*, is a list of named entries in definition order.
Each entry has a value type (`int`, `float`, or `string`) and a depth (0 for
a scalar, 1 for an array, 2 for an array of rows, and so on). Arrays can be
ragged. Every name defined by a declaration, `list`, capture, `let`,
`param`, or `store` becomes an entry; names from `type`, `as`, and `each`
do not. Defining a name again replaces its value (and type and depth) but
keeps its position.

`iris read --json` prints the data as canonical JSON:
`{"iris":"0.1","entries":[{"name":"n","type":"int","depth":0,"value":3}, ...]}`.
In this skill, data is shown as `name = value` lines.

Integers are signed 64-bit. Floats are finite binary64. Strings are UTF-8.
There is no boolean type: conditions produce the integers 0 and 1.

## 2 How input is scanned

**Tokens.** By default, a token is a maximal run of bytes other than space,
tab, LF, CR, vertical tab, and form feed. `word`, `int`, `float`, `num`,
ranges, `expect`, `list` with a sentinel, and fixed-size arrays read tokens
and ignore line breaks: `a[4]: int` reads `1 2` / `3 4` and `1 2 3 4` alike.

**Normal skipping** happens before every read: skip separators, then, at the
start of a line (only spaces, tabs, and CRs before it on that line), skip a
line that begins with a `comment` prefix or stop at a line that begins with a
`stop` prefix. Blank lines are therefore invisible: they never produce an
empty value or an empty row.

**Line-aware statements** are `line`, `find`, `skip`, `list ... until eol`,
`rest`, and `sections`. They work on the *rest of the current line*. When the
current line has no data left, they move to the next line that has data.
This rule causes most line-related surprises:

- `list a: int until eol` after a token that ended its line reads the
  *next* line, never an empty list.
- `skip line` discards the rest of the current line if data remains on it,
  and otherwise discards the next line with data.
- `x: rest` takes the rest of the current line, or the next line with data
  when the current one is used up.

**Line endings.** LF ends a line; CR LF works too (the CR is trimmed from
`rest` and from line matching). A final LF is optional.

**Trailing input.** After the last statement, any remaining data (after
normal skipping) is the error `unexpected input after the end of the spec`.
The setting `ignore trailing` allows it.

**Positions.** Lines and columns start at 1; columns count bytes.

## 3 Spec syntax

- Comments start with `#` and run to the end of the line. `//` is not a
  comment.
- Statements need no separators; newlines are only whitespace. There are
  no semicolons. Several statements may share a line.
- Names are ASCII `[A-Za-z_][A-Za-z0-9_]*`, case-sensitive. Statement words
  are contextual keywords: `line: int` declares a value named `line`. Prefer
  names that are not keywords anyway. Names starting with `gecode.` are
  reserved.
- Strings use double quotes, end on the same line, and know four escapes:
  `\n`, `\t`, `\\`, `\"`. There are no single-quoted strings.
- Numbers in expressions: `12`, `1.5`, `1e3`, `2.`. A negative number is
  unary minus applied to a literal.
- Blocks are `{ ... }` and belong to `repeat`, `if`/`else`, `instances`, and
  sections arms. Standalone blocks do not exist.
- An expression continues across a newline when the next token is an
  operator, `[`, `.`, or `(`. Do not start a line with `-` or `(` after an
  expression unless you mean to continue it.

## 4 Settings

Settings may appear only at the top level (never inside a block) and take
effect when execution reaches them. Repeated settings accumulate, and none
can be turned off. Put them first unless earlier input must be read
without them: a `tokens "0123456789"` placed after `n: 1..` reads the
count `12` whole and splits only what follows.

| Setting | Effect |
|---|---|
| `format name` | Names the format (metadata only). The name may contain glued `-` and `.`, as in `format dimacs-cnf`, or be a string. |
| `comment "c" "%%"` | Lines starting with any prefix are skipped. Only at the start of a line; `5 # note` is data. |
| `stop "%"` | A line starting with the prefix ends the input. |
| `tokens "()"` | Each listed byte is a token by itself: `(1` reads as `(` and `1`. |
| `separators ","` | Each listed byte separates tokens like a space. |
| `ignore trailing` | Allow unread input after the last statement. |

Prefix matching is exact and case-sensitive. If a prefix ends with a letter,
digit, or `_`, the next character must not be one: `comment "c"` skips
`c foo` and `c` but reads `cat` as data.

```iris
comment "#"
separators ","
n: 1..
pairs[n][2]: int
```

```input
# comma-separated pairs
2
1,2
3,4
```

```data
n = 2
pairs = [[1, 2], [3, 4]]
```

## 5 Reading values

### Declarations and arrays

`x: type` reads one value. `x[e1][e2]...: type` reads an array of fixed
size in row-major order (last index fastest), ignoring line breaks. Sizes
are integer expressions in 0..2147483647; a zero size reads nothing. The
name `_` reads and validates a value but stores nothing.

### Types

| Type | Reads | Stored as |
|---|---|---|
| `int` | Decimal integer token: `-7`, `+007`. No hex, no `1e2`, no `2.0`. | `int` |
| `float` | Decimal number token: `3`, `-0.5`, `.5`, `1e-3`. No `inf` or `nan`. | `float` |
| `num` | Integer if possible, else float. One float in an array turns the whole array into floats. | `int` or `float` |
| `word` | Any token. | `string` |
| `rest` | The rest of the line, trailing blanks trimmed. Scalar only. | `string` |
| `lo..hi`, `lo..<hi`, `lo..`, `..hi`, `..<hi` | Integer in the range (inclusive, upper-exclusive, or open). Bounds are integer expressions. | `int` |
| `alphabet "chars" ...` | One character per cell of a two-dimensional array (see below). | `int` |
| A named type | As its definition. | As its definition |

There is no `string` type (use `word` or `rest`) and no bounded `float`.

### Named types

`type id = 1..n` names a type. The bounds are evaluated once, where the type
is defined; later changes to `n` do not affect it. A named type is visible
after its definition in the same block and in nested blocks. Type names must
differ from all value names in the spec.

### Offsets and layouts

`offset e` adds the integer `e` to every number read, after range checking.
It is the standard way to turn 1-based identifiers into 0-based ones.
`layout e` applies to a two-dimensional numeric array and names a TSPLIB
matrix layout: `"FULL_MATRIX"`, `"UPPER_ROW"`, `"LOWER_ROW"`,
`"UPPER_DIAG_ROW"`, `"LOWER_DIAG_ROW"`, `"UPPER_COL"`, `"LOWER_COL"`,
`"UPPER_DIAG_COL"`, or `"LOWER_DIAG_COL"`. Triangular layouts read only one
triangle (with or without the diagonal), mirror it, and leave an unread
diagonal 0. When both appear, `layout` comes before `offset`.

```iris
n: 2..
type city = 1..n
start: city offset -1
dist[n][n]: 0.. layout "LOWER_DIAG_ROW"
```

```input
3 2
0
5 0
6 7 0
```

```data
n = 3
start = 1
dist = [[0, 5, 6], [5, 0, 7], [6, 7, 0]]
```

### Lists

`list x: type until SENTINEL` reads values until a token equal to the
sentinel, across lines. The sentinel is consumed and not stored, and is not
range-checked. A numeric sentinel is compared as a number; a string sentinel
such as `"END"` is compared with the token text, so it can end a numeric
list. For a `word` list, use a string sentinel.

`list x: type until eol` reads the values on the rest of the current line,
or on the next line with data if the current one is used up. It never
returns an empty list: a blank line is skipped, not read as empty.

A list may end with `offset e`. Lists cannot be `rest` or alphabets.

```iris
n: 1..
list order: 1..n until 0 offset -1
list tags: word until eol
list scores: float until "END"
```

```input
3
3 1 2 0
red green blue
1.5 2
END
```

```data
n = 3
order = [2, 0, 1]
tags = ["red", "green", "blue"]
scores = [1.5, 2.0]
```

### Alphabets (character grids)

`g[r][c]: alphabet "symbols"` reads `r * c` single characters, skipping
whitespace (and comment lines) between them, so rows need not be on separate
lines and may contain spaces. Symbol `k` (from 0) of the alphabet has value
`k + base`; `base` defaults to 1. Options, in any order:

- `blank "."`: these characters are 0.
- `strip "|-"`: these characters are skipped like whitespace.
- `base 0`: the value of the first symbol.

Any other character is an error. An alphabet needs exactly two dimensions,
cannot be a list or a capture, and works on bytes, so use ASCII symbols.

```iris
h: 1..
w: 1..
maze[h][w]: alphabet ".#SE" base 0
```

```input
2 3
S.#
.#E
```

```data
h = 2
w = 3
maze = [[2, 0, 1], [0, 1, 3]]
```

## 6 Line statements and patterns

| Statement | Effect |
|---|---|
| `line "pattern"` | The rest of the current line (or the next line with data) must match the pattern; consumes the line. |
| `find "pattern"` | Discard lines until one matches; consume it. |
| `skip line`, `skip e lines` | Discard lines (rule in §2). Blank and comment lines do not count. |
| `expect "p" "cnf"` | The next tokens must be exactly these strings, in order. |

### Patterns

A pattern is literal text with captures. It must match the whole rest of the
line (trailing spaces ignored), exactly and case-sensitively.

| Item | Meaning |
|---|---|
| `{x:type}` | Read a value of any type except an alphabet, store it as `x`. |
| `{x}` | Same as `{x:word}`. |
| `{:int}`, `{_}` | Check a value without storing it. |
| `{$x}` | Match the current value of `x` (an integer or a string). Defines nothing. |
| A space or tab | Matches any run of spaces and tabs, including none. Between two word-like items (captures, letters, digits) at least one blank is required in the input. |
| `...` | At the end of a pattern: accept any remaining text. |

So `"DIMENSION : {n:int}"` accepts `DIMENSION: 5` and `DIMENSION :5`, while
`"N={n:int}"` rejects `N = 5`. Write a space around punctuation in a
pattern to accept optional spacing in the input.

Captures match greedily and never backtrack: a word capture ends at a blank
or at the first character of the literal that follows it; a numeric capture
takes the longest number-like run; a `rest` capture takes everything left.
`{x:float}e` therefore fails on `2e`. Range bounds of captures are checked
after the whole line matched, so `"{v:0..n} {n:int}"` works. A capture's
range violation is an error, not a mismatch.

```iris
line "p edge {n:1..} {m:0..}"
repeat m {
  line "e {u:1..n} {v:1..n} ..."
}
line "name: {title:rest}"
```

```input
p edge 3 2
e 1 2 the rest of this line is ignored
e 2 3
name: Small graph
```

```data
n = 3
m = 2
u = [1, 2]
v = [2, 3]
title = "Small graph"
```

## 7 Repeats, records, and columns

### Forms

| Form | Iterations |
|---|---|
| `repeat e { ... }` | `e` times; `e` is evaluated once and must be in 0..2147483647. |
| `repeat until "END" "STOP" { ... }` | Until the next token is one of the strings. That token is consumed; `store x` before the block keeps it in `x`. Missing terminator is an error. |
| `repeat until eof { ... }` | Until no data remains. The body must consume input. |

The terminator test happens only *before* each iteration, against the next
token after normal skipping, with exact text comparison.

### Columns

Each name defined in a repeat body becomes an array with one element per
iteration: a scalar field becomes a depth-1 column, an array field a column
of rows (ragged if lengths differ), and nested repeats add further levels.
Records are stored column-wise.

```iris
t: 1..
repeat t {
  n: 0..
  a[n]: int
}
```

```input
2
3
1 2 3
1
9
```

```data
t = 2
n = [3, 1]
a = [[1, 2, 3], [9]]
```

Rules that matter when you design the body:

- **A scalar must be defined in every iteration.** If an `if` reads `x: int`
  in only some iterations, reading fails with `'x' is defined in only some
  repetitions`. Define it in every branch (for example with `let x = 0`),
  or make it an array, such as `x[1]: int` or `x[0]: int`: a missing or
  empty array is an empty row.
- **All rows of a column must have the same depth**, and a column cannot
  mix strings and numbers. Integers and floats mix by promotion to float.
- **The column replaces any earlier entry of the same name**, including a
  top-level scalar: after `n: int repeat n { n: int }`, `n` is the column.
  Use distinct names everywhere.
- Inside the body, a field's value from the previous iteration is not
  available; enclosing values are.
- A repeat with zero iterations still creates its (empty) columns.

### Records, `as`, and `each`

Each iteration is a *record*. `repeat n as nodes { each node ... }` names
the list of records `nodes` and the current record `node`; either is
optional, and `each` must be the first statement of the body. Record names
are not data entries and must differ from all other names.

- `nodes[0].ready`: a field of one record. `size(nodes)` counts records.
- `node.ready` inside the body: the current record's field.
- In checks after the repeat, `node.ready` (or plain `ready`) means *each*
  record (§10).
- Nested lists are reached through a record: `job.steps`, `jobs[1].steps`.

### `index`

`index b` inside a repeat reads an integer that must equal the current
iteration number plus `b` (iterations count from 0), then discards it. Use
`index 1` for 1-based row numbers in the input.

```iris
n: 1..
repeat n as items {
  each item
  index 1
  weight: 0..
  value: 0..
}
check item.weight > 0 "items must have positive weight"
```

```input
2
1 3 10
2 4 7
```

```data
n = 2
weight = [3, 4]
value = [10, 7]
```

## 8 Conditionals, let, and sections

`if e { ... } else if f { ... } else { ... }` runs one branch. Branches
introduce no scope: a definition inside is visible afterwards.

`let x = e` evaluates an expression and stores it as an entry. Use it for
derived values, defaults, and whole-column aggregates for checks.
`defined(x)` tests whether `x` has been defined on the executed path.

### Sections

`sections` handles lines in any order, as in TSPLIB headers:

```iris
sections until "EOF" skip other {
  "NAME : {name:rest}" optional
  "DIMENSION : {n:1..}"
  "CAPACITY : {capacity:0..}"
  "DEMAND_SECTION" {
    repeat n {
      index 1
      demand: 0..capacity
    }
  }
}
```

```input
NAME : small one
TYPE : CVRP
DIMENSION: 3
CAPACITY : 10
DEMAND_SECTION
1 0
2 4
3 5
EOF
```

```data
name = "small one"
n = 3
capacity = 10
demand = [0, 4, 5]
```

- Each line is matched against the arms in order; the first match runs. An
  arm's body may read the following input (the section's data).
- Every arm must match **exactly once**, unless marked `optional` (at most
  once). A second matching line is an error, so sections cannot read
  repeated records; use `repeat` for those.
- `until "EOF"` ends at that token (optional at end of input); `store x`
  keeps it. `skip other` ignores lines no arm matches; without it they are
  errors.
- An arm marked `last` ends the statement after its body; modifiers come in
  the order `optional last`. Missing required arms are reported at the end.
- An arm that uses a count such as `n` must come after the arm that defines
  it in the input.

## 9 Expressions

| Kind | Forms |
|---|---|
| Literals | `3`, `2.5`, `1e9`, `"text"`, `gecode.int_max` (2147483646), `gecode.int_min` (-2147483646) |
| Arithmetic | `+ - * / % ^` on integers only, checked for overflow; `/` truncates toward zero; `^` is power. |
| Comparison | `== != < > <= >=` on scalars. Strings support only `==` and `!=`; a string never equals a number (`"5" == 5` is 0). |
| Logic | `&&`, `\|\|` (short circuit), `!`. Results are 0 or 1. |
| Indexing | `a[i]` from 0; `a[i][j]` on rows; `rows[i].field` on record lists. |
| Functions | `size(a)` / `count(a)` (outer length), `min(...)`, `max(...)`, `sum(...)` (over all integer leaves of all arguments), `distinct(...)` (all integer leaves differ), `defined(x)` |
| Comprehensions | `all(x in a: e)`, `any(x in a: e)`, `count(x in a: e)`, `sum(x in a: e)` |

Precedence from loosest: `||`, `&&`, `== !=`, `< > <= >=`, `+ -`, `* / %`,
unary `- !`, `^` (right-associative), then `[i]` and `.f`. So `-2^2` is -4.

Truth: 0, 0.0, `""`, and arrays of length 0 are false; everything else is
true.

There is **no float arithmetic**: `x * 2` with a float `x` is an error.
Floats can be compared with numbers and negated. There are no string
operations (no length, indexing, or concatenation) and no ranges as values
(`0..n` is only a type). A comprehension binding name must differ from every
other name in the spec.

## 10 Checks

`check condition "message"` fails reading when the condition is false.
A check runs where it is written, so put it after the values it uses.
Errors while evaluating the condition (division by zero, index out of
range) are errors, not failed checks.

### Checks range over records

In a check, a name defined inside a repeat that does not enclose the check
stands for **one record's value**, and the check must hold for every
record. Fields of the same repeat are taken from the same record; fields of
nested repeats range over every inner record of every outer record.

```iris
m: 0..
repeat m as edges {
  each e
  u: int
  v: int
  w: float
}
check e.u != e.v "self-loops are not allowed"
check w > 0.0 "weights must be positive"
```

```input
2
1 2 0.5
3 3 1.5
```

```error
check failed for record 2 of 'edges': self-loops are not allowed
```

The record names are lifted **only in checks**. In `let`, `if`, sizes, and
counts, a repeat field is the whole column.

Inside a check, these positions see the **whole column** instead:
a name directly inside `size(...)` or `defined(...)`, and a name directly
after `in` in a comprehension. `count`, `sum`, `min`, `max`, and `distinct`
are *not* such positions: `check sum(demand) <= capacity` checks each
demand separately, and `check count(x) == n` fails if `x` is a scalar
field. To check a whole column, compute it with `let` first or use a
comprehension:

```iris
n: 1..
capacity: 0..
repeat n {
  demand: 0..
}
check size(demand) == n
let total_demand = sum(demand)
check total_demand <= capacity "the demand exceeds the capacity"
check sum(d in demand: d) <= capacity "the same check, written inline"
```

```input
3 10
4 5 2
```

```error
check failed: the demand exceeds the capacity
```

More rules:

- A check cannot combine fields of two different repeats (`'x' and 'y' are
  defined in different repeats`). Aggregate them with `let` first, as in
  the example above.
- A name defined both inside and outside a repeat, or in two repeats,
  cannot be lifted. Use distinct names.
- Inside a repeat, a check sees the current iteration's values.
- Checks over records of an empty repeat pass.
- A failure message names the failing record, using the `as` name when
  there is one (`record 2 of 'edges'`), and points at the record's input
  line.

## 11 Parameters and instances

`param name` or `param name = default` defines a value supplied by the
caller (`iris read --param name=value`). With a default, the caller's string
is converted to the default's type (`int`, `float`, or kept as a string);
without one, it is an integer if possible, then a float, then a string. A
parameter without a default must be supplied. Parameters appear in the data.
Unknown caller parameters are errors.

`instances { ... }` describes a file with several instances, one per pass
through the block. It must be the last statement (only `format` may
follow). Statements before it run once (a leading count of test cases can
be read there). Reading keeps one instance:

- Without a selector, the file must contain exactly one instance.
- `--param instance=2` selects the second instance (counting from 1).
- `--param instance=NAME` selects the first instance whose `name` value is
  `NAME`; the body may define `name`.
- Reading stops after the selected instance; later input is not read.

`iris list SPEC FILE` lists the instances, with their names.

```iris
instances {
  line "case {name:word}"
  n: 1..
  a[n]: int
}
```

```input instance=beta
case alpha
2 1 2
case beta
1 7
```

```data
name = "beta"
n = 1
a = [7]
```

When the host wants all test cases at once (the usual case for a solver
that processes every case), read them with `repeat` instead (§7).

## 12 Limits and work budgets

- Array sizes, repeat counts, and their products must be at most
  2147483647.
- Blocks nest at most 256 deep; expressions have a similar nesting limit.
- *Idle work* (repeat iterations and empty rows that read no input) is
  limited to about one million per read.
- *Expression work* (values visited by expressions and checks) is limited
  to 1048576 + 16 × input bytes. A quadratic check such as
  `all(x in a: count(y in a: y == x) == 1)` fails on a few thousand values
  with `the expressions do too much work for the size of the input`. Prefer
  the built-in functions: `distinct(a)` is linear.
