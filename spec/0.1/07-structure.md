# 7 Input structure

This chapter defines statement execution, line patterns, repeats, sections,
logical records, conditional execution, and `let`. [Chapter 5](05-reader-specs.md)
and the [formal grammar](grammar.ebnf) define syntax;
[chapter 4](04-input.md) defines scanning and input positions;
[chapter 6](06-types-and-values.md) defines reading types and value reads.
Examples use [chapter 2's transport](02-notation.md) and complete canonical
JSON documents under [chapter 3](03-data-model.md), with schemas for
[data](../schema/data.schema.json) and [errors](../schema/error.schema.json).

## 7.1 Statement execution and frames

Implementations MUST execute statements in source order along the selected
control-flow path. Ordinary blocks, if/else bodies, and sections bodies
MUST NOT introduce value frames. Each repeat iteration MUST introduce a
fresh frame. A value definition MUST define or replace its name in the
current frame. Within an iteration, an enclosing value remains available
until a local definition replaces that spelling. A previous iteration's
local values MUST NOT carry into the next iteration.

After each iteration, implementations MUST collect its final local values
as rows of columns (§7.5), then discard its frame. Looking up an enclosing
value MUST NOT supply an omitted local row. On completion, the repeat MUST
publish its columns in the enclosing frame. Column order follows possible
body definitions, as specified in §7.5, rather than the order definitions
first execute. General lookup and expression resolution belong to chapter 8,
`08-expressions-and-checks.md` (planned).

## 7.2 Line patterns and captures

Pattern construction and parsing MUST follow
[appendix A.4](appendix-a-grammar.md#a4-decoded-line-patterns),
[chapter 5](05-reader-specs.md), and [grammar.ebnf](grammar.ebnf).
Matching MUST be byte-exact, case-sensitive, deterministic, and without
backtracking. It MUST start at the remaining physical line after
[normal skipping](04-input.md#47-comment-and-stop-prefixes). The matching
text excludes LF and trailing [line blanks](04-input.md#42-white-space-and-line-blanks).
A successful selected match MUST consume through the physical line's end,
including trailing blanks, leaving LF for subsequent normal skipping.
A failed candidate MUST NOT commit captures or consume its line text;
initial skipping has already occurred.

| Form | Required effect |
|---|---|
| `{x:t}` | Convert using reading type t; define scalar x when the complete match is selected. Named types follow [chapter 6](06-types-and-values.md#65-named-types-and-visibility). |
| `{x}` | Read a word capture named x. |
| `{:int}` | Convert and validate an integer without defining an entry. Other permitted reading types can likewise be unnamed. |
| `{_}` | Convert a word and discard it. A typed `_` capture MUST also validate and discard. |
| `{$x}` | Match the current integer scalar's decimal spelling or the current string scalar's exact bytes. This is a reference, not a capture; it MUST NOT define x. |

Every capture MUST be nonempty. Numeric conversion follows
[§§4.4–4.5](04-input.md#44-integer-tokens); reading types follow
[§6.1](06-types-and-values.md#61-reading-types-and-resulting-entries).
Capture boundaries MUST instead follow these rules:

- A word ends at a line blank or at the first byte of an immediately
  following literal item. Without such a literal, NUL is also a stopping
  byte. Matching MUST NOT try a later delimiter if the remaining literal
  fails at the first occurrence.
- A rest capture consumes the remaining matching text. It cannot leave
  text for a following nonempty literal.
- Numeric captures scan an optional leading `+` or `-`, then the maximal
  run of ASCII digits for `int` and ranges. For `float` and `num`, the run
  also admits `.`, `e`, `E`, and a sign immediately after `e` or `E`.
  Complete conversion of that candidate is required. Matching MUST NOT
  shorten it to accommodate a following literal.
- `tokens` and `separators` affect initial skipping, not interior capture
  boundaries. Vertical tab and form feed MAY occur inside a word capture.
- Captures MUST NOT perform a declaration's default offset addition.
  In particular, `float` and `num` captures MUST retain negative zero.

Each run of pattern blanks MUST consume the maximal run of input line
blanks, possibly empty. A blank is required when both neighboring items
are word-like.
Captures and references are word-like. A literal is word-like on the
relevant side when its boundary byte is an ASCII letter, digit, or
underscore. Reaching the end of matching text satisfies a whitespace item,
but remaining items MUST still match. Thus `A : {n:int}` accepts `A:7`,
while `a b` rejects `ab`. Vertical tab and form feed MUST NOT satisfy pattern
whitespace. A terminal `...` MUST accept any remaining text, including no
text; its source syntax remains appendix A.4's rule.

Implementations MUST select and commit a match in this order:

1. Match literals, whitespace, references, and converted captures without
   defining captured names or checking capture ranges.
2. After selecting a complete match, apply
   [§4.10's UTF-8 checks](04-input.md#410-utf-8-validity) to word/rest
   captures, including unnamed and discarded captures.
3. Define all named, nondiscarded captures in capture order.
4. Evaluate and check capture ranges in capture order. A range MAY use
   a capture later in the pattern. Named ranges use their
   [saved bounds](06-types-and-values.md#65-named-types-and-visibility).
5. Consume the line.

Conversion failure MUST mean that the candidate does not match. A selected
capture's invalid bounds or rejected range MUST be a hard error: `find`
MUST NOT continue searching and sections MUST NOT try another arm.
References MUST use values available during matching, before any capture
in that candidate is committed. An undefined reference MUST report
`E-NAME-UNDEFINED` when reached. A float, array, or other unacceptable
reference MUST report `E-PATTERN-REFERENCE`. An empty current string MUST
NOT match, even as an empty substitution.

The next examples show typed, default word, unnamed, discarded, and rest
captures; float and num conversion; a named range; and signed zero.

```iris example
id: structure-captures
operation: read
--- spec
line "v={x:int} {w} {:int} {_} {r:rest}"
--- input
v=7 hi 8 gone tail words
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "int", "depth": 0, "value": 7},
  {"name": "w", "type": "string", "depth": 0, "value": "hi"},
  {"name": "r", "type": "string", "depth": 0, "value": "tail words"}
]}
```

```iris example
id: structure-capture-types
operation: read
--- spec
line "{a:float},{b:num},{c:word}!"
--- input
1.2345678901234567,2.5,yes!
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "float", "depth": 0, "value": 1.2345678901234567},
  {"name": "b", "type": "float", "depth": 0, "value": 2.5},
  {"name": "c", "type": "string", "depth": 0, "value": "yes"}
]}
```

```iris example
id: structure-capture-named-type
operation: read
--- spec
type t=1..3 line "{x:t}"
--- input
2
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "int", "depth": 0, "value": 2}
]}
```

```iris example
id: structure-capture-signed-zero
operation: read
--- spec
line "{x:float} {y:num}"
--- input
-0.0 -0.0
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "float", "depth": 0, "value": -0.0},
  {"name": "y", "type": "float", "depth": 0, "value": -0.0}
]}
```

References substitute existing values. The whitespace example combines
optional spacing beside punctuation, required word spacing, and `...`.
The range example uses a later capture as a bound. The `find` example
rejects an incomplete candidate before checking its captured range.

```iris example
id: structure-pattern-reference
operation: read
--- spec
let x=7 let s="hi" line "{$s}:{$x} {y:int}"
--- input
hi:7 9
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "int", "depth": 0, "value": 7},
  {"name": "s", "type": "string", "depth": 0, "value": "hi"},
  {"name": "y", "type": "int", "depth": 0, "value": 9}
]}
```

```iris example
id: structure-pattern-whitespace
operation: read
--- spec
line "A : {n:int} x {m:int} ..."
--- input
A:2 x 3more
--- expected data
{"iris": "0.1", "entries": [
  {"name": "n", "type": "int", "depth": 0, "value": 2},
  {"name": "m", "type": "int", "depth": 0, "value": 3}
]}
```

```iris example
id: structure-pattern-range-peers
operation: read
--- spec
line "{x:0..n} {n:int}"
--- input
2 3
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "int", "depth": 0, "value": 2},
  {"name": "n", "type": "int", "depth": 0, "value": 3}
]}
```

```iris example
id: structure-find-speculation
operation: read
--- spec
find "A {x:1..2} Z"
--- input
A 9 NO
A 2 Z
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "int", "depth": 0, "value": 2}
]}
```

## 7.3 Line, find, skip, and expect

`line` MUST require the rest of the current line, after normal skipping,
to match its pattern (§7.2). If that line is exhausted, normal skipping
seeks subsequent data, passing blank and comment lines. A final LF is
not required. Mismatch or absent input MUST report `E-LINE-MATCH`.
The examples show a current-line remainder, a subsequent line, and input
without a final LF.

```iris example
id: structure-line-current-and-next
operation: read
--- spec
n:int line "X {x:int}" line "Y {y:int}"
--- input
1 X 2

Y 3
--- expected data
{"iris": "0.1", "entries": [
  {"name": "n", "type": "int", "depth": 0, "value": 1},
  {"name": "x", "type": "int", "depth": 0, "value": 2},
  {"name": "y", "type": "int", "depth": 0, "value": 3}
]}
```

```iris example
id: structure-line-final-lf
operation: read
--- spec
line "A {x:int}"
--- input no-final-newline
A 7
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "int", "depth": 0, "value": 7}
]}
```

`find` MUST try the current remaining line first, then discard each
unsuccessful candidate line until one matches. It MUST consume the matching
line too. If no remaining line matches, it MUST report `E-FIND-MATCH`.
Reference errors and committed-capture errors remain hard errors (§7.2).

```iris example
id: structure-find
operation: read
--- spec
n:int find "X {x:int}" y:int
--- input
1 noise
no
X 2
3
--- expected data
{"iris": "0.1", "entries": [
  {"name": "n", "type": "int", "depth": 0, "value": 1},
  {"name": "x", "type": "int", "depth": 0, "value": 2},
  {"name": "y", "type": "int", "depth": 0, "value": 3}
]}
```

`skip line` MUST act as `skip 1 lines`. `skip e lines` MUST evaluate its
count once as an integer scalar in 0..2147483647, otherwise reporting
`E-COUNT`. Each requested skip MUST first perform normal skipping, then
discard the remaining physical line. Blank and comment lines passed during
normal skipping MUST NOT count separately; a partially consumed line MUST
count. Zero skips MUST consume nothing. Missing requested data MUST report
`E-SKIP-EOF`. These examples show both skip forms, comments and blank lines,
a partially consumed line, and zero skips.

```iris example
id: structure-skip
operation: read
--- spec
comment "#" n:int skip line skip 2 lines y:int
--- input
1 tail

# comment
one
two
7
--- expected data
{"iris": "0.1", "entries": [
  {"name": "n", "type": "int", "depth": 0, "value": 1},
  {"name": "y", "type": "int", "depth": 0, "value": 7}
]}
```

```iris example
id: structure-skip-zero
operation: read
--- spec
skip 0 lines n:int
--- input
7
--- expected data
{"iris": "0.1", "entries": [
  {"name": "n", "type": "int", "depth": 0, "value": 7}
]}
```

`expect` MUST consume its literal strings sequentially as exact ordinary
tokens, with normal skipping before each. Its strings are a sequence;
repeat and sections terminating strings are alternatives. Token and
separator settings MUST apply. Mismatch MUST report `E-EXPECTED-TOKEN` at
the offending token's beginning; absent input MUST report the current EOF
position. `expect` MUST NOT create entries. Here `tokens` makes the colon
an ordinary token between the two words.

```iris example
id: structure-expect
operation: read
--- spec
tokens ":" expect "A" ":" "B" x:int
--- input
A:B
7
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "int", "depth": 0, "value": 7}
]}
```

## 7.4 Repeat forms and termination

A counted repeat MUST evaluate its count once, before the first iteration,
as an integer scalar in 0..2147483647; otherwise it MUST report `E-COUNT`.
Reassigning the count's name in the body MUST NOT change the admitted
count. Zero MUST execute no body but MUST still construct empty columns
for its possible definitions (§7.5). The counted example redefines n in
each iteration and shows that only its final local value is collected.
The zero example shows declaration, array, list, and `let` fallbacks.

```iris example
id: structure-repeat-count
operation: read
--- spec
let n=2 repeat n {index 1 x:int let n=1} y:int
--- input
1 7 2 8 9
--- expected data
{"iris": "0.1", "entries": [
  {"name": "n", "type": "int", "depth": 1, "value": [1, 1]},
  {"name": "x", "type": "int", "depth": 1, "value": [7, 8]},
  {"name": "y", "type": "int", "depth": 0, "value": 9}
]}
```

```iris example
id: structure-repeat-zero
operation: read
--- spec
repeat 0 {x:int a[2]:float list b:word until eol let c="s"}
--- input
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "int", "depth": 1, "value": []},
  {"name": "a", "type": "float", "depth": 2, "value": []},
  {"name": "b", "type": "string", "depth": 2, "value": []},
  {"name": "c", "type": "int", "depth": 1, "value": []}
]}
```

`repeat until` with strings MUST test the next complete ordinary token
before each iteration, after normal skipping. Any listed exact token MUST
terminate the repeat and be consumed. The strings are alternatives. A
terminator encountered during body execution has no special status; the
body MAY read it as data. EOF before a required terminator MUST report
`E-REPEAT-TERMINATOR`.

`store` MUST define the matched token as a string scalar in the enclosing
frame, outside this repeat's iteration frames. It MUST store neither the
line nor an iteration count, and MUST define its name only when a token
matches. That stored value MAY become a column of an enclosing repeat.
`store _` defines an entry named `_` at top level; §7.5 excludes `_` from
an enclosing repeat's discovered columns, so such an iteration-local entry
is discarded with its frame.

These examples show alternative terminators and store, immediate
termination with empty columns, and a terminator read within a body.

```iris example
id: structure-repeat-until-store
operation: read
--- spec
repeat until "END" "STOP" store ending {x:int} tail:int
--- input
1 2 STOP 9
--- expected data
{"iris": "0.1", "entries": [
  {"name": "ending", "type": "string", "depth": 0, "value": "STOP"},
  {"name": "x", "type": "int", "depth": 1, "value": [1, 2]},
  {"name": "tail", "type": "int", "depth": 0, "value": 9}
]}
```

```iris example
id: structure-repeat-immediate
operation: read
--- spec
repeat until "END" store ending {x:int}
--- input
END
--- expected data
{"iris": "0.1", "entries": [
  {"name": "ending", "type": "string", "depth": 0, "value": "END"},
  {"name": "x", "type": "int", "depth": 1, "value": []}
]}
```

```iris example
id: structure-repeat-body-terminator
operation: read
--- spec
repeat until "END" {a:word b:word}
--- input
x END END
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "string", "depth": 1, "value": ["x"]},
  {"name": "b", "type": "string", "depth": 1, "value": ["END"]}
]}
```

`repeat until eof` MUST perform normal skipping before each iteration and
end when no data remains, including after applicable comment/stop handling.

```iris example
id: structure-repeat-eof
operation: read
--- spec
repeat until eof {x:int}
--- input
1 2
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "int", "depth": 1, "value": [1, 2]}
]}
```

An indefinite repeat body MUST advance the input cursor on each iteration.
Otherwise it MUST report `E-NO-PROGRESS` before column collection. Counted
repeats MAY consume no input, subject to chapter 10, `10-work-budgets.md`
(planned). Nested repeats MUST apply these rules independently and add
array levels to their field columns.

## 7.5 Column construction and missing rows

Before executing a repeat, implementations MUST discover its possible
field names by traversing its body in source order and retaining the first
occurrence of each nondiscarded name. They MUST traverse both if branches,
all sections arms, and nested repeat bodies even when execution would not
reach them. In sections, visit its store name first, then each arm's
captures before its body definitions. A nested repeat contributes its
store name, then its body's fields with one additional depth. Discovery
MUST exclude type definitions, record aliases, `_`, and implementation
bookkeeping.

The first syntactic occurrence MUST supply the fallback metadata below.
A first actual local value MUST instead determine the actual local depth
and initial value type.

| Definition | Fallback value type and depth before this repeat's extra level |
|---|---|
| Declaration | Reading type's value type, with num represented as int; number of dimensions |
| List | Reading type's value type, with num represented as int; depth 1 |
| Capture | Reading type's value type, with num represented as int; depth 0 |
| `let` or `param` | int; depth 0 |
| Terminator store | string; depth 0 |
| Nested repeat field | Recursively determined fallback, plus one depth |

Each iteration MUST contribute its final local value for each discovered
name, or a missing row. A scalar becomes one element of a depth-1 column.
A depth-d local array becomes one row of a depth-(d+1) column. Implementations
MUST preserve iteration order as row order and element order within each
local value. Rows MAY have different lengths.

```iris example
id: structure-repeat-ragged
operation: read
--- spec
repeat 2 {n:int a[n]:int}
--- input
2 4 5 1 6
--- expected data
{"iris": "0.1", "entries": [
  {"name": "n", "type": "int", "depth": 1, "value": [2, 1]},
  {"name": "a", "type": "int", "depth": 2, "value": [[4, 5], [6]]}
]}
```

A missing array-valued row MUST become an empty outer row at the established
local depth. Implementations MUST NOT invent lower rows or scalar leaves.
When no actual row exists, the fallback determines depth and type. These
examples show missing rows before and after an actual array, and entirely
missing depth-2 local arrays.

```iris example
id: structure-repeat-missing-rows
operation: read
--- spec
repeat 3 {k:int if k {list a:int until eol}}
--- input
0
1 4 5
0
--- expected data
{"iris": "0.1", "entries": [
  {"name": "k", "type": "int", "depth": 1, "value": [0, 1, 0]},
  {"name": "a", "type": "int", "depth": 2, "value": [[], [4, 5], []]}
]}
```

```iris example
id: structure-repeat-all-missing-rows
operation: read
--- spec
repeat 2 {k:int if k {a[2][3]:float}}
--- input
0 0
--- expected data
{"iris": "0.1", "entries": [
  {"name": "k", "type": "int", "depth": 1, "value": [0, 0]},
  {"name": "a", "type": "float", "depth": 3, "value": [[], []]}
]}
```

A scalar MUST NOT be missing in a nonempty repeat. If a missing row precedes
the first actual scalar, implementations MUST fail when that scalar arrives.
If a missing row follows an actual scalar, they MUST fail then. If no local
value is ever produced and the fallback is scalar, they MUST fail when
finishing the column. All three failures MUST report `E-REPEAT-MISSING`.
Looking up an enclosing scalar does not supply a local row (§7.1).

A zero-iteration repeat MUST produce an empty column at fallback depth plus
one, even for scalar fallbacks. An entirely unexecuted repeat outside
another repeat MUST create nothing. All actual local rows of a column
MUST have the same depth, including empty arrays; otherwise implementations
MUST report `E-REPEAT-DEPTH`.

Nonempty scalar leaves MUST determine type consistency. Integer and float
leaves MUST combine by numeric promotion using
[§6.2's binary64 conversion](06-types-and-values.md#62-scalar-reading-and-numeric-promotion).
Mixing nonempty string and numeric leaves MUST report `E-VALUE-TYPE-MIX`.
Empty arrays alone MUST NOT force promotion or introduce a string/number
conflict. If all actual values are empty, the first actual value's type
MUST be retained. If no actual value exists, the fallback MUST be used.

```iris example
id: structure-repeat-promotion
operation: read
--- spec
repeat 2 {x:num}
--- input
1 2.5
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "float", "depth": 1, "value": [1, 2.5]}
]}
```

```iris example
id: structure-repeat-empty-type
operation: read
--- spec
repeat 2 {k:int if k {a[0]:word} else {a[1]:int}}
--- input
1 0 7
--- expected data
{"iris": "0.1", "entries": [
  {"name": "k", "type": "int", "depth": 1, "value": [1, 0]},
  {"name": "a", "type": "int", "depth": 2, "value": [[], [7]]}
]}
```

Columns MUST be published in discovered name order. Replacing an enclosing
entry MUST retain its existing position under
[§3.1](03-data-model.md#31-entries-and-definition-order). A terminating
store executes before column publication, so a new store entry can precede
the columns. Outside repeat collection, sections definitions follow actual
arm execution order (§7.8). Here the unexecuted array definition places z
before a and supplies its missing-row fallback.

```iris example
id: structure-repeat-order
operation: read
--- spec
repeat 1 {if 0 {z[1]:word} a:int}
--- input
7
--- expected data
{"iris": "0.1", "entries": [
  {"name": "z", "type": "string", "depth": 2, "value": [[]]},
  {"name": "a", "type": "int", "depth": 1, "value": [7]}
]}
```

## 7.6 Logical records and names

Each repeat iteration MUST form one logical record. Its ordinary fields
MUST be represented by the corresponding column entries (§7.5). Nested
repeats contribute lists of nested records and deeper field columns.
Implementations MUST retain each record's start position for later check
diagnostics independently of its field values. This MUST be the position
normal skipping would reach at iteration entry, including EOF when no data
remains. Saving it MUST NOT itself advance the input cursor.

`repeat ... as rows { each row ... }` names the list of logical records
`rows` and each individual record `row`. Either name MAY be supplied
independently. `each` MUST consume no input; it is a structural naming
statement whose placement follows
[§5.11](05-reader-specs.md#511-placement-restrictions).

A record or record-list denotation is not a value and MUST NOT itself
create a data entry. Explicit `as` and `each` names MUST NOT appear as
entries. Ordinary fields still appear as their column entries; a field
name's additional role as a record name in a check MUST NOT create another
entry. The examples show both names, a list name alone, and an individual
record name alone. The first also computes a field in the current record.

```iris example
id: structure-record-names
operation: read
--- spec
repeat 2 as rows {each row a:int let b=row.a+1}
--- input
4 5
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 1, "value": [4, 5]},
  {"name": "b", "type": "int", "depth": 1, "value": [5, 6]}
]}
```

```iris example
id: structure-record-list-only
operation: read
--- spec
repeat 1 as rows {a:int}
--- input
7
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 1, "value": [7]}
]}
```

```iris example
id: structure-record-each-only
operation: read
--- spec
repeat 1 {each row a:int}
--- input
7
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 1, "value": [7]}
]}
```

In the nested example, each job has n and a steps list; each step has x.
The data contains n at depth 1 and x at depth 2. It contains no objects or
entries named jobs, job, steps, or step. Chapter 8,
`08-expressions-and-checks.md` (planned), defines field selection, checks,
record quantification, comprehensions, name conflicts, and scope rules.

```iris example
id: structure-nested-records
operation: read
--- spec
repeat 2 as jobs {each job n:int repeat n as steps {each step x:int}}
--- input
2 4 5 1 6
--- expected data
{"iris": "0.1", "entries": [
  {"name": "n", "type": "int", "depth": 1, "value": [2, 1]},
  {"name": "x", "type": "int", "depth": 2, "value": [[4, 5], [6]]}
]}
```

## 7.7 Index statements

`index b` MUST read an integer token equal to the innermost active repeat's
iteration number counted from zero plus b. It MUST evaluate b on each
execution, before reading the token, as an integer scalar in
0..2147483647. It MUST consume its token and MUST NOT produce an entry.
It imposes no physical row or line boundary. The nested example uses the
innermost repeat's index; the second example reevaluates b each time.

```iris example
id: structure-index-nested
operation: read
--- spec
repeat 2 {index 1 repeat 2 {index 0 x:int}}
--- input
1 0 4 1 5 2 0 6 1 7
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "int", "depth": 2, "value": [[4, 5], [6, 7]]}
]}
```

```iris example
id: structure-index-base
operation: read
--- spec
repeat 2 {b:int index b x:int}
--- input
1 1 7 5 6 8
--- expected data
{"iris": "0.1", "entries": [
  {"name": "b", "type": "int", "depth": 1, "value": [1, 5]},
  {"name": "x", "type": "int", "depth": 1, "value": [7, 8]}
]}
```

Executing `index` without an active repeat MUST report `E-INDEX-CONTEXT`
before evaluating b. Invalid b MUST report `E-COUNT`. Failed integer
conversion MUST report `E-EXPECTED-VALUE`. A different integer MUST report
`E-INDEX-MISMATCH` at the token's beginning.

## 7.8 Sections

Sections MUST process input with the following loop and completion rules:

1. Perform normal skipping. EOF ends sections, even when `until` was supplied.
2. Test terminating tokens before patterns. On a match, consume only the
   terminating token, store it when requested, and end sections. A same-line
   suffix remains available.
3. Try arms in declaration order and select the first complete match.
   Failed candidates MUST NOT commit captures. A reached current-value
   reference's error MUST remain a hard error.
4. If no arm matches, discard the remaining line when `skip other` is
   enabled; otherwise report `E-SECTIONS-UNEXPECTED`.
5. Reject a second match of the selected arm with `E-SECTIONS-REPEATED`.
   An optional arm is still permitted at most once. In ordinary reading,
   this failure MUST precede committing its second captures or executing
   its second body.
6. Commit captures and consume the matching line under §7.2, then execute
   that arm's body in the current frame. The body MAY read subsequent input.
7. A `last` arm ends sections after its body. It MUST NOT make that arm
   optional or waive other required arms.
8. On EOF, a terminating token, or a `last` arm, check unseen required arms
   in declaration order. A missing arm MUST report `E-LINE-MATCH`, shared
   with `line`; there is no separate missing-sections code.

An ordinary arm MUST match exactly once; `optional` permits zero or one
match. Arms MUST execute in input order. Sections MUST produce only its
captures, body definitions, and an actually matched terminator store.
EOF and `last` MUST NOT populate store. An unset store inside a repeat
can therefore cause a missing scalar (§7.5). Repeated options MUST follow
[§5.7](05-reader-specs.md#57-types-ranges-and-options).

The first example combines alternative terminators, store, `skip other`,
an optional arm, and an optional last arm. B executes before A. The second
executes a required last arm. The third ends at EOF despite an until/store
option and creates no ending entry.

```iris example
id: structure-sections-options
operation: read
--- spec
sections until "END" "STOP" store ending skip other {"A {a:int}" "B {b:int}" optional "DONE" optional last} tail:int
--- input
noise
B 2
A 1
STOP 7
--- expected data
{"iris": "0.1", "entries": [
  {"name": "b", "type": "int", "depth": 0, "value": 2},
  {"name": "a", "type": "int", "depth": 0, "value": 1},
  {"name": "ending", "type": "string", "depth": 0, "value": "STOP"},
  {"name": "tail", "type": "int", "depth": 0, "value": 7}
]}
```

```iris example
id: structure-sections-last
operation: read
--- spec
sections {"A {a:int}" "DONE" last} tail:int
--- input
A 1
DONE
7
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 0, "value": 1},
  {"name": "tail", "type": "int", "depth": 0, "value": 7}
]}
```

```iris example
id: structure-sections-eof
operation: read
--- spec
sections until "END" store ending {"A {a:int}"}
--- input
A 1
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 0, "value": 1}
]}
```

These examples show declaration-order selection, an unsuccessful arm that
commits no captures, and an arm body reading values after its matching line.

```iris example
id: structure-sections-first-arm
operation: read
--- spec
sections {"A {x:int}" "A {y:int}" optional}
--- input
A 7
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "int", "depth": 0, "value": 7}
]}
```

```iris example
id: structure-sections-speculation
operation: read
--- spec
sections {"A {x:int} BAD" optional "A {y:int}"}
--- input
A 7
--- expected data
{"iris": "0.1", "entries": [
  {"name": "y", "type": "int", "depth": 0, "value": 7}
]}
```

```iris example
id: structure-sections-body
operation: read
--- spec
sections {"N {n:int}" "DATA" {a[n]:int}}
--- input
N 2
DATA
4 5
--- expected data
{"iris": "0.1", "entries": [
  {"name": "n", "type": "int", "depth": 0, "value": 2},
  {"name": "a", "type": "int", "depth": 1, "value": [4, 5]}
]}
```

A terminating token takes priority over an arm and leaves its same-line
suffix for the next statement. Repeated options accumulate terminators and
use the last store name under §5.7, as the second example shows.

```iris example
id: structure-sections-token-priority
operation: read
--- spec
sections until "END" store ending {"END" optional} tail:int
--- input
END 7
--- expected data
{"iris": "0.1", "entries": [
  {"name": "ending", "type": "string", "depth": 0, "value": "END"},
  {"name": "tail", "type": "int", "depth": 0, "value": 7}
]}
```

```iris example
id: structure-sections-repeated-options
operation: read
--- spec
sections until "X" store old skip other until "Y" store ending {"A" optional}
--- input
X
--- expected data
{"iris": "0.1", "entries": [
  {"name": "ending", "type": "string", "depth": 0, "value": "X"}
]}
```

A missing-arm diagnostic MUST locate the terminating token's beginning,
the last arm's matching-line beginning, or EOF. It MUST NOT move to the
position after the last arm's body. It MUST retain the active reader-spec
line: normally the sections statement's line, but when ending through a
last arm with an executed body, the body's last executed statement line.
Deferred instance rejection and its EOF exception are defined only by
[§11.2.1](errors.md#1121-deferred-instance-failures-and-catalog) and chapter 9,
`09-params-and-instances.md` (planned).

## 7.9 Conditional execution and let

An `if` MUST evaluate its condition once and execute exactly the selected
body. With a false condition and no `else`, it MUST execute no body.
Else-if is nested conditional execution. Branches MUST NOT introduce
a frame. Unselected bodies MUST NOT read input or define values, although
repeat field discovery MUST still visit their possible definitions (§7.5).
Chapter 8, `08-expressions-and-checks.md` (planned), defines truth conversion
and expression errors. This example shows if, else-if, else, `let`, and
replacement without changing entry order.

```iris example
id: structure-if-and-let
operation: read
--- spec
let x=1 if 0 {let x=9} else if 1 {let y="yes"} else {let y="no"} let x=2
--- input
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "int", "depth": 0, "value": 2},
  {"name": "y", "type": "string", "depth": 0, "value": "yes"}
]}
```

A `let` MUST evaluate its expression at that statement and define or replace
its name in the current frame without reading input. It MAY retain a scalar
or an array result. It MUST NOT automatically range over records; chapter 8
defines expression and record-name interpretation. Replacement MUST retain
entry position. In a repeat, its final local result MUST be collected under
§7.5. The example retains an array result.

```iris example
id: structure-let-array
operation: read
--- spec
a[2]:int let b=a
--- input
4 5
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 1, "value": [4, 5]},
  {"name": "b", "type": "int", "depth": 1, "value": [4, 5]}
]}
```

## 7.10 Errors and execution context

Implementations MUST use the [error registry](errors.md) and its
[location profiles](errors.md#1132-location-profiles), without introducing
statement-specific variants. Every read-time error below uses profile R:
input origin, line and column; active reader-spec origin and line with
null column; current context when present; and null record_path.
Shared errors retain the definitions in their owning chapters.

| Code | Structural use; primary input position |
|---|---|
| `E-LINE-MATCH` | Failed line or missing required sections arm; candidate line start, terminator start, last matching-line start, or EOF (§§7.3, 7.8). |
| `E-FIND-MATCH` | No remaining matching line; EOF (§7.3). |
| `E-SKIP-EOF` | No data for a requested skip; EOF (§7.3). |
| `E-EXPECTED-TOKEN` | Failed expect; offending token start or EOF (§7.3). |
| `E-COUNT` | Invalid repeat count, line count, or index base; current cursor when evaluated (§§7.3–7.4, 7.7). |
| `E-INDEX-CONTEXT` | No active repeat; current cursor (§7.7). |
| `E-INDEX-MISMATCH` | Wrong index; token start (§7.7). |
| `E-PATTERN-REFERENCE`, `E-NAME-UNDEFINED` | Reached unacceptable or undefined reference; candidate line start (§7.2). |
| `E-EXPECTED-VALUE` | Selected capture outside its range, or failed index conversion; capture/token start (§§7.2, 7.7). Speculative conversion failure is only a mismatch. |
| `E-RANGE-BOUNDS` | Invalid selected capture bounds; candidate line start (§7.2). |
| `E-REPEAT-TERMINATOR` | EOF without a required terminating token (§7.4). |
| `E-NO-PROGRESS` | No cursor advance by an indefinite body; current cursor, before collection (§7.4). |
| `E-REPEAT-MISSING` | Missing scalar; cursor at collection or column completion (§7.5). |
| `E-REPEAT-DEPTH`, `E-VALUE-TYPE-MIX` | Inconsistent column rows; cursor at collection (§7.5). |
| `E-SECTIONS-REPEATED`, `E-SECTIONS-UNEXPECTED` | Repeated selected arm or unhandled line; candidate line start (§7.8). |

Malformed patterns and forbidden alphabet captures MUST report
`E-SPEC-PATTERN` with profile P under
[appendix A.4](appendix-a-grammar.md#a4-decoded-line-patterns).
[Iris-only E-UTF8](04-input.md#410-utf-8-validity) applies to selected
captures. Chapter 10, `10-work-budgets.md` (planned), defines `E-BUDGET`.

Execution context MUST retain outer-to-inner text: finite repeats use
`repeat [r of n]`, indefinite repeats use `repeat [r]`, sections arm bodies
use `section "PATTERN"`, and active value reads add their names.
Human-facing repeat numbers start at 1. Explicit `as` and `each` names
MUST NOT replace the literal repeat label. Capture commitment occurs before
entering an arm-body context. Collection failures retain the active
iteration context; final all-missing failures occur after that context is
removed. Nested statements MUST retain their own reader-spec lines.

Execution context MUST NOT be converted to a check witness path.
[§3.7](03-data-model.md#37-canonical-error-document) defines record_path;
ordinary structural errors have null paths. Chapter 8,
`08-expressions-and-checks.md` (planned), defines check witnesses, which can
omit surrounding active iteration indices.

The following failures cover the chapter's error codes, missing scalar
timing, and the distinction between a failed pattern and a committed range.
Each expected part is a complete error document.

```iris example
id: structure-line-mismatch
operation: read
--- spec
line "A"
--- input
B
--- expected error
{
  "code": "E-LINE-MATCH",
  "message": "expected a line matching \"A\"",
  "input_location": {
    "origin": "input.bin",
    "line": 1,
    "column": 1
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": null,
  "record_path": null
}
```

```iris example
id: structure-line-eof
operation: read
--- spec
line "A"
--- input
--- expected error
{
  "code": "E-LINE-MATCH",
  "message": "expected a line matching \"A\", found end of input",
  "input_location": {
    "origin": "input.bin",
    "line": 1,
    "column": 1
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": null,
  "record_path": null
}
```

```iris example
id: structure-find-eof
operation: read
--- spec
find "A"
--- input
B
--- expected error
{
  "code": "E-FIND-MATCH",
  "message": "no line matching \"A\" found",
  "input_location": {
    "origin": "input.bin",
    "line": 2,
    "column": 1
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": null,
  "record_path": null
}
```

```iris example
id: structure-skip-eof
operation: read
--- spec
skip 2 lines
--- input
a
--- expected error
{
  "code": "E-SKIP-EOF",
  "message": "cannot skip a line at end of input",
  "input_location": {
    "origin": "input.bin",
    "line": 2,
    "column": 1
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": null,
  "record_path": null
}
```

```iris example
id: structure-skip-count
operation: read
--- spec
skip -1 lines
--- input
--- expected error
{
  "code": "E-COUNT",
  "message": "number of lines is negative (-1)",
  "input_location": {
    "origin": "input.bin",
    "line": 1,
    "column": 1
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": null,
  "record_path": null
}
```

```iris example
id: structure-expect-token
operation: read
--- spec
expect "A" "B"
--- input
A C
--- expected error
{
  "code": "E-EXPECTED-TOKEN",
  "message": "expected 'B', found 'C'",
  "input_location": {
    "origin": "input.bin",
    "line": 1,
    "column": 3
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": null,
  "record_path": null
}
```

```iris example
id: structure-expect-eof
operation: read
--- spec
expect "A"
--- input
--- expected error
{
  "code": "E-EXPECTED-TOKEN",
  "message": "expected 'A', found end of input",
  "input_location": {
    "origin": "input.bin",
    "line": 1,
    "column": 1
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": null,
  "record_path": null
}
```

```iris example
id: structure-repeat-count-type
operation: read
--- spec
repeat 1.0 {}
--- input
--- expected error
{
  "code": "E-COUNT",
  "message": "repeat count must be an integer",
  "input_location": {
    "origin": "input.bin",
    "line": 1,
    "column": 1
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": null,
  "record_path": null
}
```

```iris example
id: structure-repeat-count-negative
operation: read
--- spec
repeat -1 {}
--- input
--- expected error
{
  "code": "E-COUNT",
  "message": "repeat count is negative (-1)",
  "input_location": {
    "origin": "input.bin",
    "line": 1,
    "column": 1
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": null,
  "record_path": null
}
```

```iris example
id: structure-repeat-count-large
operation: read
--- spec
repeat 2147483648 {}
--- input
--- expected error
{
  "code": "E-COUNT",
  "message": "repeat count is too large (2147483648)",
  "input_location": {
    "origin": "input.bin",
    "line": 1,
    "column": 1
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": null,
  "record_path": null
}
```

```iris example
id: structure-index-context
operation: read
--- spec
index 1
--- input
--- expected error
{
  "code": "E-INDEX-CONTEXT",
  "message": "'index' must be inside a repeat",
  "input_location": {
    "origin": "input.bin",
    "line": 1,
    "column": 1
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": null,
  "record_path": null
}
```

```iris example
id: structure-index-base-negative
operation: read
--- spec
repeat 1 {index -1}
--- input
0
--- expected error
{
  "code": "E-COUNT",
  "message": "index base is negative (-1)",
  "input_location": {
    "origin": "input.bin",
    "line": 1,
    "column": 1
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": "repeat [1 of 1]",
  "record_path": null
}
```

```iris example
id: structure-index-mismatch
operation: read
--- spec
repeat 2 {index 1 x:int}
--- input
1 5
3 6
--- expected error
{
  "code": "E-INDEX-MISMATCH",
  "message": "expected index 2, found 3",
  "input_location": {
    "origin": "input.bin",
    "line": 2,
    "column": 1
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": "repeat [2 of 2]",
  "record_path": null
}
```

```iris example
id: structure-index-value
operation: read
--- spec
repeat 1 {index 1}
--- input
bad
--- expected error
{
  "code": "E-EXPECTED-VALUE",
  "message": "expected int, found 'bad'",
  "input_location": {
    "origin": "input.bin",
    "line": 1,
    "column": 1
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": "repeat [1 of 1]",
  "record_path": null
}
```

```iris example
id: structure-pattern-ref-type
operation: read
--- spec
let x=1.0 line "{$x}"
--- input
1
--- expected error
{
  "code": "E-PATTERN-REFERENCE",
  "message": "'x' must be a string or an integer",
  "input_location": {
    "origin": "input.bin",
    "line": 1,
    "column": 1
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": null,
  "record_path": null
}
```

```iris example
id: structure-pattern-ref-undefined
operation: read
--- spec
line "{$x}"
--- input
x
--- expected error
{
  "code": "E-NAME-UNDEFINED",
  "message": "'x' is not defined yet",
  "input_location": {
    "origin": "input.bin",
    "line": 1,
    "column": 1
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": null,
  "record_path": null
}
```

```iris example
id: structure-pattern-range-failure
operation: read
--- spec
line "{x:0..n} {n:int}"
--- input
4 3
--- expected error
{
  "code": "E-EXPECTED-VALUE",
  "message": "expected an int in 0..3, found '4'",
  "input_location": {
    "origin": "input.bin",
    "line": 1,
    "column": 1
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": null,
  "record_path": null
}
```

```iris example
id: structure-pattern-range-bounds
operation: read
--- spec
line "{x:0..1.0}"
--- input
1
--- expected error
{
  "code": "E-RANGE-BOUNDS",
  "message": "the bounds of a range must be integers",
  "input_location": {
    "origin": "input.bin",
    "line": 1,
    "column": 1
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": null,
  "record_path": null
}
```

```iris example
id: structure-repeat-missing-end
operation: read
--- spec
repeat until "END" "STOP" {x:int}
--- input
1
--- expected error
{
  "code": "E-REPEAT-TERMINATOR",
  "message": "expected 'END' or 'STOP' before end of input",
  "input_location": {
    "origin": "input.bin",
    "line": 2,
    "column": 1
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": null,
  "record_path": null
}
```

```iris example
id: structure-repeat-progress
operation: read
--- spec
repeat until eof {let x=1}
--- input
1
--- expected error
{
  "code": "E-NO-PROGRESS",
  "message": "the body of a repeat until read no input",
  "input_location": {
    "origin": "input.bin",
    "line": 1,
    "column": 1
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": "repeat [1]",
  "record_path": null
}
```

```iris example
id: structure-repeat-missing-scalar
operation: read
--- spec
repeat 2 {k:int if k {x:int}}
--- input
1 4
0
--- expected error
{
  "code": "E-REPEAT-MISSING",
  "message": "'x' is defined in only some repetitions",
  "input_location": {
    "origin": "input.bin",
    "line": 2,
    "column": 2
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": "repeat [2 of 2]",
  "record_path": null
}
```

```iris example
id: structure-repeat-missing-first
operation: read
--- spec
repeat 2 {k:int if k {x:int}}
--- input
0
1 4
--- expected error
{
  "code": "E-REPEAT-MISSING",
  "message": "'x' is defined in only some repetitions",
  "input_location": {
    "origin": "input.bin",
    "line": 2,
    "column": 4
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": "repeat [2 of 2]",
  "record_path": null
}
```

```iris example
id: structure-repeat-all-missing-scalar
operation: read
--- spec
repeat 2 {k:int if k {x:int}}
--- input
0 0
--- expected error
{
  "code": "E-REPEAT-MISSING",
  "message": "'x' is defined in no repetition",
  "input_location": {
    "origin": "input.bin",
    "line": 1,
    "column": 4
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": null,
  "record_path": null
}
```

```iris example
id: structure-repeat-depth
operation: read
--- spec
repeat 2 {k:int if k {x:int} else {x[1]:int}}
--- input
1 4 0 5
--- expected error
{
  "code": "E-REPEAT-DEPTH",
  "message": "'x' has rows of different depths",
  "input_location": {
    "origin": "input.bin",
    "line": 1,
    "column": 8
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": "repeat [2 of 2]",
  "record_path": null
}
```

```iris example
id: structure-sections-repeated
operation: read
--- spec
sections {"A {x:int}" optional}
--- input
A 1
A 2
--- expected error
{
  "code": "E-SECTIONS-REPEATED",
  "message": "a second line matching \"A {x:int}\" (the first is on line 1)",
  "input_location": {
    "origin": "input.bin",
    "line": 2,
    "column": 1
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": null,
  "record_path": null
}
```

```iris example
id: structure-sections-unexpected
operation: read
--- spec
sections {"A"}
--- input
B
--- expected error
{
  "code": "E-SECTIONS-UNEXPECTED",
  "message": "unexpected line; expected one of \"A\"",
  "input_location": {
    "origin": "input.bin",
    "line": 1,
    "column": 1
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": null,
  "record_path": null
}
```

```iris example
id: structure-sections-missing
operation: read
--- spec
sections until "END" {"A" "B"}
--- input
A
END
--- expected error
{
  "code": "E-LINE-MATCH",
  "message": "expected a line matching \"B\" before 'END'",
  "input_location": {
    "origin": "input.bin",
    "line": 2,
    "column": 1
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": null,
  "record_path": null
}
```

```iris example
id: structure-value-type-mix
operation: read
--- spec
repeat 2 {k:int if k {list a:word until eol} else {list a:int until eol}}
--- input
1 hi
0 2
--- expected error
{
  "code": "E-VALUE-TYPE-MIX",
  "message": "'a' mixes strings and numbers",
  "input_location": {
    "origin": "input.bin",
    "line": 2,
    "column": 4
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": "repeat [2 of 2]",
  "record_path": null
}
```

```iris example
id: structure-pattern-word-gap
operation: read
--- spec
line "a b"
--- input
ab
--- expected error
{
  "code": "E-LINE-MATCH",
  "message": "expected a line matching \"a b\"",
  "input_location": {
    "origin": "input.bin",
    "line": 1,
    "column": 1
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": null,
  "record_path": null
}
```

```iris example
id: structure-find-range-committed
operation: read
--- spec
find "A {x:1..2} Z"
--- input
A 9 Z
A 2 Z
--- expected error
{
  "code": "E-EXPECTED-VALUE",
  "message": "expected an int in 1..2, found '9'",
  "input_location": {
    "origin": "input.bin",
    "line": 1,
    "column": 3
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": null,
  "record_path": null
}
```

```iris example
id: structure-sections-range-selected
operation: read
--- spec
sections {"A {x:1..2}" optional "A {y:int}"}
--- input
A 9
--- expected error
{
  "code": "E-EXPECTED-VALUE",
  "message": "expected an int in 1..2, found '9'",
  "input_location": {
    "origin": "input.bin",
    "line": 1,
    "column": 3
  },
  "spec_location": {
    "origin": "spec.iris",
    "line": 1,
    "column": null
  },
  "context": null,
  "record_path": null
}
```
