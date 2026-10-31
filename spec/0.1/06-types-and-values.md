# 6 Types and value-reading statements

This chapter defines the execution of declarations, lists, and named-type
definitions. [Chapter 5](05-reader-specs.md#57-types-ranges-and-options)
defines their syntax; [chapter 4](04-input.md) defines scanning, token
conversion, and positions. Results use the entries and canonical JSON of
[chapter 3](03-data-model.md).

## 6.1 Reading types and resulting entries

A reading type determines how input is converted and validated. Its
resulting value type describes the stored scalar leaves. Implementations
MUST use the following correspondence and reject incompatible statement
forms during parsing with [`E-SPEC-TYPE`](errors.md#114-parse-time-codes).
Aliases MUST obey the same restrictions as their underlying reading type.

| Reading type | Stored value type | Permitted value reads |
|---|---|---|
| `int` | `int` | Scalar, fixed-size array, list |
| `float` | `float` | Scalar, fixed-size array, list |
| `num` | `int` or `float` | Scalar, fixed-size array, list |
| Integer range | `int` | Scalar, fixed-size array, list |
| `word` | `string` | Scalar, fixed-size array, list |
| `rest` | `string` | Scalar only |
| Alphabet | `int` | Exactly two-dimensional fixed-size array |
| Named type | Its underlying reading type's result | Its underlying type's permitted forms |

Numeric reading types are `int`, `float`, `num`, integer ranges, and their
aliases.

A scalar declaration MUST produce depth 0. A declaration with k dimensions
MUST produce depth k; a list MUST produce depth 1. A successful value read
MUST store its name, value type, depth, and complete value as an entry,
except for a discarded read (§6.3). A type definition MUST NOT produce an
entry. Repeat collection adds array structure around these local results;
chapter 7 (`07-structure.md`, planned) defines that collection.

## 6.2 Scalar reading and numeric promotion

An `int` read MUST produce a signed 64-bit integer. A `float` read MUST
produce binary64 even for an integer-looking token. A `num` read MUST try
integer conversion first, then float conversion. An integer spelling
outside the signed 64-bit domain can therefore produce a float through
`num`. The accepted spellings, conversion failures, and rounding rules are
in [§§4.4–4.5](04-input.md#44-integer-tokens).

A `word` read MUST produce the next token as a string. A `rest` read MUST
produce the remaining line text under
[§4.2](04-input.md#42-white-space-and-line-blanks). It requires available
input after initial skipping. Both produce value type `string`; `string`
is not a built-in reading type. All reads, including discarded reads,
MUST obey [§4.10](04-input.md#410-utf-8-validity).

```iris example
id: values-scalars
operation: read
--- spec no-final-newline
a:int b:float c:num d:num w:word r:rest
--- input hex
2d 37 20 32 20 33 20 34 2e 35 20 68 69 20 20 74 68 65 20 72 65 73 74 20 20 0a
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 0, "value": -7},
  {"name": "b", "type": "float", "depth": 0, "value": 2},
  {"name": "c", "type": "int", "depth": 0, "value": 3},
  {"name": "d", "type": "float", "depth": 0, "value": 4.5},
  {"name": "w", "type": "string", "depth": 0, "value": "hi"},
  {"name": "r", "type": "string", "depth": 0, "value": "the rest"}
]}
```

In a `num` array or list, implementations MUST retain integer storage until
a stored float requires promotion. They MUST then convert all earlier
integer leaves and every later integer leaf to binary64. Promotion MUST
apply across the whole array, including different rows. It can round an
earlier large integer. A sentinel is not stored and MUST NOT itself cause
promotion. Empty `num` arrays and lists MUST have value type `int`.

```iris example
id: values-num-promotion
operation: read
--- spec no-final-newline
a[2][2]:num list b:num until 0 c:num
--- input no-final-newline
1 2.5 3 4 9007199254740993 1.0 0 9223372036854775808
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "float", "depth": 2, "value": [[1, 2.5], [3, 4]]},
  {"name": "b", "type": "float", "depth": 1, "value": [9007199254740992, 1]},
  {"name": "c", "type": "float", "depth": 0, "value": 9223372036854776000}
]}
```

## 6.3 Names, discarded reads, and replacement

Names MUST follow [§5.4](05-reader-specs.md#54-names), including its reserved
prefix and contextual-keyword rules. A declaration or list named `_` MUST
perform its complete read and validation but MUST NOT store an entry.
Discarding MUST NOT bypass conversion, dimensions, ranges, layouts,
offsets, or their errors.

A later successful value read with the same name MUST replace the earlier
value, including its type and depth, while retaining its original entry
position under [§3.1](03-data-model.md#31-entries-and-definition-order).
General value lookup and scope belong to chapter 8
(`08-expressions-and-checks.md`, planned).

```iris example
id: values-discard-redefine
operation: read
--- spec no-final-newline
x:int y:word _:int x[2]:float list _:int until 0
--- input no-final-newline
1 hi 99 2 3 8 0
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "float", "depth": 1, "value": [2, 3]},
  {"name": "y", "type": "string", "depth": 0, "value": "hi"}
]}
```

Named types instead obey §6.5: a visible type MUST NOT be redefined.
Types and ordinary value definitions MUST NOT share a name anywhere in a
reader spec. A type and a record-list name MAY coincide. Chapter 7 (`07-structure.md`,
planned) defines record names and record-list names; they are not value
entries.

## 6.4 Integer ranges

A range MUST read integers. Iris 0.1 has no bounded `float` or bounded
`num` reading type. A bound that evaluates to a float, even an integral
float, MUST fail with [`E-RANGE-BOUNDS`](errors.md#115-read-time-codes).
An input token `2.0` cannot satisfy an integer range.

```iris example
id: values-range-float-value
operation: read
--- spec no-final-newline
x:1..3
--- input no-final-newline
2.0
--- expected error
{
  "code": "E-EXPECTED-VALUE",
  "message": "expected an int in 1..3, found '2.0'",
  "input_location": {"origin": "input.bin", "line": 1, "column": 1},
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": "x",
  "record_path": null
}
```

The range `lo..hi` MUST include both bounds; `lo..<hi` MUST include lo and
exclude hi. An omitted bound MUST leave that end unbounded within the
signed 64-bit input domain. The permitted omissions are specified by
[§5.7](05-reader-specs.md#57-types-ranges-and-options).

Implementations MUST evaluate lower then upper bounds as integer scalars.
Strings, arrays, and floats MUST fail with `E-RANGE-BOUNDS`. For an exclusive
upper bound, implementations MUST subtract 1 using checked integer
arithmetic; subtracting from -9223372036854775808 MUST fail with
`E-INTEGER-OVERFLOW`. An inverted interval MUST NOT itself be rejected;
it accepts no ordinary value, but an immediate sentinel can end an empty
list.

```iris example
id: values-range-reversed
operation: read
--- spec no-final-newline
list a:2..1 until 0
--- input no-final-newline
0
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 1, "value": []}
]}
```

For an anonymous range, implementations MUST evaluate its bounds once on
each execution of the declaration or list, before reading values, including
when its result is empty. They MUST NOT reevaluate bounds for each array
or list element. Reexecuting a statement reevaluates its anonymous range.
A named range instead uses the bounds saved by its executed definition
(§6.5). Chapter 8 (`08-expressions-and-checks.md`, planned) defines the
expressions used as bounds.

```iris example
id: values-range-empty-evaluates
operation: read
--- spec no-final-newline
a[0]:0..2.5
--- input
--- expected error
{
  "code": "E-RANGE-BOUNDS",
  "message": "the bounds of a range must be integers",
  "input_location": {"origin": "input.bin", "line": 1, "column": 1},
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": "a",
  "record_path": null
}
```

```iris example
id: values-range-exclusive-overflow
operation: read
--- spec no-final-newline
x:0..<(-9223372036854775807-1)
--- input no-final-newline
1
--- expected error
{
  "code": "E-INTEGER-OVERFLOW",
  "message": "integer overflow in the upper bound of a range",
  "input_location": {"origin": "input.bin", "line": 1, "column": 1},
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": "x",
  "record_path": null
}
```

Implementations MUST check each input integer against the range before
applying offset. They MUST recognize numeric sentinels before range
checking and MUST NOT range-check them. Unread triangular diagonal zeros
are not input values and MUST NOT be range-checked.

```iris example
id: values-ranges
operation: read
--- spec no-final-newline
n:int a[n]:0..<n b:..0 c:-2..-1 d:1..2 offset -1 list e:1..2 until -1 offset 10
--- input no-final-newline
2 0 1 -5 -2 2 1 2 -1
--- expected data
{"iris": "0.1", "entries": [
  {"name": "n", "type": "int", "depth": 0, "value": 2},
  {"name": "a", "type": "int", "depth": 1, "value": [0, 1]},
  {"name": "b", "type": "int", "depth": 0, "value": -5},
  {"name": "c", "type": "int", "depth": 0, "value": -2},
  {"name": "d", "type": "int", "depth": 0, "value": 1},
  {"name": "e", "type": "int", "depth": 1, "value": [11, 12]}
]}
```

A failed conversion or out-of-range value MUST report `E-EXPECTED-VALUE`
at the offending token's start, with profile R availability from
[§11.3.2](errors.md#1132-location-profiles): the active reader-spec line,
null reader-spec column, active execution context, and null record path.
There is no separate range-rejection code. Named-type diagnostics SHOULD
include the name and saved range, as in §6.5.

```iris example
id: values-range-reject
operation: read
--- spec no-final-newline
x:1..3
--- input
4
--- expected error
{
  "code": "E-EXPECTED-VALUE",
  "message": "expected an int in 1..3, found '4'",
  "input_location": {"origin": "input.bin", "line": 1, "column": 1},
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": "x",
  "record_path": null
}
```

## 6.5 Named types and visibility

Type visibility MUST be lexical, according to this table. These rules
apply even in blocks execution would not reach.

| Situation | Rule |
|---|---|
| Definition's right-hand side | The new type is not yet visible. |
| Following statements in the same block | The type is visible. |
| Nested blocks after the definition | The type remains visible. |
| Statements outside its defining block | The type is not visible. |
| Forward use or use after leaving its block | `E-SPEC-UNKNOWN-TYPE` |
| Redefinition while a same-named type is visible | `E-SPEC-NAME-CONFLICT`; no shadowing |
| Separate sibling blocks | Each may define its own type with the same name. |
| Defining a base-type name, including `alphabet` | `E-SPEC-NAME-CONFLICT` |
| Same spelling as an ordinary value definition anywhere in the reader spec | `E-SPEC-NAME-CONFLICT`, including disjoint or unreachable blocks |

The final row MUST be checked globally after statement parsing, before
name resolution, under [§5.1](05-reader-specs.md#51-reader-specs-and-parsing).
Type visibility is determined during parsing; bound-expression resolution
and record-name checks do not extend that visibility.

```iris example
id: values-type-forward
operation: parse
--- spec no-final-newline
a:t type t=int
--- expected error
{
  "code": "E-SPEC-UNKNOWN-TYPE",
  "message": "unknown type 't'",
  "input_location": null,
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": null,
  "record_path": null
}
```

```iris example
id: values-type-escape
operation: parse
--- spec no-final-newline
if 1 {type t=int} a:t
--- expected error
{
  "code": "E-SPEC-UNKNOWN-TYPE",
  "message": "unknown type 't'",
  "input_location": null,
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": null,
  "record_path": null
}
```

```iris example
id: values-type-shadow
operation: parse
--- spec no-final-newline
type t=int if 1 {type t=word}
--- expected error
{
  "code": "E-SPEC-NAME-CONFLICT",
  "message": "the type 't' is already defined",
  "input_location": null,
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": null,
  "record_path": null
}
```

```iris example
id: values-type-base
operation: parse
--- spec no-final-newline
type int=0..
--- expected error
{
  "code": "E-SPEC-NAME-CONFLICT",
  "message": "'int' is a base type",
  "input_location": null,
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": null,
  "record_path": null
}
```

```iris example
id: values-type-value-conflict
operation: parse
--- spec no-final-newline
if 0 {type t=int} t:int
--- expected error
{
  "code": "E-SPEC-NAME-CONFLICT",
  "message": "'t' names a type, so it cannot also name a value",
  "input_location": null,
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": null,
  "record_path": null
}
```

```iris example
id: values-type-siblings
operation: read
--- spec no-final-newline
if 1 {type t=int a:t} if 1 {type t=word b:t}
--- input no-final-newline
7 hi
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 0, "value": 7},
  {"name": "b", "type": "string", "depth": 0, "value": "hi"}
]}
```

```iris example
id: values-type-nested
operation: read
--- spec no-final-newline
type t=int if 1 {a:t}
--- input no-final-newline
7
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 0, "value": 7}
]}
```

Executing a type definition MUST evaluate and save its range at that point
and MUST NOT read input or produce an entry. Subsequent changes to values
used by its bounds MUST NOT change the saved range. Executing an alias
definition MUST capture the referenced type's evaluated range. The alias
also retains its underlying reading type and alphabet options, if any.

```iris example
id: values-range-timing
operation: read
--- spec no-final-newline
n:int type frozen=0..n type alias=frozen let n=3 a:alias b:0..n
--- input no-final-newline
2 2 3
--- expected data
{"iris": "0.1", "entries": [
  {"name": "n", "type": "int", "depth": 0, "value": 3},
  {"name": "a", "type": "int", "depth": 0, "value": 2},
  {"name": "b", "type": "int", "depth": 0, "value": 3}
]}
```

```iris example
id: values-range-frozen-reject
operation: read
--- spec no-final-newline
n:int type t=0..n let n=3 a:t
--- input no-final-newline
2 3
--- expected error
{
  "code": "E-EXPECTED-VALUE",
  "message": "expected t (an int in 0..2), found '3'",
  "input_location": {"origin": "input.bin", "line": 1, "column": 3},
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": "a",
  "record_path": null
}
```

```iris example
id: values-type-aliases
operation: read
--- spec no-final-newline
type w=word type v=w type c=alphabet "ab" blank "." a:v g[1][2]:c
--- input no-final-newline
hi a.
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "string", "depth": 0, "value": "hi"},
  {"name": "g", "type": "int", "depth": 2, "value": [[1, 0]]}
]}
```

A type definition in a repeat MUST execute again on each iteration. An
unexecuted type definition MUST NOT evaluate its bounds.

```iris example
id: values-range-reexecution
operation: read
--- spec no-final-newline
repeat 2 {n:int type t=0..n a:t}
--- input no-final-newline
1 1 3 3
--- expected data
{"iris": "0.1", "entries": [
  {"name": "n", "type": "int", "depth": 1, "value": [1, 3]},
  {"name": "a", "type": "int", "depth": 1, "value": [1, 3]}
]}
```

```iris example
id: values-type-unreached
operation: read
--- spec no-final-newline
if 0 {type t=0..1.0 a:t}
--- input
--- expected data
{"iris": "0.1", "entries": []}
```

If runtime lookup of a parsed named type has no executed definition,
implementations MUST report `E-TYPE-UNDEFINED`. This is a conditional guard
preserved from the reference. No public reader spec exercising it has been
established: lexical visibility and ordinary sequencing appear to place
each legal use after its definition. Forward or escaped uses fail during
parsing instead, as the examples above show.

## 6.6 Fixed-size arrays and empty rows

Implementations MUST evaluate dimensions from left to right. Every dimension
MUST be an integer scalar in 0..2147483647; violations MUST report
`E-COUNT`. A zero dimension MUST NOT suppress evaluation of later dimensions.
All dimensions MUST be evaluated before checking their product.

```iris example
id: values-count-type
operation: read
--- spec no-final-newline
x[1.0]:int
--- input
--- expected error
{
  "code": "E-COUNT",
  "message": "array size must be an integer",
  "input_location": {"origin": "input.bin", "line": 1, "column": 1},
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": "x",
  "record_path": null
}
```

```iris example
id: values-count-negative
operation: read
--- spec no-final-newline
x[-1]:int
--- input
--- expected error
{
  "code": "E-COUNT",
  "message": "array size is negative (-1)",
  "input_location": {"origin": "input.bin", "line": 1, "column": 1},
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": "x",
  "record_path": null
}
```

```iris example
id: values-count-large
operation: read
--- spec no-final-newline
x[2147483648]:int
--- input
--- expected error
{
  "code": "E-COUNT",
  "message": "array size is too large (2147483648)",
  "input_location": {"origin": "input.bin", "line": 1, "column": 1},
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": "x",
  "record_path": null
}
```

For ordinary arrays, implementations MUST multiply dimensions in order and
report `E-SIZE-LIMIT` if any intermediate product exceeds 2147483647.
Thus a later zero dimension does not rescue an excessive earlier product.
Implementations MUST read leaves with the last dimension varying fastest,
independently of physical input line boundaries.

```iris example
id: values-size-limit
operation: read
--- spec no-final-newline
x[46341][46341]:int
--- input
--- expected error
{
  "code": "E-SIZE-LIMIT",
  "message": "array is too large",
  "input_location": {"origin": "input.bin", "line": 1, "column": 1},
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": "x",
  "record_path": null
}
```

```iris example
id: values-arrays
operation: read
--- spec no-final-newline
n:int a[n][2][2]:int
--- input no-final-newline
2 1 2 3 4 5 6 7 8
--- expected data
{"iris": "0.1", "entries": [
  {"name": "n", "type": "int", "depth": 0, "value": 2},
  {"name": "a", "type": "int", "depth": 3, "value": [[[1, 2], [3, 4]], [[5, 6], [7, 8]]]}
]}
```

Zero dimensions MUST consume no values and MUST preserve the declared
value type and depth, with `num` stored as `int`. For example, `[2][0]`
produces two empty rows, while `[0][3]` produces no rows at depth 2.
Implementations MUST NOT invent structures below a nonexistent row.
Alphabet arrays likewise retain value type `int` and depth 2 when empty.

```iris example
id: values-empty
operation: read
--- spec no-final-newline
a[0]:int b[0]:float c[0]:word d[2][0]:float e[0][3]:num list f:float until 0 list g:word until "END"
--- input no-final-newline
0 END
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 1, "value": []},
  {"name": "b", "type": "float", "depth": 1, "value": []},
  {"name": "c", "type": "string", "depth": 1, "value": []},
  {"name": "d", "type": "float", "depth": 2, "value": [[], []]},
  {"name": "e", "type": "int", "depth": 2, "value": []},
  {"name": "f", "type": "float", "depth": 1, "value": []},
  {"name": "g", "type": "string", "depth": 1, "value": []}
]}
```

Empty rows can also be read using an immediate sentinel. Chapter 7
(`07-structure.md`, planned) defines their collection by repeat.

```iris example
id: values-empty-rows
operation: read
--- spec no-final-newline
a[2][0]:int repeat 2 {list b:int until 0}
--- input
0
0
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 2, "value": [[], []]},
  {"name": "b", "type": "int", "depth": 2, "value": [[], []]}
]}
```

A blank physical line MUST NOT supply an empty `until eol` list: initial
skipping seeks data on a later line and fails if none remains (§6.9).
Use a zero dimension or an immediate sentinel for an empty row. Empty
structures remain subject to chapter 10 (`10-work-budgets.md`, planned).

```iris example
id: values-type-rest-array
operation: parse
--- spec no-final-newline
type t=rest a[1]:t
--- expected error
{
  "code": "E-SPEC-TYPE",
  "message": "'rest' values cannot be arrays",
  "input_location": null,
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": null,
  "record_path": null
}
```

## 6.7 Offsets and evaluation order

An offset MUST be permitted only for numeric reads and MUST evaluate to an
integer scalar. A nonnumeric reading type with an offset MUST fail during
parsing with `E-SPEC-TYPE`; an offset value of the wrong type or depth MUST
fail during execution with `E-OFFSET-TYPE`.

```iris example
id: values-type-word-offset
operation: parse
--- spec no-final-newline
a:word offset 1
--- expected error
{
  "code": "E-SPEC-TYPE",
  "message": "'offset' needs numbers",
  "input_location": null,
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": null,
  "record_path": null
}
```

Implementations MUST add the offset to each converted, range-checked number,
preserving its `int` or `float` value type. Integer addition MUST be checked
and report `E-INTEGER-OVERFLOW` on overflow. Float addition MUST convert
the offset to binary64. Sentinels MUST NOT be adjusted. For triangular
layouts, implementations MUST adjust each stored triangle value before
mirroring; omitted diagonals MUST stay zero (§6.8).

The evaluation order MUST be:

| Statement | Order |
|---|---|
| Declaration | Evaluate offset; obtain or evaluate range; evaluate dimensions; evaluate layout if present; read and range-check values; adjust values; store. |
| List | Obtain or evaluate range; evaluate sentinel if present; read through termination; evaluate offset; adjust stored values; store. |
| Type definition | Evaluate and save its range. |

A failing step MUST stop execution before later steps. In particular, a
list's offset failure occurs after consuming its sentinel or reaching end
of line, while a declaration's offset-type failure occurs before reading.

```iris example
id: values-offset-type
operation: read
--- spec no-final-newline
x:int offset 1.0
--- input no-final-newline
7
--- expected error
{
  "code": "E-OFFSET-TYPE",
  "message": "offset must be an integer",
  "input_location": {"origin": "input.bin", "line": 1, "column": 1},
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": "x",
  "record_path": null
}
```

```iris example
id: values-offset-list-timing
operation: read
--- spec no-final-newline
list x:int until 0 offset "x"
--- input no-final-newline
1 2 0
--- expected error
{
  "code": "E-OFFSET-TYPE",
  "message": "offset must be an integer",
  "input_location": {"origin": "input.bin", "line": 1, "column": 6},
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": "x",
  "record_path": null
}
```

```iris example
id: values-offset-overflow
operation: read
--- spec no-final-newline
x:int offset 1
--- input no-final-newline
9223372036854775807
--- expected error
{
  "code": "E-INTEGER-OVERFLOW",
  "message": "integer overflow when applying offset",
  "input_location": {"origin": "input.bin", "line": 1, "column": 20},
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": "x",
  "record_path": null
}
```

When no offset is written, the default zero MUST still be added to stored
numbers. The effect on floating point signed zero is defined and illustrated
in [§3.5](03-data-model.md#35-floating-point-values).

## 6.8 Matrix layouts

A layout MUST be used only with a two-dimensional numeric declaration;
other uses MUST fail during parsing with `E-SPEC-TYPE`. Its expression MUST
evaluate to a string scalar. Implementations MUST recognize exactly the
nine case-sensitive names in this table; there are no other aliases or
a diagonal-only layout.

Indices below start at 0. For r rows and c columns, or an n by n triangular
layout, implementations MUST consume values in the following cell order.
Both loops increase; an empty index interval contributes no cells.

| Layout | Outer loop | Inner loop; cell is (i, j) |
|---|---|---|
| `FULL_MATRIX` | i = 0 through r−1 | j = 0 through c−1 |
| `UPPER_ROW` | i = 0 through n−1 | j = i+1 through n−1 |
| `LOWER_ROW` | i = 0 through n−1 | j = 0 through i−1 |
| `UPPER_DIAG_ROW` | i = 0 through n−1 | j = i through n−1 |
| `LOWER_DIAG_ROW` | i = 0 through n−1 | j = 0 through i |
| `UPPER_COL` | j = 0 through n−1 | i = 0 through j−1 |
| `LOWER_COL` | j = 0 through n−1 | i = j+1 through n−1 |
| `UPPER_DIAG_COL` | j = 0 through n−1 | i = 0 through j |
| `LOWER_DIAG_COL` | j = 0 through n−1 | i = j through n−1 |

`FULL_MATRIX` MUST read all cells without mirroring. It MUST accept
rectangular and asymmetric matrices and use ordinary array size checks.

```iris example
id: values-layout-full-matrix
operation: read
--- spec no-final-newline
m[4][4]:int layout "FULL_MATRIX" offset 10
--- input
1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16
--- expected data
{"iris": "0.1", "entries": [
  {"name": "m", "type": "int", "depth": 2, "value": [[11, 12, 13, 14], [15, 16, 17, 18], [19, 20, 21, 22], [23, 24, 25, 26]]}
]}
```

```iris example
id: values-layout-rectangular
operation: read
--- spec no-final-newline
m[2][3]:int layout "FULL_MATRIX" offset -1
--- input no-final-newline
1 2 3 4 5 6
--- expected data
{"iris": "0.1", "entries": [
  {"name": "m", "type": "int", "depth": 2, "value": [[0, 1, 2], [3, 4, 5]]}
]}
```

Every triangular layout MUST require equal dimensions and n*n at most
2147483647, reporting `E-LAYOUT` or `E-SIZE-LIMIT`, respectively.
Implementations MUST initialize the matrix to numeric zero, read and
range-check the stored triangle, apply offset to its values, and assign
each adjusted value at (i, j) to both (i, j) and (j, i). The result MUST be
symmetric at depth 2. Non-diagonal layouts MUST leave all diagonal cells
zero, regardless of range or offset. Diagonal layouts MUST read and adjust
the diagonals as well.

Each example uses distinct stored input values and offset 10. Mirrored
copies necessarily repeat. The four row/column pairs yield equal results:
`UPPER_ROW`/`LOWER_COL`, `LOWER_ROW`/`UPPER_COL`,
`UPPER_DIAG_ROW`/`LOWER_DIAG_COL`, and
`LOWER_DIAG_ROW`/`UPPER_DIAG_COL`.

```iris example
id: values-layout-upper-row
operation: read
--- spec no-final-newline
m[4][4]:int layout "UPPER_ROW" offset 10
--- input
1 2 3 4 5 6
--- expected data
{"iris": "0.1", "entries": [
  {"name": "m", "type": "int", "depth": 2, "value": [[0, 11, 12, 13], [11, 0, 14, 15], [12, 14, 0, 16], [13, 15, 16, 0]]}
]}
```

```iris example
id: values-layout-lower-row
operation: read
--- spec no-final-newline
m[4][4]:int layout "LOWER_ROW" offset 10
--- input
1 2 3 4 5 6
--- expected data
{"iris": "0.1", "entries": [
  {"name": "m", "type": "int", "depth": 2, "value": [[0, 11, 12, 14], [11, 0, 13, 15], [12, 13, 0, 16], [14, 15, 16, 0]]}
]}
```

```iris example
id: values-layout-upper-diag-row
operation: read
--- spec no-final-newline
m[4][4]:int layout "UPPER_DIAG_ROW" offset 10
--- input
1 2 3 4 5 6 7 8 9 10
--- expected data
{"iris": "0.1", "entries": [
  {"name": "m", "type": "int", "depth": 2, "value": [[11, 12, 13, 14], [12, 15, 16, 17], [13, 16, 18, 19], [14, 17, 19, 20]]}
]}
```

```iris example
id: values-layout-lower-diag-row
operation: read
--- spec no-final-newline
m[4][4]:int layout "LOWER_DIAG_ROW" offset 10
--- input
1 2 3 4 5 6 7 8 9 10
--- expected data
{"iris": "0.1", "entries": [
  {"name": "m", "type": "int", "depth": 2, "value": [[11, 12, 14, 17], [12, 13, 15, 18], [14, 15, 16, 19], [17, 18, 19, 20]]}
]}
```

```iris example
id: values-layout-upper-col
operation: read
--- spec no-final-newline
m[4][4]:int layout "UPPER_COL" offset 10
--- input
1 2 3 4 5 6
--- expected data
{"iris": "0.1", "entries": [
  {"name": "m", "type": "int", "depth": 2, "value": [[0, 11, 12, 14], [11, 0, 13, 15], [12, 13, 0, 16], [14, 15, 16, 0]]}
]}
```

```iris example
id: values-layout-lower-col
operation: read
--- spec no-final-newline
m[4][4]:int layout "LOWER_COL" offset 10
--- input
1 2 3 4 5 6
--- expected data
{"iris": "0.1", "entries": [
  {"name": "m", "type": "int", "depth": 2, "value": [[0, 11, 12, 13], [11, 0, 14, 15], [12, 14, 0, 16], [13, 15, 16, 0]]}
]}
```

```iris example
id: values-layout-upper-diag-col
operation: read
--- spec no-final-newline
m[4][4]:int layout "UPPER_DIAG_COL" offset 10
--- input
1 2 3 4 5 6 7 8 9 10
--- expected data
{"iris": "0.1", "entries": [
  {"name": "m", "type": "int", "depth": 2, "value": [[11, 12, 14, 17], [12, 13, 15, 18], [14, 15, 16, 19], [17, 18, 19, 20]]}
]}
```

```iris example
id: values-layout-lower-diag-col
operation: read
--- spec no-final-newline
m[4][4]:int layout "LOWER_DIAG_COL" offset 10
--- input
1 2 3 4 5 6 7 8 9 10
--- expected data
{"iris": "0.1", "entries": [
  {"name": "m", "type": "int", "depth": 2, "value": [[11, 12, 13, 14], [12, 15, 16, 17], [13, 16, 18, 19], [14, 17, 19, 20]]}
]}
```

Layout validation MUST first require a string scalar, then handle
`FULL_MATRIX`, then require squareness, then recognize a triangular name,
then check its cell count. An unknown non-`FULL_MATRIX` name on a
rectangular matrix therefore fails the square-matrix check first.

```iris example
id: values-type-layout-shape
operation: parse
--- spec no-final-newline
a[1]:int layout "FULL_MATRIX"
--- expected error
{
  "code": "E-SPEC-TYPE",
  "message": "'layout' needs a two-dimensional array of numbers",
  "input_location": null,
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": null,
  "record_path": null
}
```

```iris example
id: values-layout-type
operation: read
--- spec no-final-newline
m[2][2]:int layout 1
--- input
--- expected error
{
  "code": "E-LAYOUT",
  "message": "matrix layout must be a string",
  "input_location": {"origin": "input.bin", "line": 1, "column": 1},
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": "m",
  "record_path": null
}
```

```iris example
id: values-layout-square
operation: read
--- spec no-final-newline
m[2][3]:int layout "UPPER_ROW"
--- input
--- expected error
{
  "code": "E-LAYOUT",
  "message": "layout UPPER_ROW needs a square matrix",
  "input_location": {"origin": "input.bin", "line": 1, "column": 1},
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": "m",
  "record_path": null
}
```

```iris example
id: values-layout-unknown
operation: read
--- spec no-final-newline
m[2][2]:int layout "DIAGONAL"
--- input
--- expected error
{
  "code": "E-LAYOUT",
  "message": "unknown matrix layout 'DIAGONAL'",
  "input_location": {"origin": "input.bin", "line": 1, "column": 1},
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": "m",
  "record_path": null
}
```

```iris example
id: values-layout-limit
operation: read
--- spec no-final-newline
m[46341][46341]:int layout "UPPER_ROW"
--- input
--- expected error
{
  "code": "E-SIZE-LIMIT",
  "message": "matrix is too large",
  "input_location": {"origin": "input.bin", "line": 1, "column": 1},
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": "m",
  "record_path": null
}
```

## 6.9 Sentinel and end-of-line lists

A list MUST produce a depth-1 array. For a sentinel list, implementations
MUST evaluate the sentinel expression once as a scalar, then read across
physical lines until it is found. They MUST consume but MUST NOT store the
sentinel. Its type MUST NOT cause numeric promotion of the result.

Sentinel recognition MUST follow these rules:

- Two integers compare exactly. Other numeric comparisons convert both
  operands to binary64 before testing equality.
- A string sentinel compares against the complete next token before
  conversion, so a nonnumeric string such as `END` can end a numeric list.
- A word list accepts a string sentinel, or an integer sentinel converted
  to its ordinary decimal spelling. The words `+0` and `00` do not match
  the integer sentinel 0. A float sentinel for a word list MUST report
  `E-LIST-TERMINATOR`.
- An input token matching a numeric sentinel MUST still be readable by
  the list's reading type. Numeric conversion precedes equality; equality
  precedes range checking.
- Sentinels MUST NOT be range-checked or adjusted by offset.

```iris example
id: values-lists
operation: read
--- spec no-final-newline
list a:1..3 until -1 offset -1 list b:float until "END" list c:word until 0 list d:num until 2.0 tail:int
--- input no-final-newline
1 3 -1 1 2.5 END +0 00 0 1 2 9
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 1, "value": [0, 2]},
  {"name": "b", "type": "float", "depth": 1, "value": [1, 2.5]},
  {"name": "c", "type": "string", "depth": 1, "value": ["+0", "00"]},
  {"name": "d", "type": "int", "depth": 1, "value": [1]},
  {"name": "tail", "type": "int", "depth": 0, "value": 9}
]}
```

Mixed numeric comparison can consume a mathematically different integer
because of binary64 rounding. The sentinel below excludes the integer
9007199254740993 without promoting the empty result.

```iris example
id: values-sentinel-rounding
operation: read
--- spec no-final-newline
list a:num until 9007199254740992.0 tail:int
--- input no-final-newline
9007199254740993 7
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 1, "value": []},
  {"name": "tail", "type": "int", "depth": 0, "value": 7}
]}
```

For `until eol`, implementations MUST use the remaining data on the current
physical line, or seek the next data if that line is exhausted, then read
while data remains on that line. They MUST apply
[chapter 4's scanning settings](04-input.md#43-tokens-and-separators).
Blank physical lines MUST NOT produce empty lists. If no data remains,
the statement MUST report `E-EXPECTED-VALUE` rather than return an empty
list. End of line is a scanning boundary, not a token sentinel.

```iris example
id: values-lists-eol
operation: read
--- spec no-final-newline
list a:int until 0 list b:int until eol list c:int until eol
--- input
1 0 2 3

4 5
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 1, "value": [1]},
  {"name": "b", "type": "int", "depth": 1, "value": [2, 3]},
  {"name": "c", "type": "int", "depth": 1, "value": [4, 5]}
]}
```

```iris example
id: values-lists-eof
operation: read
--- spec no-final-newline
list a:int until eol
--- input


--- expected error
{
  "code": "E-EXPECTED-VALUE",
  "message": "expected a line of values, found end of input",
  "input_location": {"origin": "input.bin", "line": 3, "column": 1},
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": "a",
  "record_path": null
}
```

At end of input without its sentinel, a string-sentinel list MUST report
`E-EXPECTED-TOKEN`; a numeric-sentinel list MUST report `E-EXPECTED-VALUE`
for the missing reading value.

```iris example
id: values-lists-numeric-eof
operation: read
--- spec no-final-newline
list a:int until 0
--- input
1
--- expected error
{
  "code": "E-EXPECTED-VALUE",
  "message": "expected int, found end of input",
  "input_location": {"origin": "input.bin", "line": 2, "column": 1},
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": "a",
  "record_path": null
}
```

```iris example
id: values-lists-string-eof
operation: read
--- spec no-final-newline
list a:int until "END"
--- input
1
--- expected error
{
  "code": "E-EXPECTED-TOKEN",
  "message": "expected 'END', found end of input",
  "input_location": {"origin": "input.bin", "line": 2, "column": 1},
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": "a",
  "record_path": null
}
```

```iris example
id: values-lists-terminator-shape
operation: read
--- spec no-final-newline
t[1]:int list a:int until t
--- input no-final-newline
0
--- expected error
{
  "code": "E-LIST-TERMINATOR",
  "message": "list terminator must be a single value",
  "input_location": {"origin": "input.bin", "line": 1, "column": 2},
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": "a",
  "record_path": null
}
```

```iris example
id: values-lists-terminator-word
operation: read
--- spec no-final-newline
list a:word until 1.0
--- input
--- expected error
{
  "code": "E-LIST-TERMINATOR",
  "message": "a list of words needs a string or integer terminator",
  "input_location": {"origin": "input.bin", "line": 1, "column": 1},
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": "a",
  "record_path": null
}
```

```iris example
id: values-type-list-rest
operation: parse
--- spec no-final-newline
list a:rest until eol
--- expected error
{
  "code": "E-SPEC-TYPE",
  "message": "a list cannot be of 'rest' or of an alphabet type",
  "input_location": null,
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": null,
  "record_path": null
}
```

## 6.10 Alphabets

An alphabet MUST read exactly rows*columns symbols into an `int` entry
at depth 2, in row-major order. Physical input lines MUST NOT delimit
matrix rows. The dimensions follow §6.6; their product MUST be at most
2147483647, otherwise `E-SIZE-LIMIT`.

Before each symbol, implementations MUST apply normal skipping under
[chapter 4](04-input.md#47-comment-and-stop-prefixes), then discard a byte
in `strip` and repeat skipping until a non-stripped byte is available.
For that byte they MUST apply this priority:

1. A byte in `blank` produces 0, even if it also occurs in the alphabet.
2. Otherwise, a byte in the alphabet produces its first zero-based byte
   index plus base.
3. Otherwise, report `E-SYMBOL` at that byte.

Base defaults to 1; blank and strip default to empty strings. Strip thus
takes priority over blank, which takes priority over alphabet mapping.
Duplicate alphabet bytes MUST use their first occurrence. Repeated options
MUST replace earlier values independently, as specified in
[§5.7](05-reader-specs.md#57-types-ranges-and-options).

```iris example
id: values-alphabet
operation: read
--- spec no-final-newline
g[2][3]:alphabet "ab" base 0 blank "." strip "|-"
--- input no-final-newline
a|b.
---
b|a.
--- expected data
{"iris": "0.1", "entries": [
  {"name": "g", "type": "int", "depth": 2, "value": [[0, 1, 0], [1, 0, 0]]}
]}
```

```iris example
id: values-alphabet-options
operation: read
--- spec no-final-newline
g[1][4]:alphabet "aba" base 5 blank "a" strip "b" base -2 blank "." strip "|"
--- input no-final-newline
a|b.a
--- expected data
{"iris": "0.1", "entries": [
  {"name": "g", "type": "int", "depth": 2, "value": [[-2, -1, 0, -2]]}
]}
```

```iris example
id: values-alphabet-priority
operation: read
--- spec no-final-newline
g[1][2]:alphabet "ab" blank "a" strip "."
--- input no-final-newline
a.b
--- expected data
{"iris": "0.1", "entries": [
  {"name": "g", "type": "int", "depth": 2, "value": [[0, 2]]}
]}
```

```iris example
id: values-alphabet-overlap-strip
operation: read
--- spec no-final-newline
g[1][1]:alphabet "ab" blank "a" strip "a"
--- input no-final-newline
ab
--- expected data
{"iris": "0.1", "entries": [
  {"name": "g", "type": "int", "depth": 2, "value": [[2]]}
]}
```

Alphabet matching, indexing, and membership in options MUST operate on
bytes, including UTF-8 bytes, rather than Unicode scalar values. A
non-ASCII character in the alphabet occupies one position per UTF-8 byte;
in `blank` or `strip`, each of its bytes is an independent member. Each
byte selected after skipping and stripping occupies one matrix cell. Thus
`éa` below supplies three cells. This integer result does not relax the
input-wide [UTF-8 requirements](04-input.md#410-utf-8-validity) or the
[reader-spec UTF-8 requirements](05-reader-specs.md#51-reader-specs-and-parsing).

```iris example
id: values-alphabet-bytes
operation: read
--- spec no-final-newline
g[1][3]:alphabet "éa"
--- input no-final-newline
éa
--- expected data
{"iris": "0.1", "entries": [
  {"name": "g", "type": "int", "depth": 2, "value": [[1, 2, 3]]}
]}
```

Empty alphabets MUST be permitted, including blank-only reads and empty
matrices. Empty arrays MUST retain value type `int` and depth 2.

```iris example
id: values-alphabet-empty
operation: read
--- spec no-final-newline
a[0][2]:alphabet "ab" b[2][0]:alphabet ""
--- input
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 2, "value": []},
  {"name": "b", "type": "int", "depth": 2, "value": [[], []]}
]}
```

Base-plus-index addition MUST be checked and report `E-INTEGER-OVERFLOW`
on overflow. An unrecognized byte or missing symbol MUST report `E-SYMBOL`.
Missing symbols locate end of input; the recommended message identifies
the required row and column, both starting at 1.

```iris example
id: values-alphabet-symbol
operation: read
--- spec no-final-newline
g[1][2]:alphabet "ab"
--- input no-final-newline
ax
--- expected error
{
  "code": "E-SYMBOL",
  "message": "unexpected symbol 'x', expected one of 'ab'",
  "input_location": {"origin": "input.bin", "line": 1, "column": 2},
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": "g",
  "record_path": null
}
```

```iris example
id: values-alphabet-eof
operation: read
--- spec no-final-newline
g[2][2]:alphabet "ab"
--- input no-final-newline
ab
a
--- expected error
{
  "code": "E-SYMBOL",
  "message": "expected a symbol for row 2, column 2, found end of input",
  "input_location": {"origin": "input.bin", "line": 2, "column": 2},
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": "g",
  "record_path": null
}
```

```iris example
id: values-alphabet-overflow
operation: read
--- spec no-final-newline
g[1][1]:alphabet "ab" base 9223372036854775807
--- input no-final-newline
b
--- expected error
{
  "code": "E-INTEGER-OVERFLOW",
  "message": "integer overflow in the value of a symbol",
  "input_location": {"origin": "input.bin", "line": 1, "column": 1},
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": "g",
  "record_path": null
}
```

Alphabet reading types MUST have exactly two dimensions and MUST NOT be
used as scalars, lists, or with layout or offset. These are parse-time
`E-SPEC-TYPE` restrictions. Captures MUST reject alphabet types with
`E-SPEC-PATTERN`; chapter 7 (`07-structure.md`, planned) defines captures.

```iris example
id: values-type-alphabet-shape
operation: parse
--- spec no-final-newline
g[1]:alphabet "ab"
--- expected error
{
  "code": "E-SPEC-TYPE",
  "message": "an alphabet type needs a two-dimensional array",
  "input_location": null,
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": null,
  "record_path": null
}
```

## 6.11 Errors and related chapters

The [parse-time registry](errors.md#114-parse-time-codes) assigns codes for
unknown types, name conflicts, and incompatible reading forms. The
[read-time registry](errors.md#115-read-time-codes) assigns codes for value
conversion and range rejection, invalid bounds, counts, layouts, offsets,
sentinels, symbols, structural size limits, and checked integer adjustment.
Implementations MUST use those codes and location profiles, following the
local evaluation order in §6.7 and the general
[error order](errors.md#112-error-order). Errors after consumption, such as
offset overflow, MUST retain their reported cursor rather than be moved
to a token's start.

`E-VALUE-TYPE-MIX` guards attempts to combine strings and numbers. A single
fixed reading type cannot produce that mixture in an ordinary array or
list; the direct value-construction guard is defensive. Repeat columns can
encounter it through different reading statements. Chapter 7
(`07-structure.md`, planned) owns that behavior and its example.
`E-TYPE-UNDEFINED` remains the conditional runtime guard described in §6.5,
without a verified public reproducer.

Expression evaluation and its failures belong to chapter 8
(`08-expressions-and-checks.md`, planned); parameters and instances belong
to chapter 9 (`09-params-and-instances.md`, planned). Chapter 10
(`10-work-budgets.md`, planned) defines work accounting, including empty
rows. Appendix B (`appendix-b-dzn.md`, planned) defines dzn output. The
[existing data model](03-data-model.md) defines the complete canonical JSON
used in this chapter's examples.
