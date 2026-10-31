# 3 Data model and canonical JSON

## 3.1 Entries and definition order

Data is an ordered sequence of entries. Each entry has a name, a value type
(`int`, `float`, or `string`), a depth, and a value. Names MUST be unique in
the resulting data. Entries MUST appear in the order their names were first
defined; updating a name MUST retain its position. Implementations MUST NOT
sort entries by name.

The value type describes stored scalar leaves. A reading type such as `num`
or `word` is distinct from the value type it produces. A record is one
repeat iteration's collection of fields. Records MUST be stored column-wise
as entries for their fields; records themselves are not object values.
Record names used by checks need not be data entries.

```iris example
id: data-definition-order
operation: read
--- spec
z: int
a: word
let z = 9
--- input
1 a
--- expected data
{"iris": "0.1", "entries": [
  {"name": "z", "type": "int", "depth": 0, "value": 9},
  {"name": "a", "type": "string", "depth": 0, "value": "a"}
]}
```

## 3.2 Depth, arrays, and paths

Depth MUST be a nonnegative integer. At depth 0 the value is a scalar of
the entry's value type. At depth greater than 0 it is an array, each of
whose elements has one less remaining depth. Every existing scalar leaf
MUST have the entry's value type and occur at exactly the declared depth.
Arrays MAY be ragged: siblings need not have the same length.

A path is a sequence of indices counted from 0, outermost first. The empty
path `[]` selects the whole entry. Each further index selects an element
of the current array and MUST be less than its length. For the following
entry, `[0]` selects `[1, -2]`, and `[0, 1]` selects `-2`.

```iris example
id: data-ragged-rows
operation: read
--- spec
repeat 2 { list row: int until eol }
--- input
1 -2
3
--- expected data
{"iris": "0.1", "entries": [
  {"name": "row", "type": "int", "depth": 2, "value": [[1, -2], [3]]}
]}
```

An empty array retains its declared value type and depth. Implementations
MUST NOT infer either solely from the serialized array. An empty array can
occur at any array level; dimensions below it are not represented and MUST
NOT be invented. An empty `num` array has value type `int`.

```iris example
id: data-empty-arrays
operation: read
--- spec
a[0]: int
b[0]: float
c[0]: word
d[2][0]: float
e[0]: num
repeat 0 { f[2][3]: float }
--- input
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 1, "value": []},
  {"name": "b", "type": "float", "depth": 1, "value": []},
  {"name": "c", "type": "string", "depth": 1, "value": []},
  {"name": "d", "type": "float", "depth": 2, "value": [[], []]},
  {"name": "e", "type": "int", "depth": 1, "value": []},
  {"name": "f", "type": "float", "depth": 3, "value": []}
]}
```

## 3.3 Canonical data document

The canonical data document MUST be a JSON object with exactly two members:
`iris`, whose value is `"0.1"`, and `entries`, an array in definition order.
Each entry MUST have exactly the members `name` (string), `type` (value-type
string), `depth` (nonnegative integer), and `value` (scalar or nested array).
Objects, booleans, and null MUST NOT occur as entry values or leaves.

Encoders SHOULD present members in the order shown below. Object-member
order and insignificant JSON whitespace do not affect conformance; entry
order, value types, depths, and values do. JSON documents MUST use UTF-8
without a BOM and MUST NOT have duplicate object members.

```json
{"iris": "0.1", "entries": [
  {"name": "vars", "type": "int", "depth": 0, "value": 3},
  {"name": "clause", "type": "int", "depth": 2, "value": [[1, -2], [3]]}
]}
```

The [data schema](../schema/data.schema.json) checks the structural subset
of these rules described in §3.9.

## 3.4 Integers

Integer values MUST be signed 64-bit integers, from -9223372036854775808
through 9223372036854775807 inclusive. Canonical JSON MUST encode them as
base-10 integer number tokens, with no fraction or exponent, no leading
zeros, and no plus sign. Integer zero is `0`. Encoders and decoders MUST
preserve every integer exactly; strings and rounded binary64 approximations
are not valid substitutes.

These bounds differ from the host constants `gecode.int_min` and
`gecode.int_max`, whose fixed values are -2147483646 and 2147483646.

```iris example
id: data-integer-limits
operation: read
--- spec
lo: int
hi: int
--- input
-9223372036854775808 9223372036854775807
--- expected data
{"iris": "0.1", "entries": [
  {"name": "lo", "type": "int", "depth": 0, "value": -9223372036854775808},
  {"name": "hi", "type": "int", "depth": 0, "value": 9223372036854775807}
]}
```

## 3.5 Floating point values

Float values MUST be finite IEEE 754 binary64 values. A float leaf MUST be
a JSON number, including when its value is integral. Non-finite values have
no canonical Iris 0.1 encoding: `inf`, `-inf`, `nan`, and `NaN` are not
accepted float tokens, and an overflowing conversion such as `1e309` fails.
Strings naming those values MUST NOT stand in for float numbers.

