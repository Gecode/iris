# Patterns for common input shapes

Each pattern shows a spec, a sample input, and the data `iris read` returns
(`name = value`, floats written with a decimal point). All of them pass
`iris check` and `iris fmt --check`. The language rules behind them are in
[language.md](language.md); the last section lists what Iris 0.1 cannot
express and the workaround for each.

## Contents

- [Counts, then records](#counts-then-records)
- [Arrays and matrices](#arrays-and-matrices)
- [A count of test cases](#a-count-of-test-cases)
- [Input until end of file](#input-until-end-of-file)
- [Sentinel-terminated input](#sentinel-terminated-input)
- [Rows of varying length](#rows-of-varying-length)
- [Strings and whole lines](#strings-and-whole-lines)
- [Real numbers](#real-numbers)
- [Query lines with an operation keyword](#query-lines-with-an-operation-keyword)
- [Character grids](#character-grids)
- [Compact tokens](#compact-tokens)
- [Header lines, keywords, and punctuation](#header-lines-keywords-and-punctuation)
- [KEY : VALUE headers, matrices, several instances](#key--value-headers-matrices-several-instances)
- [Checks worth adding](#checks-worth-adding)
- [What Iris 0.1 cannot express](#what-iris-01-cannot-express)

## Counts, then records

Read the counts first and bound them with ranges. Fields of a repeat become
arrays, one element per record. Use `offset -1` to turn 1-based identifiers
into 0-based ones.

```iris
n: 1..
m: 0..
repeat m as edges {
  each edge
  u: 1..n offset -1
  v: 1..n offset -1
  w: 0..
}
check edge.u != edge.v "self-loops are not allowed"
```

```input
3 2
1 2 5
2 3 7
```

```data
n = 3
m = 2
u = [0, 1]
v = [1, 2]
w = [5, 7]
```

Values are tokens, so it does not matter whether a record is on one line or
spread over several. The repeat reads exactly `m` records; extra input is an
error (`unexpected input after the end of the spec`).

## Arrays and matrices

`a[n]: int` reads `n` values; `a[n][m]: int` reads `n` rows of `m`, row by
row, regardless of line breaks.

```iris
r: 1..
c: 1..
cost[r][c]: int
weights[c]: 0..100
```

```input
2 3
1 2 3
4 5 6
10 20 30
```

```data
r = 2
c = 3
cost = [[1, 2, 3], [4, 5, 6]]
weights = [10, 20, 30]
```

Adjacency lists with a count before each row give a ragged array:

```iris
n: 1..
repeat n {
  degree: 0..
  adj[degree]: 1..n
}
```

```input
3
2 2 3
0
1 1
```

```data
n = 3
degree = [2, 0, 1]
adj = [[2, 3], [], [1]]
```

For TSPLIB matrix layouts (`UPPER_ROW`, `LOWER_DIAG_ROW`, and so on), see
[language.md §5](language.md#offsets-and-layouts).

## A count of test cases

Read the count, then repeat the per-case structure. Every field becomes an
array over the cases; array fields become rows.

```iris
t: 1..
repeat t {
  n: 0..
  k: int
  a[n]: int
}
```

```input
2
3 10
1 2 3
1 -5
9
```

```data
t = 2
n = [3, 1]
k = [10, -5]
a = [[1, 2, 3], [9]]
```

A grid per test case adds one more level:

```iris
t: 1..
repeat t {
  rows: 1..
  cols: 1..
  grid[rows][cols]: alphabet ".#" base 0
}
```

```input
2
1 2
.#
2 1
#
.
```

```data
t = 2
rows = [1, 2]
cols = [2, 1]
grid = [[[0, 1]], [[1], [0]]]
```

If the host wants one case at a time instead of all of them, use
`instances` ([language.md §11](language.md#11-parameters-and-instances)).

## Input until end of file

`repeat until eof` runs while data remains. Its body must read something.

```iris
repeat until eof {
  n: 0..
  a[n]: int
}
```

```input
2 1 2
0
3 4 5 6
```

```data
n = [2, 0, 3]
a = [[1, 2], [], [4, 5, 6]]
```

Lines of values until end of file:

```iris
repeat until eof {
  list row: int until eol
}
```

```input
1 2 3

4
5 6
```

```data
row = [[1, 2, 3], [4], [5, 6]]
```

The blank line produced no row: blank lines are always skipped.

## Sentinel-terminated input

A single list of values ended by a sentinel value (not stored):

```iris
list a: int until 0
```

```input
5 3
8 0
```

```data
a = [5, 3, 8]
```

Records until a terminating token. The token is tested before each record
and consumed; `store` keeps which one ended the repeat:

```iris
repeat until "END" "end" store terminator {
  name: word
  score: int
}
```

```input
ann 3
bob 5
END
```

```data
terminator = "END"
name = ["ann", "bob"]
score = [3, 5]
```

The comparison is on token text: `repeat until "0"` stops at `0` but not
at `00` or `-0`. Numeric list sentinels compare as numbers.

A terminator line with several tokens, such as `0 0`, works when its first
token never starts a real record. Consume the rest with `expect`:

```iris
repeat until "0" {
  n: 1..
  m: 0..
  w[n][m]: int
}
expect "0"
```

```input
2 1
5
6
1 2
7 8
0 0
```

```data
n = [2, 1]
m = [1, 2]
w = [[[5], [6]], [[7, 8]]]
```

If the first token of the terminator can also start a record (pairs ending
with `0 0`, where `0 5` is a valid pair), the repeat cannot stop on it.
When the terminator is the last thing in the file, read to the end and
check the final record; the host then drops it:

```iris
repeat until eof {
  a: int
  b: int
}
let pairs = size(a)
check pairs > 0 "the input must end with 0 0"
let last_a = a[pairs - 1]
let last_b = b[pairs - 1]
check last_a == 0 && last_b == 0 "the input must end with 0 0"
```

```input
0 5
3 4
0 0
```

```data
a = [0, 3, 0]
b = [5, 4, 0]
pairs = 3
last_a = 0
last_b = 0
```

## Rows of varying length

`list x: t until eol` reads the rest of a line. It never reads an empty
list: if the current line has nothing left, it reads the *next* line with
data. So it fails when a row can be empty, and it misreads a line whose
values started on the previous line. Prefer a count when the format has
one (see the adjacency list above), or a sentinel.

```iris
n: 1..
repeat n {
  list row: int until eol
}
check size(row) == n
```

```input
2
1 2 3
4
```

```data
n = 2
row = [[1, 2, 3], [4]]
```

Here the first `list` starts on line 2 because line 1 is used up after `n`.

## Strings and whole lines

`word` reads one token; `rest` reads the rest of a line (trimmed). `rest`
is scalar only, so read several lines with a repeat. Like `until eol`,
`rest` moves to the next line when the current one is used up.

```iris
n: 1..
names[n]: word
repeat n {
  title: rest
}
```

```input
2
alice bob
The first title
second, with commas
```

```data
n = 2
names = ["alice", "bob"]
title = ["The first title", "second, with commas"]
```

A name followed by numbers on the same line:

```iris
n: 1..
repeat n {
  line "{city:word} {x:int} {y:int}"
}
```

```input
2
Oslo 10 60
Rome 12 41
```

```data
n = 2
city = ["Oslo", "Rome"]
x = [10, 12]
y = [60, 41]
```

Compare strings with `==` and `!=` only; a string never equals a number,
so `"5" == 5` is false. There is no string length or indexing.

## Real numbers

`float` reads any decimal number (`3`, `-0.5`, `1e-3`) as a float. `num`
keeps integers as integers unless a float appears in the same array.

```iris
n: 1..
repeat n as points {
  each point
  x: float
  y: float
}
radius: float
check point.x >= 0.0 && point.y >= 0.0 "coordinates must not be negative"
check radius > 0 "the radius must be positive"
```

```input
2
0.5 1
2.25 3e2
1.5
```

```data
n = 2
x = [0.5, 2.25]
y = [1.0, 300.0]
radius = 1.5
```

Floats can be compared and negated but not used in arithmetic: `x * 2` is
an error (`'*' needs integers`). Ranges are integer-only, so bound floats
with comparisons in a check. Canonical JSON writes integral floats without
a decimal point (`1`, `3e2`); the entry's `type` says `float`.

## Query lines with an operation keyword

Read the keyword with `word`, then branch. A scalar field must be defined in
**every** iteration, so give missing arguments a default with `let`, or
store arguments as an array (an iteration without it gets an empty row).

Arguments as one array per query:

```iris
q: 1..
repeat q {
  op: word
  check op == "add" || op == "query" || op == "print" "unknown operation"
  if op == "add" {
    args[2]: int
  } else if op == "query" {
    args[1]: int
  } else {
    args[0]: int
  }
}
```

```input
3
add 1 2
print
query 4
```

```data
q = 3
op = ["add", "print", "query"]
args = [[1, 2], [], [4]]
```

Named scalar arguments with defaults, and a string argument kept in its own
array because a column cannot mix strings and numbers:

```iris
q: 1..
repeat q {
  op: word
  check op == "set" || op == "get" || op == "name" "unknown operation"
  key: int
  if op == "set" {
    value: int
  } else {
    let value = 0
  }
  if op == "name" {
    label[1]: word
  } else {
    label[0]: word
  }
}
```

```input
3
set 1 5
get 1
name 2 bob
```

```data
q = 3
op = ["set", "get", "name"]
key = [1, 1, 2]
value = [5, 0, 0]
label = [[], [], ["bob"]]
```

A command whose argument count varies, ended by a terminator token, reads
its arguments with `list ... until`; the other branches give the same name
an empty array:

```iris
repeat until eof {
  op: word
  check op == "def" || op == "calc" || op == "clear" "unknown command"
  if op == "def" {
    name[1]: word
    value[1]: int
    terms[0]: word
  } else if op == "calc" {
    name[0]: word
    value[0]: int
    list terms: word until "="
  } else {
    name[0]: word
    value[0]: int
    terms[0]: word
  }
}
```

```input
def foo 3
calc foo + bar =
clear
```

```data
op = ["def", "calc", "clear"]
name = [["foo"], [], []]
value = [[3], [], []]
terms = [[], ["foo", "+", "bar"], []]
```

Numeric operation codes work the same way with `op: 1..3` and
`if op == 1 { ... }`. Commands until end of file use `repeat until eof`.

To choose among several line forms, read the token that tells them apart
and branch on it, as above. A `line` pattern cannot select a form: it is a
single match that fails when the line does not fit. When no token tells
the forms apart (`red 10` and `10 red` in one file), read every field as
`word` and leave the typing to the host.

Avoid `list args: int until eol` for queries: a query with no arguments
makes it read the next query's line. Sections cannot read queries either,
because each arm matches at most once.

## Character grids

An alphabet maps each character to an integer: symbol `k` of the alphabet
is `k + base`, and `blank` characters are 0. Whitespace between characters
is skipped, so rows may contain spaces.

```iris
h: 1..
w: 1..
maze[h][w]: alphabet ".#SE" base 0
check sum(row in maze: count(cell in row: cell == 2)) == 1 "there must be exactly one S"
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

Sudoku-style digits with `.` for empty cells:

```iris
n: 1..
cells[n][n]: alphabet "123456789" blank "."
```

```input
3
1.3
..2
9..
```

```data
n = 3
cells = [[1, 0, 3], [0, 0, 2], [9, 0, 0]]
```

To keep rows as strings instead (for example when the dimensions are not
given), read them as words:

```iris
repeat until eof {
  row: word
}
```

```input
S.#
.#E
```

```data
row = ["S.#", ".#E"]
```

An alphabet does not see line boundaries: a short row silently takes
characters from the next line, and the error appears later or not at all.
Iris 0.1 cannot check a row's length.

The same mechanism splits tokens of a fixed length into characters, such
as playing cards written as rank and suit (`TH`, `9C`). Give each token
one row; when the character sets are disjoint, one alphabet tells them
apart by value:

```iris
n: 1..
cards[n][2]: alphabet "AKQJT987SHDC"
```

```input
3
TH
9C
AS
```

```data
n = 3
cards = [[5, 10], [6, 12], [1, 9]]
```

Here ranks are 1 to 8 and suits 9 to 12. An alphabet type is only for
two-dimensional arrays, so a single token of known length is a `[1][k]`
array. Tokens of varying length stay `word` values.

A board drawn with a frame reads the same way once the frame characters
are separators: `expect` checks each border segment, and an alphabet
splits each fixed-width cell. A repeat around the alphabet adds a
dimension.

```iris
separators "|+"
repeat 2 {
  repeat 2 {
    expect "---"
  }
  square[2][3]: alphabet ".:KQk"
}
repeat 2 {
  expect "---"
}
```

```input
+---+---+
|.k.|:::|
+---+---+
|:::|.Q.|
+---+---+
```

```data
square = [[[1, 5, 1], [2, 2, 2]], [[2, 2, 2], [1, 4, 1]]]
```

## Compact tokens

Several values packed into one token can often be split by a setting,
before reaching for `rest`. A setting applies from where it appears to the
end of the input, so check that no later value contains the byte, and
place the setting after any header it must not split.

`separators` makes punctuation act like a space:

```iris
separators "-"
repeat until eof {
  part: word
}
```

```input
Knuth-Morris-Pratt
```

```data
part = ["Knuth", "Morris", "Pratt"]
```

`tokens` makes each listed byte a token of its own, which separates
letters from the numbers between them. This splits a run of codes with no
separators and no count, such as `P01K02H03` with `tokens "PKHT"`:

```iris
tokens "AB"
repeat until eof {
  player: word
  points: 1..3
}
```

```input
A2B1A3
```

```data
player = ["A", "B", "A"]
points = [2, 1, 3]
```

Digits packed in fixed-width groups (`001004016` as codes 1, 4, 16) split
the same way: `tokens "0123456789"` makes every digit a token, and each
record reads one group. Place the setting after any header numbers.

```iris
tokens "0123456789"
repeat until eof {
  d2: 0..9
  d1: 0..9
  d0: 0..9
  let code = 100 * d2 + 10 * d1 + d0
  check code >= 1 "codes start at 001"
}
```

```input
001004016
```

```data
d2 = [0, 0, 0]
d1 = [0, 0, 1]
d0 = [1, 4, 6]
code = [1, 4, 16]
```

A separator byte is gone from the input: with `separators "-"`, `-5`
reads as `5` with no error, so this does not suit input with negative
numbers. Empty fields vanish the same way: `separators ","` reads `1,,,5`
as `1 5`. When an empty field means something, make the punctuation a
token instead and read the parts as words: with `tokens ","`, `list part:
word until eol` reads `1,,,5` as `["1", ",", ",", ",", "5"]`, keeping
every position. Tokens of a fixed length split with an alphabet
([Character grids](#character-grids)); a line pattern splits a token
whose punctuation is fixed (`{h:int}:{m:int}`).

## Header lines, keywords, and punctuation

A fixed header line, such as DIMACS `p cnf 3 2`, is a `line` pattern. Bound
the counts in the captures:

```iris
comment "c"
line "p cnf {vars:1..} {clauses:0..}"
repeat clauses {
  list clause: -vars..vars until 0
}
```

```input
c example
p cnf 3 2
1 -2 0
2 3 -1 0
```

```data
vars = 3
clauses = 2
clause = [[1, -2], [2, 3, -1]]
```

Keywords between tokens: `expect` for fixed words, `word` with a check for
choices. Punctuation: `separators ","` makes commas into spaces; `tokens
"()"` splits parentheses off as tokens of their own; a line pattern matches
punctuation literally.

```iris
n: 1..
repeat n {
  line "({x:int}, {y:int}) at {hour:0..23}:{minute:0..59}"
}
```

```input
2
(1, 2) at 09:30
(3,4) at 23:05
```

```data
n = 2
x = [1, 3]
y = [2, 4]
hour = [9, 23]
minute = [30, 5]
```

A space in a pattern accepts any amount of spacing, including none next to
punctuation. To accept trailing text, such as an inline comment, end the
pattern with `...`:

```iris
n: 1..
repeat n {
  line "{u:int} {v:int} ..."
}
```

```input
2
1 2 # first edge
3 4
```

```data
n = 2
u = [1, 3]
v = [2, 4]
```

Free text before the data: `find "pattern"` discards lines until one
matches, and `skip e lines` discards a known number of lines.

## KEY : VALUE headers, matrices, several instances

- Headers in any order, optional keys, and data sections (TSPLIB, CVRP):
  `sections`, in [language.md §8](language.md#sections) and the TSPLIB
  spec in [formats.md](formats.md#tsplib).
- Symmetric matrices stored as a triangle: `layout`, in
  [language.md §5](language.md#offsets-and-layouts).
- Several instances in one file, selected one at a time: `instances`, in
  [language.md §11](language.md#11-parameters-and-instances) and the job
  shop spec in [formats.md](formats.md#job-shop).

## Checks worth adding

Ranges catch most bad values while reading; put one on every count and
every identifier. Then add checks for what the model relies on:

```iris
n: 1..
perm[n]: 1..n
check distinct(perm) "perm must be a permutation of 1..n"
m: 0..
repeat m as jobs {
  each job
  release: 0..
  due: 0..
}
check job.release <= job.due "a job is due before its release"
let total_due = sum(due)
check total_due <= gecode.int_max "the due dates are too large for Gecode"
```

```input
3
2 3 1
2
0 5
1 4
```

```data
n = 3
perm = [2, 3, 1]
m = 2
release = [0, 1]
due = [5, 4]
total_due = 9
```

Remember that in a check a repeat field means each record
([language.md §10](language.md#10-checks)): aggregate a whole column with
`let` or a comprehension, never with `sum(field)` inside the check.

When the data feeds Gecode integer variables, bound values by
`gecode.int_max` (2147483646) and `gecode.int_min` (-2147483646).

## What Iris 0.1 cannot express

The left column names the need; § numbers point into the Specification.

| Need | Status in Iris 0.1 | Workaround |
|---|---|---|
| Blank lines as structure: a blank line separating cases, or an empty line meaning an empty list or empty string | Impossible: normal skipping passes blank lines (§4.7), and `until eol` never returns an empty list (§6.9). | Use the counts in the format. When the parts differ in width (two-word dictionary lines, then one-word message lines), read each line with `list row: word until eol` inside `repeat until eof` and tell the parts apart by `size(row)`. Otherwise read with `repeat until eof` and split in the host. |
| Comments after data on the same line | Comment prefixes match only at the start of a line (§4.7). | A `line` pattern ending in `...`; or `skip line` after the values, but only if text always follows them. |
| A multi-token terminator whose first token also starts records | A repeat stops only on one token (§7.4); no `break`. | Read until end of file and check the last record (above). |
| Stopping a repeat on a value condition | Only counts, tokens, and end of file end a repeat (§7.4). | Same as above. |
| Lines with a keyword and varying arguments | Possible, with care: no `switch`, and scalars must exist in every iteration (§7.5). | `word` plus `if`, with `let` defaults or array arguments (above). |
| Zero-argument lines read with `list ... until eol` | The list reads the next line instead (§6.9). | Branch on the keyword and read a fixed count. |
| A grid without given dimensions | Alphabets need both sizes (§6.10). | Read rows as `word` and convert in the host. |
| Checking a row's or a string's length | No string functions; alphabets ignore line boundaries (§8.8, §6.10). | None in the spec; trust the dimensions or check in the host. |
| Spaces as grid cells | Spaces are separators and are skipped (§4.2, §6.10). | None; read lines with `rest` and convert in the host (leading and trailing spaces are lost). |
| Float arithmetic in expressions and checks | All arithmetic is integer-only (§8.5). | Compare floats directly; compute in the host. |
| Bounded floats | Ranges read integers only (§6.4). | `x: float` plus a `check`. |
| Ordering or measuring strings | Strings support only `==` and `!=` (§8.6). | Check in the host. |
| Position-dependent checks: sorted arrays, `a[i] < a[i+1]` | No index ranges or loop counters in expressions (§8.10). | Check in the host. |
| A previous record's values inside a repeat | Each iteration starts with a fresh frame (§7.1). | Check after the repeat when the condition does not need positions. |
| Integers beyond 64 bits, hexadecimal, or decimal commas | Numeric tokens are decimal 64-bit integers and `.`-decimals (§4.4, §4.5). | Read as `word`. For one big integer per line, `tokens "0123456789"` and `list digit: 0..9 until eol` give an exact digit array that checks can bound. Place the setting after the count, which it would otherwise split. |
| Case-insensitive keywords | Matching is byte-exact (§7.2). | List each spelling: `repeat until "END" "end"`, or one optional sections arm per spelling. |
| Alternative names for one header key (`width N` or `columns N`) | Each sections arm has its own occurrence rule (§7.8). | One optional arm per spelling, then `let` and checks to merge them. |
| Every instance of a multi-instance file at once | `instances` keeps one instance (§9.4). | Use `repeat` over the instances. |
| An array of `rest` values | `rest` is scalar only (§6.1). | `repeat n { text: rest }` makes the column. |
