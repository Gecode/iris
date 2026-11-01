# Formatting reader specs

This guide proposes the style for `iris fmt`. It covers Iris 0.1 reader
specs. The formatter is planned; the rules marked R1–R14 below are
recommendations awaiting review. [Choices for review](#choices-for-review)
lists their alternatives and the evidence behind them.

The [Iris Language Specification](../spec/0.1/README.md) defines the
language. Formatting changes whitespace and line breaks while preserving
tokens, comments, strings, names, numbers, and statement order. In particular,
it preserves every byte inside a string, including a line pattern's captures,
current-value references, blanks, and escapes. It never adds parentheses or
changes a range's spelling to make a line fit.

Every Iris code block in this guide is a complete, valid reader spec.
“Before” means valid source with a different layout; “after” applies the
proposed rules. The two versions in each pair have the same meaning.

## Fixed points

The formatter has one style and one setting: the line width. It reads
`max_line_length` from `.editorconfig` for the reader spec's path, using
100 when that property is unset or `off`. It reads no other property and
has no configuration file of its own. These decisions and the starting
layout rules are settled in [PLAN.md §8.12](../PLAN.md#812-formatter-iris-fmt-in-iris-syntax):

- Put each statement on its own line.
- Indent inside braces by two spaces per level.
- Use no column alignment.
- Keep single blank lines and collapse longer runs to one.
- Keep comments in place.
- Change the style only in minor package releases and record changes in
  the changelog.

The remaining sections spell out the proposed details. A statement may
occupy several lines when it needs to wrap.

## Source, comments, and blank lines

[§5.1](../spec/0.1/05-reader-specs.md#51-reader-specs-and-parsing),
[§5.2](../spec/0.1/05-reader-specs.md#52-white-space-and-comments), and
[§5.3](../spec/0.1/05-reader-specs.md#53-strings) define source text,
whitespace, comments, and strings.

Use spaces for indentation, LF for line endings outside strings and
comments, and one final LF in a nonempty file (R1). An empty reader spec
stays empty. Remove trailing whitespace outside strings and comments.
Keep string and comment contents byte for byte, including any literal CR;
the LF that ends a comment is its delimiter, not part of its text.

Keep a standalone comment between the same surrounding tokens and indent
it at that position. Keep a trailing comment after the same token, with
one separating space (R2). Do not align, reword, wrap, or move comments to
another line. If a comment interrupts a statement, preserve that break
and indent the continuation by two spaces. A directive comment, such as
one starting with `# iris:`, follows the same rule.

Keep blank lines between the same surrounding tokens, including within a
statement. Collapse each run to one blank line. Remove leading and trailing
blank lines, and do not insert blank lines between groups of statements or
arms (R3). A comment-only file follows the same rules as any nonempty file.

Before:

```iris
# dimensions
n:int    # number of rows


repeat n { # records
 # keep this comment before the field
 x: # field type
 int
}
```

After:

```iris
# dimensions
n: int # number of rows

repeat n { # records
  # keep this comment before the field
  x: # field type
    int
}
```

## Names, strings, and numbers

Keep the spellings defined by
[§5.3, strings](../spec/0.1/05-reader-specs.md#53-strings),
[§5.4, names](../spec/0.1/05-reader-specs.md#54-names), and
[§5.5, numbers](../spec/0.1/05-reader-specs.md#55-numbers). Leading zeros,
exponent case, escapes, and the spelling of a host constant are unchanged.
Use spaces around assignment and binary operators; attach unary `-` and
`!` to their operands (R4).

Before:

```iris
let label="a\nb\tc\\d\"e" let i=007 let f=1.e2 let lo=-9223372036854775807-1
let limit=gecode.int_max
```

After:

```iris
let label = "a\nb\tc\\d\"e"
let i = 007
let f = 1.e2
let lo = -9223372036854775807 - 1
let limit = gecode.int_max
```

Token boundaries take priority over compact spacing (R5). Never glue a
field selection into a host-constant token. In this example, `gecode` is
a record name; retain one space on each side of its dot. Other field
selections have no spaces around the dot.

Before:

```iris
repeat 1 as rows {each gecode value:int} check gecode . value==rows [0] . value
```

After:

```iris
repeat 1 as rows {
  each gecode
  value: int
}
check gecode . value == rows[0].value
```

## Declarations and types

[§5.6](../spec/0.1/05-reader-specs.md#56-statements-declarations-and-blocks)
defines declaration dispatch;
[§5.7](../spec/0.1/05-reader-specs.md#57-types-ranges-and-options) defines
types, ranges, and options. Their reading behavior is defined in
[chapter 6](../spec/0.1/06-types-and-values.md).

Write `name: type`, with no space before the colon and one after it (R6).
Attach array dimensions to the name and to one another, with no spaces
inside their brackets (R5). Keep the base types `int`, `float`, `num`,
`word`, and `rest`, a named type, or an alphabet in the same position.
The discard name `_` and contextual keyword names follow the same spacing.

Before:

```iris
line :int a [2] [3] :float b:num c:word d:rest _:int
```

After:

```iris
line: int
a[2][3]: float
b: num
c: word
d: rest
_: int
```

Attach `..` and `..<` to their present bounds (R7). Keep omitted bounds
omitted. Spaces inside bound expressions follow the expression rules.
For a named type, put spaces around `=`. Leave options in source order,
including repeated alphabet options; put one space between an option
keyword and its value (R8).

Before:

```iris
type item=0 ..< 10 a :item low:.. 3 high:0 .. negative:..< 0
m [2] [2]:0 .. 9 layout "FULL_MATRIX" offset -1
g [1] [3]:alphabet "ab" base 0 blank "." strip "|" base -2
```

After:

```iris
type item = 0..<10
a: item
low: ..3
high: 0..
negative: ..<0
m[2][2]: 0..9 layout "FULL_MATRIX" offset -1
g[1][3]: alphabet "ab" base 0 blank "." strip "|" base -2
```

The [list statement](../spec/0.1/06-types-and-values.md#69-sentinel-and-end-of-line-lists)
uses the same colon and type spacing. Keep `until` and its sentinel or
`eol`, followed by any `offset`, on the line when they fit (R8).

Before:

```iris
list values : 1 .. 9 until 0 offset -1 list words:word until "END" list row:int until eol
```

After:

```iris
list values: 1..9 until 0 offset -1
list words: word until "END"
list row: int until eol
```

## Settings and format metadata

[§5.8](../spec/0.1/05-reader-specs.md#58-settings) defines settings.
Use one space between a keyword and its argument, and between consecutive
strings (R8). Keep every setting at its original position; moving it to a
header can change how input is read.

Before:

```iris
comment "#"   "//" stop "END" tokens ":" separators "," ignore   trailing
n:int comment "NOTE"
```

After:

```iris
comment "#" "//"
stop "END"
tokens ":"
separators ","
ignore trailing
n: int
comment "NOTE"
```

[§5.9](../spec/0.1/05-reader-specs.md#59-format-metadata) defines format
metadata. Put one space after `format` (R8). Preserve a quoted name or a
glued name such as `dimacs-cnf.v2` exactly. Inserting spaces into the latter
changes its tokens' role. Keep metadata after `instances` there.

Before:

```iris
format   dimacs-cnf.v2 instances {x:int} format   "last name"
```

After:

```iris
format dimacs-cnf.v2
instances {
  x: int
}
format "last name"
```

## Input statements and line patterns

[§7.3](../spec/0.1/07-structure.md#73-line-find-skip-and-expect)
defines `expect`, `line`, `find`, and `skip`. Put one space between their
tokens (R8). A
[line pattern](../spec/0.1/07-structure.md#72-line-patterns-and-captures)
is one string token: even type syntax inside captures retains its exact
source spelling.

Before:

```iris
expect   "BEGIN" "DATA" line   "v={x:int} {w} {:int} {_} {r:rest}"
find   "{$x} {next:0..9} ..." skip   line skip  2  lines
```

After:

```iris
expect "BEGIN" "DATA"
line "v={x:int} {w} {:int} {_} {r:rest}"
find "{$x} {next:0..9} ..."
skip line
skip 2 lines
```

## Blocks, repeats, record names, and index statements

[§5.6](../spec/0.1/05-reader-specs.md#56-statements-declarations-and-blocks)
defines blocks; [§7.4](../spec/0.1/07-structure.md#74-repeat-forms-and-termination)
defines repeats. Put the opening brace after the header with one space
before it. Expand every nonempty body, including a single-statement body;
put its closing brace on a line at the header's indentation (R9).
Write an empty body as `{}`. A body containing a comment is expanded.

Keep `as` and its record name together in the header. Put
[`each`](../spec/0.1/07-structure.md#76-logical-records-and-names) first in
the body, on its own line. An
[`index`](../spec/0.1/07-structure.md#77-index-statements) statement has
one space before its expression (R8).

Before:

```iris
param n=2 repeat n as rows {each row index 1 x:int}
repeat until "END" "STOP" store ending {value:word}
repeat until eof {tail:int} repeat 0 { }
```

After:

```iris
param n = 2
repeat n as rows {
  each row
  index 1
  x: int
}
repeat until "END" "STOP" store ending {
  value: word
}
repeat until eof {
  tail: int
}
repeat 0 {}
```

## Sections and arms

[§5.7](../spec/0.1/05-reader-specs.md#57-types-ranges-and-options) and
[§7.8](../spec/0.1/07-structure.md#78-sections) define sections options,
arms, and their bodies. Put each arm on its own line, indented two spaces
inside `sections`. Keep `optional` and `last` after its pattern, in their
original order. An arm without a body stays without one; do not add `{}`.
An arm with a body follows the block rules, with its statements another
two spaces inside (R10).

Before:

```iris
sections until "END" store old skip other until "STOP" store ending {
"A {a:int}" optional "B {b:int}" {x:int}
# the final arm
"DONE" optional last {}
}
```

After:

```iris
sections until "END" store old skip other until "STOP" store ending {
  "A {a:int}" optional
  "B {b:int}" {
    x: int
  }
  # the final arm
  "DONE" optional last {}
}
```

The comment stays between the same arms. Repeated or interleaved options
stay in source order. The formatter does not collect terminators or remove
repeated `skip other` options.

## Conditional execution and definitions

[§7.9](../spec/0.1/07-structure.md#79-conditional-execution-and-let)
defines `if` and `let`. Keep `} else {` and `} else if` together unless a
comment or preserved blank line separates them (R9). Expand their bodies.
Use one space around `=` in `let`,
[`param`](../spec/0.1/09-params-and-instances.md#91-caller-parameters),
and `type` statements (R4).

Before:

```iris
param n=1 param required let x=2
if n {let x=3} else if x>0 {let x=4} else {let x=5}
```

After:

```iris
param n = 1
param required
let x = 2
if n {
  let x = 3
} else if x > 0 {
  let x = 4
} else {
  let x = 5
}
```

Declaration-first dispatch also applies after `else`. Format a declaration
named `if` as a declaration and keep it after `else` (R9); introducing a
block would change the syntax tree.

Before:

```iris
if 0 { } else if [1] : int
```

After:

```iris
if 0 {} else if[1]: int
```

## Expressions and checks

[§5.10](../spec/0.1/05-reader-specs.md#510-expression-syntax-and-precedence)
defines expression syntax and precedence. Apply R4 to every binary
operator: `+`, `-`, `*`, `/`, `%`, `^`, comparisons, `&&`, and `||`.
Do not add spaces inside parentheses or between a function name and `(`.
Put one space after a comma and none before it (R5). Comprehensions use
`binding in domain: expression`, with the same colon spacing as a
declaration (R6). These rules apply recursively to nested calls,
comprehensions, indexing, and field selections.

Before:

```iris
let a=2*3^2 let b=2^3^2 let c=-2^2 let d=8/2/2 let e=7%3
let f=1<2<3 let g=1||0&&0 let h=!(1!=2)
values [2]:int let first=values [ 0 ] let total=sum (values,1)
check all(v in values:v>=0)&&any(v in values:v==first) "nonnegative values include the first"
let selected=count(v in values:v>0) let subtotal=sum(v in values:v)
```

After:

```iris
let a = 2 * 3 ^ 2
let b = 2 ^ 3 ^ 2
let c = -2 ^ 2
let d = 8 / 2 / 2
let e = 7 % 3
let f = 1 < 2 < 3
let g = 1 || 0 && 0
let h = !(1 != 2)
values[2]: int
let first = values[0]
let total = sum(values, 1)
check all(v in values: v >= 0) && any(v in values: v == first)
  "nonnegative values include the first"
let selected = count(v in values: v > 0)
let subtotal = sum(v in values: v)
```

A [check](../spec/0.1/08-expressions-and-checks.md#813-checks-and-failure-witnesses)
keeps its optional message after its expression, separated by one space
when it fits (R8). A check without a message stays without one.

Before:

```iris
x:int check x>0 "positive" check x<=gecode.int_max
```

After:

```iris
x: int
check x > 0 "positive"
check x <= gecode.int_max
```

## Instances and placement

An [instances statement](../spec/0.1/09-params-and-instances.md#93-instance-blocks-and-frames)
follows the block rules. Keep it at the top level. Formatting preserves all
[§5.11 placement restrictions](../spec/0.1/05-reader-specs.md#511-placement-restrictions):
it never moves settings into blocks, moves `each` past another statement,
or moves executable statements past `instances`.

Before:

```iris
comment "#" instances {name:word n:int repeat n {value:int}}
```

After:

```iris
comment "#"
instances {
  name: word
  n: int
  repeat n {
    value: int
  }
}
```

## Line breaking

The width includes indentation. Count Unicode scalar values, each as one
column, rather than UTF-8 bytes or terminal display cells (R11). The width
is a target: an indivisible token or a comment can exceed it. Preserve
strings, comments, glued format names, and host-constant tokens even when
they are wider than the limit.

First apply the mandatory statement, block, arm, comment, and blank-line
breaks above. Keep each remaining construct flat when it fits. Otherwise
use the following order (R12–R14). Continuations start two spaces beyond
the statement or arm's indentation, with another two spaces for each
nested broken expression. Continuations never align with an earlier token.
Existing comments and blank lines are hard breaks and take priority.

1. If a check has a message, move the message to a continuation line before
   breaking its expression. Keep an attached trailing comment after the
   message.
2. For a declaration, type, list, repeat, or sections header, move its
   option groups onto continuation lines before breaking expressions in
   its head. Break every group onto its own line: `layout expression`,
   `offset expression`, `blank string`, `strip string`, `base integer`,
   `until terminators`, `store name`, `skip other`, and `as name`.
   A list's required `until` follows this rule too. Keep keyword pairs
   such as `until eof`, `skip other`, and `ignore trailing` together.
3. If a string list still needs wrapping, break before its first string
   and put every string on a separate continuation line. This applies to
   `comment`, `stop`, `expect`, and repeat or sections terminators. For a
   single string argument to another keyword, break before that argument
   if doing so makes the line fit. Keep format-name components glued.
4. Within an expression, break at its loosest binary precedence level
   that has an operator outside parentheses. Break after every operator
   at that level, keeping the operator on the preceding line. Recursively
   break operands that still exceed the width. For a range with both
   bounds, break after `..` or `..<` before breaking within either bound.
5. If a call still needs wrapping, break after `(`, put every argument
   on its own continuation line, and put `)` on a separate line at the
   call's starting indentation. For a comprehension, first break after
   its colon; if its binding and domain still do not fit, also break
   after `(`. Put its closing `)` on a separate line. Apply the expression
   rules to the domain and body.
6. Parenthesized expressions and expressions inside brackets may break
   after their opener and before their closer in the same way. Keep
   unary operators with their operands and keep postfix names, dots,
   and simple bracket suffixes together. If an expression has no internal
   break, break before it, after its introducing keyword, `=`, or declaration
   `:`, when this makes the line fit. If none of these breaks helps, allow
   the remaining indivisible group to exceed the width.

Put an opening brace after the last header token, even in a wrapped
header. Indent its body relative to the start of the header. Trailing
comments stay on their attached line even if this exceeds the width;
when a statement wraps, the comment follows the token it originally
followed. Do not break inside a token or add a token to meet the width.

### Ranges and options at width 40

Before:

```iris
param minimum_allowed_value=0 param maximum_allowed_value=100
samples[2][2]: minimum_allowed_value..maximum_allowed_value layout "FULL_MATRIX" offset -1
```

After:

```iris
param minimum_allowed_value = 0
param maximum_allowed_value = 100
samples[2][2]: minimum_allowed_value..
  maximum_allowed_value
  layout "FULL_MATRIX"
  offset -1
```

### An expression and message at width 40

Before:

```iris
param first_value=1 param second_value=2 param third_value=3
check first_value > 0 && second_value > 0 && third_value > 0 "all values must be positive"
```

After:

```iris
param first_value = 1
param second_value = 2
param third_value = 3
check first_value > 0 &&
  second_value > 0 &&
  third_value > 0
  "all values must be positive"
```

### String lists at width 40

Before:

```iris
comment "LONG_COMMENT_PREFIX" "ANOTHER_COMMENT_PREFIX" "THIRD_COMMENT_PREFIX"
repeat until "END_OF_RECORDS" "END_OF_FILE" store ending {value:int}
```

After:

```iris
comment
  "LONG_COMMENT_PREFIX"
  "ANOTHER_COMMENT_PREFIX"
  "THIRD_COMMENT_PREFIX"
repeat
  until
    "END_OF_RECORDS"
    "END_OF_FILE"
  store ending {
  value: int
}
```

### Calls and comprehensions at width 40

Before:

```iris
param first_value=1 param second_value=2 param third_value=3
let total=sum(first_value,second_value,third_value)
values[2]:int check all(value in values:value >= first_value)
```

After:

```iris
param first_value = 1
param second_value = 2
param third_value = 3
let total = sum(
  first_value,
  second_value,
  third_value
)
values[2]: int
check all(value in values:
  value >= first_value
)
```

Line breaks do not end expressions
([§5.6](../spec/0.1/05-reader-specs.md#56-statements-declarations-and-blocks)).
The formatter uses the parsed structure to distinguish a continuation from
the next statement. Whitespace changes must preserve both tokenization and
the syntax tree. They do not affect the
[§5.12 parser limits](../spec/0.1/05-reader-specs.md#512-parser-limits).

## Two Gecode reader specs, formatted by hand

These are the complete string contents from Gecode commit
`5e7bbd5c26b0702c6fd652356b2289da0710ac3c`, with the C++ delimiters
removed. The after versions use width 100. The embedded spec formatter is
outside Iris 0.1's tooling scope; these examples show the style itself.

### SAT

Before, from `examples/sat.cpp`:

```iris
format dimacs-cnf

comment "c"
stop "%"                    # SATLIB files end with "%" and "0"
line "p cnf {vars:0..gecode.int_max} {clauses:0..}"
repeat clauses {
  list clause: -vars..vars until 0   # the terminating 0 is not stored
}
```

After:

```iris
format dimacs-cnf

comment "c"
stop "%" # SATLIB files end with "%" and "0"
line "p cnf {vars:0..gecode.int_max} {clauses:0..}"
repeat clauses {
  list clause: -vars..vars until 0 # the terminating 0 is not stored
}
```

### Nonogram

Before, from `examples/nonogram.cpp`:

```iris
format nonogram

separators ","
sections skip other {
  "width {width:1..}"
  "height {height:1..}"
  "rows" { repeat height { list row: 0.. until eol } }
  "columns" { repeat width { list col: 0.. until eol } }
}

check height <= 1000000 / width "the board must have at most a million fields"
check size(row) == height && size(col) == width "the numbers of rows and columns differ from the board size"
check count(row) <= 1 || min(row) > 0 "a 0 must be the only clue of its row"
check count(col) <= 1 || min(col) > 0 "a 0 must be the only clue of its column"
check sum(row) + count(row) - 1 <= width "the clues of a row do not fit into the width of the board"
check sum(col) + count(col) - 1 <= height "the clues of a column do not fit into the height of the board"
```

After:

```iris
format nonogram

separators ","
sections skip other {
  "width {width:1..}"
  "height {height:1..}"
  "rows" {
    repeat height {
      list row: 0.. until eol
    }
  }
  "columns" {
    repeat width {
      list col: 0.. until eol
    }
  }
}

check height <= 1000000 / width "the board must have at most a million fields"
check size(row) == height && size(col) == width
  "the numbers of rows and columns differ from the board size"
check count(row) <= 1 || min(row) > 0 "a 0 must be the only clue of its row"
check count(col) <= 1 || min(col) > 0 "a 0 must be the only clue of its column"
check sum(row) + count(row) - 1 <= width "the clues of a row do not fit into the width of the board"
check sum(col) + count(col) - 1 <= height
  "the clues of a column do not fit into the height of the board"
```

## Survey of existing examples

The primary sample is Gecode's six embedded reader specs at the commit
above: `examples/{sat,tsp,tsptw,job-shop,nonogram}.cpp` and
`examples/sudoku-instances.hh`. The secondary sample is the `--- spec`
text in all 417 `iris example` blocks across
[`spec/0.1/`](../spec/0.1/README.md). Three use byte encoding; of the
414 text examples, 353 pass the pinned reference's `-check`. The counts
below use those 353, including examples whose input later fails, and
exclude the 61 rejected text examples and three byte examples. Repeated
examples count separately. Final-newline counts honor the example's
`no-final-newline` marker.

Counts are occurrences, except where the unit says files or lines.
Punctuation inside strings and comments is excluded. In particular,
capture colons and range operators inside patterns do not count as source
punctuation. These are source-style counts, not votes: many Specification
examples deliberately compress statements or exercise unusual syntax.

| Feature and existing styles | Gecode six | Specification text examples |
|---|---:|---:|
| Colons: no space before, one after / no space on either side | 18 / 0 | 100 / 359 |
| Assignment `=`: one space on each side / no spaces | 3 / 0 | 5 / 219 |
| Binary operators other than `-`: spaces / compact / line break | 44 / 0 / 1 | 15 / 137 / 1 |
| Unary `!`: attached / spaced | 2 / 0 | 15 / 0 |
| Source range operators: attached to every present bound / spaced | 9 / 0 | 26 / 0 |
| Opening square brackets: attached, with no inner space / spaced | 7 / 0 | 228 / 0 |
| Field-selection dots: attached / spaced | 5 / 0 | 25 / 0 |
| Calls: name attached to `(` / separated | 20 / 0 | 102 / 0 |
| Opening parentheses: no following space / following space | 21 / 0 | 120 / 0 |
| Commas: no following space / one following space | 0 / 0 | 51 / 0 |
| Brace pairs: nonempty inline / nonempty multiline / empty | 8 / 15 / 0 | 165 / 12 / 24 |
| Multiline nonempty bodies: one-space / two-space indentation increment | 0 / 15 | 6 / 6 |
| `else`: same line as closing brace / next line | 1 / 0 | 10 / 0 |
| Standalone / trailing source comments | 0 / 2 | 0 / 1 |
| Blank-line runs: single / longer | 13 / 0 | 0 / 0 |
| Files with / without a final LF | 6 / 0 | 191 / 162 |
| Physical lines wider than 100 | 3 | 10 |

The eight inline Gecode bodies are four in TSP and four in nonogram.
All 15 multiline bodies use two-space indentation and put the opening
brace after the header. TSP's seven bodyless sections arms and nonogram's
two each occupy their own line. Gecode has two trailing comments, with
20 and three spaces before `#`; the Specification's one has one space.
Gecode's only wrapped expression ends its first line with `||` and
indents the continuation by eight spaces from the margin, six beyond the
statement's indentation. None of the six wraps a range, option list,
string list, function argument list, or check message separately. There
are no standalone source comments, Unicode-width examples, or comments
between sections arms in either counted sample.

For plain keyword spacing, the six have six input settings (four
`comment`, one `stop`, one `separators`), all with one space before each
argument; all six `format` declarations also use one space. There is one
`layout` option and one alphabet `blank` option, both separated by one
space; no other declaration options occur. The Specification sample has
26 settings with string arguments, three format declarations, and 47
declaration or alphabet options, all using one separating space. The
corpus supplies no example of a whitespace change that must keep `gecode`
separate from a field dot.

## Choices for review

The fixed points above are already settled. Every further layout rule in
this draft belongs to one of these recommendations. “No example” means
the survey supplies no preference, rather than evidence of agreement.

| Choice | Recommendation | Alternative | Reason and evidence |
|---|---|---|---|
| R1: source whitespace and file ending | Spaces, LF outside protected text, trim trailing whitespace, one final LF for nonempty files; empty files stay empty. | Preserve original whitespace bytes and file endings. | All six Gecode specs end in LF; the Specification has 191 with and 162 without. A predictable file ending is useful, but the latter intentionally test optional LF. Preserve literal bytes inside strings and comments. |
| R2: comment spacing and attachment | One space before trailing `#`; preserve its token attachment and line, and indent standalone comments at their position. Never reflow comment text. | Retain original spacing before `#`, or move long trailing comments above statements. | Gecode has two padded comments (20 and three spaces); the Specification has one with one space. One space follows the settled no-alignment rule. There are zero standalone or between-arm comments to settle their indentation. |
| R3: blank-line edges | Keep interior single blank lines, including inside constructs, trim blank lines at file edges, and insert none. | Preserve edge blank lines or insert blank lines between statement groups. | Gecode has 13 single blank-line runs and no longer runs; the Specification has none. Keeping grouping changes the common style least. Edge trimming has no positive example in this sample. |
| R4: operators | One space around assignment and every binary operator, including `^`; no space after unary `-` or `!`. | Compact operators, or special compact spacing for multiplication and powers. | Gecode has 3 spaced assignments and 44 spaced non-minus binary operators, with one wrapped operator; the Specification has 219 compact assignments and 137 compact non-minus binary operators. All 17 unary `!` occurrences are attached. Prefer the application specs' readable expressions over compressed syntax examples. Gecode has no power operator. |
| R5: expression and dimension punctuation | Tight brackets, dots, parentheses, and call openers; one space after commas. Keep a separating space on both sides of a field dot when gluing would change tokens. | Space inside delimiters, compact commas, or break postfix chains freely. | Gecode has 7 tight bracket openers, 5 tight field dots, and 20 tight calls, with no competing style. The Specification has 228, 25, and 102 respectively. Its 51 commas are compact; comma spacing is a deliberate readability departure. The token-boundary exception is required for meaning, with zero surveyed examples. |
| R6: colons | No space before `:`, one after, for declarations and comprehensions. | Compact colons or spaces on both sides. | Gecode uses this for all 18 source colons; the Specification has 100 spaced-after and 359 compact. Keeping Gecode's style changes common reader specs least. Capture colons remain inside unchanged strings. |
| R7: ranges | Attach `..` and `..<` to present bounds; when wrapping, break after the range operator. | Spaces around the range operator, or a break before it. | All 9 Gecode and 26 Specification source range operators attach to their bounds. Neither sample has a wrapped range; breaking after the operator follows Gecode's one binary-expression continuation. |
| R8: keyword arguments and options | One separating space; keep arguments and options flat when they fit and in source order. | Put every option or message on its own line even when short. | All 6 Gecode input settings, 6 format declarations, and its 2 declaration options use one separating space. The Specification has 26 settings with string arguments, 3 format declarations, and 47 declaration or alphabet options with the same spacing. No option or message is wrapped separately. Preserving order is required for meaning. |
| R9: braces and conditional joins | Opening brace after the header; expand nonempty blocks; compact empty `{}`; join `} else` and `} else if` unless comments or blank lines intervene. Keep an else-declaration on that line. | Keep short bodies inline, expand empty bodies, or put braces and `else` on separate lines. | Gecode has 15 multiline and 8 inline bodies. Expanding the latter makes the settled one-statement-per-line rule uniform. All 15 multiline openings follow the header; its sole `else` and all 10 in the Specification share a closing-brace line. The Specification has 24 compact empty bodies; Gecode has none. |
| R10: sections arms | One arm per line, modifiers on its pattern line, expanded body indented another two spaces. Preserve bodyless arms. | Inline short arms or put modifiers on separate lines. | All 9 Gecode bodyless arms occupy their own line; both multiline and inline arm bodies occur. The two inline nonogram arm bodies expand under R9. There are no comments between arms in the sample. |
| R11: width measurement and overflow | Count Unicode scalar values; permit overflow for protected tokens, comments, and groups with no useful permitted break. | Count UTF-8 bytes or terminal display cells; treat the width as a hard limit. | Gecode already has 3 lines over 100; the Specification has 10. Neither has a Unicode-width layout example. Scalar counting is predictable without a terminal or font; a hard limit would require rewriting some string or comment contents. |
| R12: continuation indentation and expressions | Two additional spaces per continuation level; break after every operator at the loosest available precedence before recursing. Keep fitting constructs flat. | Align continuations under an operand, use four extra spaces, or put operators at line starts. | Gecode's only wrapped expression uses a trailing `||` and six additional spaces. Two additional spaces follows the settled brace indentation without column alignment. There is only one wrapped expression in the Specification too. The exact recursive policy has no existing example. |
| R13: messages, options, and string lists | Move a check message first, then option groups, then string items, before breaking expressions; vertical lists when broken, brace after last header token. | Break expressions first, greedily pack continued list items, or move the opening brace to its own line. | Neither corpus supplies a wrapped message, option list, or string list. This keeps checks and declarations readable while retaining Gecode's compact common case. The priority and vertical layout are proposals rather than surveyed conventions. |
| R14: calls, comprehensions, and delimiters | Broken calls have one argument per line and a separate closer. Break comprehensions after `:` first. Break inside parentheses and brackets only as needed, retaining unary and simple postfix groups. Move an expression with no internal break after its introducer if that makes it fit. | Pack arguments on continuation lines or align them under the opener. | All 20 Gecode calls and 102 Specification calls attach the opener; neither corpus supplies a wrapped argument list or an expression moved after its introducer. This supplies a consistent fallback at narrow widths without introducing alignment or changing tokens. |

## Checking the examples

All before and after reader specs above were checked with the pinned
reference:

```sh
eval "$(scripts/gecode-reference.sh --print-env)"
"$GECODE_REFERENCE_INSTANCE_TOOL" -check spec.iris
```

For each complete Gecode pair, the before and after versions were also
run on the same small input and their dzn output compared byte for byte:

```sh
"$GECODE_REFERENCE_INSTANCE_TOOL" -spec before.iris -dzn input.txt > before.dzn
"$GECODE_REFERENCE_INSTANCE_TOOL" -spec after.iris -dzn input.txt > after.dzn
cmp before.dzn after.dzn
```

The SAT input was:

```text
p cnf 2 2
1 -2 0
2 0
```

The nonogram input was:

```text
width 2
height 2
rows
1
1
columns
1
1
```

Both pairs parsed and produced identical dzn bytes. These checks support
the examples; the formatter implementation still needs the same-tree,
idempotence, and conformance-result checks described in PLAN.md §8.12.