Canonical float spelling MUST round-trip to the same binary64 value under
round-to-nearest, ties-to-even conversion. Choose the decimal significand
with the fewest significant digits that round-trips; among equally short
significands choose the decimal closest to the exact binary64 value, breaking
ties by an even final significand digit. Render that decimal in the shorter
of fixed notation and normalized scientific notation, preferring fixed
notation at equal lengths. Scientific notation has one digit before the
decimal point, lowercase `e`, no positive exponent sign, and no leading
exponent zeros. Omit unnecessary fractional trailing zeros and the decimal
point when there is no fraction. Positive zero MUST be `0`; negative zero
MUST be `-0.0` as an explicit exception. Thus float values may be written
`3`, `0.5`, or `1e3`; `type` distinguishes them from integers.

A `float` declaration produces float values even for integer-looking input.
A `num` array promotes all numeric leaves to float when a float is needed.
This example also preserves a value beyond the precision shown by Gecode's
human-readable summary; its dzn output retains the full value.

```iris example
id: data-numeric-types
operation: read
--- spec
integral: float
precise: float
thousand: float
x[3]: num
--- input
3.0 1.2345678901234567 1000
1 2.5 3
--- expected data
{"iris": "0.1", "entries": [
  {"name": "integral", "type": "float", "depth": 0, "value": 3},
  {"name": "precise", "type": "float", "depth": 0, "value": 1.2345678901234567},
  {"name": "thousand", "type": "float", "depth": 0, "value": 1e3},
  {"name": "x", "type": "float", "depth": 1, "value": [1, 2.5, 3]}
]}
```

Declaration offsets add zero even when no offset is written, so reading
`-0.0` by a float declaration yields positive zero. Float literals in `let`
and unary negation can retain or produce negative zero. Implementations
MUST preserve this distinction in values and canonical JSON.

```iris example
id: data-signed-zero
operation: read
--- spec
x: float
let z = -0.0
let y = -x
--- input
-0.0
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "float", "depth": 0, "value": 0},
  {"name": "z", "type": "float", "depth": 0, "value": -0.0},
  {"name": "y", "type": "float", "depth": 0, "value": -0.0}
]}
```

dzn uses its own numeric spelling, to be defined in appendix B,
`appendix-b-dzn.md` (planned). The following expected part is byte-exact dzn,
including its final LF, rather than canonical JSON.

```iris example
id: data-signed-zero-dzn
operation: dzn
--- spec
x: float
let z = -0.0
--- input
-0.0
--- expected dzn
x = 0.0;
z = -0.0;
```

## 3.6 Strings

Strings MUST be sequences of Unicode scalar values, encoded as UTF-8.
Implementations MUST NOT normalize Unicode. Canonical JSON MUST escape
quotation marks, backslashes, and control characters as JSON requires.
NUL is valid and is encoded as `\u0000`; see the
[byte example](02-notation.md#25-complete-examples).

A `word` or `rest` value that is not valid UTF-8 MUST be an error. This is a
deliberate [difference from Gecode 6.5.0](appendix-c-differences.md).
Whether validation also covers bytes outside string values is reserved for
chapter 4, `04-input.md` (planned; spec-003).

## 3.7 Canonical error document

An error MUST be encoded as a JSON object with exactly the six members
below. Encoders SHOULD present them in this order. JSON transport follows
§3.3; error strings follow the Unicode and escaping rules of §3.6.

| Member | Meaning |
|---|---|
| `code` | Stable error identifier, matching `E-` followed by uppercase ASCII letters/digits in hyphen-separated groups; the first group begins with a letter. |
| `message` | Main diagnostic text, without location prefix, excerpt, caret, execution-context suffix, or reader-spec suffix. Wording is recommended. |
| `input_location` | Input location object, or null if none is reported. |
| `spec_location` | Reader-spec location object, or null if none is reported. |
| `context` | Reported execution-context text after `while reading: `, or null when absent. |
| `record_path` | Failed-check witness path as defined below, or null for an error without such a witness. |

A location object MUST contain exactly `origin`, `line`, and `column`.
Origin is a string identifying the source, or null when unavailable. Lines
and columns are positive integers counted from 1, or null when unavailable.
Implementations MUST retain the distinction between the input location and
the reader-spec location and MUST NOT fabricate missing coordinates. A
parser that reports only a reader-spec line MUST report a null column.
Chapter 4 will specify how bytes and tabs affect input coordinates.

The context string preserves the reported outer-to-inner execution context,
including its human-facing iteration numbers. It MUST NOT be converted to
a failed-check path. For checks, `record_path` MUST contain the reported
witness's outermost-to-innermost record indices, followed by its
outermost-to-innermost comprehension element or record indices, all counted
from 0. Convert the reference's human-facing indices from 1 to 0. The path
describes the failure witness, not every surrounding execution frame;
implementations MUST NOT infer additional indices from context text.
A failed check without record or element indices has path `[]`. Other
errors have null path. Null and `[]` have distinct meanings.

The [error schema](../schema/error.schema.json) checks this shape. The
[error registry](errors.md) assigns codes, location availability, and error
ordering. The schema checks code syntax, not registry membership. Examples
use the logical origins `spec.iris` and `input.bin`.

### 3.7.1 Parse error

The parser reports a reader-spec line without a column; there is no input
location and no execution context.

```iris example
id: data-parse-error
operation: parse
--- spec
n: int
a[2: int
--- expected error
{
  "code": "E-SPEC-SYNTAX",
  "message": "unknown statement 'a'",
  "input_location": null,
  "spec_location": {"origin": "spec.iris", "line": 2, "column": null},
  "context": null,
  "record_path": null
}
```

### 3.7.2 Unknown parameter

This error identifies the reader spec but supplies neither line nor column.
The declaration's line MUST NOT be substituted.

```iris example
id: data-unknown-parameter
operation: read
--- spec
x: int
--- input
7
--- params
{"surprise": "1"}
--- expected error
{
  "code": "E-PARAM-UNKNOWN",
  "message": "the spec has no parameter 'surprise'",
  "input_location": null,
  "spec_location": {"origin": "spec.iris", "line": null, "column": null},
  "context": null,
  "record_path": null
}
```

### 3.7.3 Input error and execution context

The input error carries both an input position and a reader-spec line.

```iris example
id: data-input-error
operation: read
--- spec
x: int
--- input
oops
--- expected error
{
  "code": "E-EXPECTED-VALUE",
  "message": "expected int, found 'oops'",
  "input_location": {"origin": "input.bin", "line": 1, "column": 1},
  "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
  "context": "x",
  "record_path": null
}
```

An error while reading a repeat has execution context but no check witness.
The `2` in the context below remains human-facing text.

```iris example
id: data-repeat-input-error
operation: read
--- spec
repeat 2 as rows {
 a[2]: int
}
--- input
1 2
3 oops
--- expected error
{
  "code": "E-EXPECTED-VALUE",
  "message": "expected int, found 'oops'",
  "input_location": {"origin": "input.bin", "line": 2, "column": 3},
  "spec_location": {"origin": "spec.iris", "line": 2, "column": null},
  "context": "repeat [2 of 2] > a",
  "record_path": null
}
```

### 3.7.4 Nested check witnesses

The reference describes the inner record first in its message; the path
MUST place the outer record first. Record 2 of `jobs`, record 3 of `steps`
therefore becomes `[1, 2]`.

```iris example
id: data-nested-check
operation: read
--- spec
n: int
m: int
repeat n as jobs {
  repeat m as steps { x: int }
}
check x > 0 "positive"
--- input
2 3
1 2 3
4 5 0
--- expected error
{
  "code": "E-CHECK-FAILED",
  "message": "check failed for record 3 of 'steps' in record 2 of 'jobs': positive",
  "input_location": {"origin": "input.bin", "line": 3, "column": 5},
  "spec_location": {"origin": "spec.iris", "line": 6, "column": null},
  "context": null,
  "record_path": [1, 2]
}
```

Nested comprehension elements use the same outermost-first ordering. The
input coordinate here is the reference's reported end position, column 6;
it MUST NOT be moved to the zero token at column 5.

```iris example
id: data-element-check
operation: read
--- spec
a[2][3]: int
check all(r in a: all(x in r: x > 0)) "positive"
--- input
1 2 3
4 5 0
--- expected error
{
  "code": "E-CHECK-FAILED",
  "message": "check failed for element 3 of 'r' in element 2 of 'a': positive",
  "input_location": {"origin": "input.bin", "line": 2, "column": 6},
  "spec_location": {"origin": "spec.iris", "line": 2, "column": null},
  "context": null,
  "record_path": [1, 2]
}
```

## 3.8 Error comparison

Conformance MUST compare exactly the code, the primary line/column pair,
and `record_path`. Select the primary location once: use `input_location`
when non-null, otherwise `spec_location` when non-null, otherwise the pair
`(null, null)`. Compare the selected object's line and column, including
nulls. A missing input column MUST NOT be filled from the reader-spec
location.

Origins, message wording, context text, and secondary reader-spec coordinates
are not compared. The parse example's primary pair is `(2, null)`; the
unknown-parameter example's is `(null, null)`; the ordinary input example's
is `(1, 1)`. Conformance-001 must implement this projection rather than
expect flat `line` and `column` members.

## 3.9 Schema validation

Both schemas use JSON Schema Draft 2020-12 and internal references only.
They reject missing or extra object members. The data schema checks the
version, value types, nonnegative depth, scalar versus array at the root,
type-directed leaves, and signed 64-bit integer bounds. The error schema
checks code syntax, nullable locations and context, positive coordinates,
and nullable arrays of nonnegative path indices.

Schema validation alone is insufficient. Implementations MUST additionally
check unique entry names, exact declared depth at every existing leaf,
finite binary64 float values, Unicode scalar strings, and canonical number
spelling. JSON Schema's mathematical integer type does not enforce integer
token spelling; `1.0` can pass that check. An empty array still requires its
declared type and depth even when no leaves exist. The schemas impose no
artificial maximum depth. Error-code membership, coordinate availability,
and the meaning of a witness path require the rules of the relevant chapter.
