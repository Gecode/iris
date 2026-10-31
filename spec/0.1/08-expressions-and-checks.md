# 8 Expressions, checks, and record names

This chapter defines expression evaluation and checks. Expression syntax and
precedence remain in [§5.10](05-reader-specs.md#510-expression-syntax-and-precedence)
and [grammar.ebnf](grammar.ebnf). Values follow [chapter 3](03-data-model.md);
frames, columns, and logical records follow [chapter 7](07-structure.md).
Examples use [chapter 2's transport](02-notation.md) and complete canonical
JSON documents, checked by the [data](../schema/data.schema.json) and
[error](../schema/error.schema.json) schemas.

## 8.1 Values and value lookup

An expression MUST produce a scalar or array value, except where §8.12
permits a record or record-list denotation. Ordinary name lookup MUST search
active comprehension bindings first, innermost first, then the current value
frame and successive enclosing frames. A syntactic definition elsewhere in
the reader spec MUST NOT make an unexecuted definition available. A reached
unavailable name MUST report `E-NAME-UNDEFINED`.

```iris example
id: expressions-lookup-before-definition
operation: read
--- spec no-final-newline
let v=missing let missing=1
--- input
--- expected error
{"code": "E-NAME-UNDEFINED", "message": "'missing' is not defined yet", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

Operands MUST be evaluated from left to right unless a rule below specifies
short circuit. Both operand expressions of ordinary binary arithmetic and
comparison MUST be evaluated before validating their types. Here the right
operand's undefined name precedes the left operand's arithmetic type error.

```iris example
id: expressions-operand-before-type
operation: read
--- spec no-final-newline
let v=1.0+missing
--- input
--- expected error
{"code": "E-NAME-UNDEFINED", "message": "'missing' is not defined yet", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

## 8.2 Redefinition

A value definition MUST insert or replace its name only in the current
frame, including its value type and depth. Replacement MUST retain the
entry's position under [§3.1](03-data-model.md#31-entries-and-definition-order).
If/else and sections bodies introduce no frame. A `let` evaluates its
right-hand side before replacing its name, as specified in
[§7.9](07-structure.md#79-conditional-execution-and-let).

```iris example
id: expressions-lookup-redefine
operation: read
--- spec no-final-newline
let x=1 if 1 {let y=x let x="s"} let z=x
--- input
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "string", "depth": 0, "value": "s"},
  {"name": "y", "type": "int", "depth": 0, "value": 1},
  {"name": "z", "type": "string", "depth": 0, "value": "s"}
]}
```

Each repeat iteration MUST begin with a fresh local frame. Until a local
definition executes, lookup can find an enclosing value. After the repeat,
its completed columns replace enclosing entries under
[§7.1](07-structure.md#71-statement-execution-and-frames) and
[§7.5](07-structure.md#75-column-construction-and-missing-rows).
The example distinguishes enclosing lookup, local replacement, and lookup
of the published column.

```iris example
id: expressions-lookup-frame
operation: read
--- spec no-final-newline
let x=9 repeat 2 {let before=x x:int let after=x} let final=x
--- input no-final-newline
1 2
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "int", "depth": 1, "value": [1, 2]},
  {"name": "before", "type": "int", "depth": 1, "value": [9, 9]},
  {"name": "after", "type": "int", "depth": 1, "value": [1, 2]},
  {"name": "final", "type": "int", "depth": 1, "value": [1, 2]}
]}
```

## 8.3 Truth conversion

Conditions MUST use this conversion, without flattening arrays:

| Value | False exactly when |
|---|---|
| int scalar | Equal to 0 |
| float scalar | Equal to 0.0; both signed zeros are false |
| string scalar | Empty |
| Array of any value type or depth | Its outer length is 0 |

All other values MUST be true. Boolean results MUST be int scalars 0 or 1;
there is no separate boolean value type. An array containing one empty row
is true, although it has no scalar leaves.

```iris example
id: expressions-truth
operation: read
--- spec no-final-newline
a[0]:int b[1][0]:int let v=!a let w=!!b let f=!-0.0 let s=!"" let t=!!"0" let k=!!-3
--- input
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 1, "value": []},
  {"name": "b", "type": "int", "depth": 2, "value": [[]]},
  {"name": "v", "type": "int", "depth": 0, "value": 1},
  {"name": "w", "type": "int", "depth": 0, "value": 1},
  {"name": "f", "type": "int", "depth": 0, "value": 1},
  {"name": "s", "type": "int", "depth": 0, "value": 1},
  {"name": "t", "type": "int", "depth": 0, "value": 1},
  {"name": "k", "type": "int", "depth": 0, "value": 1}
]}
```

## 8.4 Integer arithmetic

Every binary arithmetic operator MUST require two integer scalars; unary
minus MUST require an integer scalar or the float scalar allowed by §8.5.
There is no implicit conversion of strings, arrays, or floats to integers.
Let L = -9223372036854775808 and M = 9223372036854775807. Implementations
MUST apply the following rules without wrapping or floating point
approximation:

| Operation | Required result or failure |
|---|---|
| `x + y`, `x - y`, `x * y` | Exact mathematical sum, difference, or product when in [L, M]; otherwise `E-INTEGER-OVERFLOW`, for every sign combination. |
| Integer `-x` | Exact negation, except L gives `E-INTEGER-OVERFLOW`. |
| `x / 0`, `x % 0` | `E-DIVISION-ZERO`. |
| `L / -1`, `L % -1` | `E-INTEGER-OVERFLOW`, including remainder despite its mathematical value being zero. |
| Other `x / y` | Quotient truncated toward zero. |
| Other `x % y` | Mathematical remainder `x - trunc(x/y) * y`; a nonzero remainder has the dividend's sign. This identity does not evaluate intermediate expressions. |
| `x ^ y` with y < 0 | `E-EXPONENT`, including bases 0 and 1. |
| `x ^ 0` | 1, including `0 ^ 0` and `L ^ 0`; both operands are still evaluated. |
| `x ^ y` with y > 0 | Exact power if in [L, M]; otherwise `E-INTEGER-OVERFLOW`. |
| Boundary products and powers | `L * 1 = L`, `L * 0 = 0`, `L ^ 1 = L`, `(-2) ^ 63 = L`. Bases 0, 1, and -1 permit exponents through M with their mathematical results. |
| Ordinary and comprehension `sum` | Checked additions starting at 0, in argument/leaf or domain order (§§8.8, 8.10); the first intermediate overflow fails even if later terms would cancel it. |

Literal recognition remains [§5.5](05-reader-specs.md#55-numbers)'s rule:
the minimum integer cannot be spelled directly as a negative literal.
[§5.10](05-reader-specs.md#510-expression-syntax-and-precedence) determines
which operation executes first; `-2^63` overflows in the power before
negation, while `(-2)^63` succeeds.

```iris example
id: expressions-integer-results
operation: read
--- spec
let add=7+3 let subtract=7-3 let product=-7*3
let quotient=-7/3 let negative_divisor=7/-3 let both_divisors=-7/-3
let remainder=-7%3 let negative_modulus=7%-3 let both_negative=-7%-3
--- input
--- expected data
{"iris": "0.1", "entries": [
  {"name": "add", "type": "int", "depth": 0, "value": 10},
  {"name": "subtract", "type": "int", "depth": 0, "value": 4},
  {"name": "product", "type": "int", "depth": 0, "value": -21},
  {"name": "quotient", "type": "int", "depth": 0, "value": -2},
  {"name": "negative_divisor", "type": "int", "depth": 0, "value": -2},
  {"name": "both_divisors", "type": "int", "depth": 0, "value": 2},
  {"name": "remainder", "type": "int", "depth": 0, "value": -1},
  {"name": "negative_modulus", "type": "int", "depth": 0, "value": 1},
  {"name": "both_negative", "type": "int", "depth": 0, "value": -1}
]}
```

```iris example
id: expressions-power-results
operation: read
--- spec
let zero=0^0 let minimum=(-2)^63 let identity=(-9223372036854775807-1)^1
let minimum_zero=(-9223372036854775807-1)^0
let one=1^9223372036854775807 let minus_one=(-1)^9223372036854775807
let zero_large=0^9223372036854775807
--- input
--- expected data
{"iris": "0.1", "entries": [
  {"name": "zero", "type": "int", "depth": 0, "value": 1},
  {"name": "minimum", "type": "int", "depth": 0, "value": -9223372036854775808},
  {"name": "identity", "type": "int", "depth": 0, "value": -9223372036854775808},
  {"name": "minimum_zero", "type": "int", "depth": 0, "value": 1},
  {"name": "one", "type": "int", "depth": 0, "value": 1},
  {"name": "minus_one", "type": "int", "depth": 0, "value": -1},
  {"name": "zero_large", "type": "int", "depth": 0, "value": 0}
]}
```

The following failures cover both bounds, negation, zero divisors,
negative exponents, power overflow, and intermediate aggregate overflow.

```iris example
id: expressions-add-overflow
operation: read
--- spec no-final-newline
let v=9223372036854775807+1
--- input
--- expected error
{"code": "E-INTEGER-OVERFLOW", "message": "integer overflow in '+'", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-addition-underflow
operation: read
--- spec no-final-newline
let v=(-9223372036854775807-1)+(-1)
--- input
--- expected error
{"code": "E-INTEGER-OVERFLOW", "message": "integer overflow in '+'", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-subtract-overflow
operation: read
--- spec no-final-newline
let v=9223372036854775807-(-1)
--- input
--- expected error
{"code": "E-INTEGER-OVERFLOW", "message": "integer overflow in '-'", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-subtract-underflow
operation: read
--- spec no-final-newline
let v=(-9223372036854775807-1)-1
--- input
--- expected error
{"code": "E-INTEGER-OVERFLOW", "message": "integer overflow in '-'", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-multiply-overflow
operation: read
--- spec no-final-newline
let v=9223372036854775807*2
--- input
--- expected error
{"code": "E-INTEGER-OVERFLOW", "message": "integer overflow in '*'", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-multiply-underflow
operation: read
--- spec no-final-newline
let v=(-9223372036854775807-1)*2
--- input
--- expected error
{"code": "E-INTEGER-OVERFLOW", "message": "integer overflow in '*'", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-minimum-div-minus-one
operation: read
--- spec no-final-newline
let v=(-9223372036854775807-1)/-1
--- input
--- expected error
{"code": "E-INTEGER-OVERFLOW", "message": "integer overflow in '/'", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-minimum-mod-minus-one
operation: read
--- spec no-final-newline
let v=(-9223372036854775807-1)%-1
--- input
--- expected error
{"code": "E-INTEGER-OVERFLOW", "message": "integer overflow in '%'", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-negate-minimum
operation: read
--- spec no-final-newline
let v=-(-9223372036854775807-1)
--- input
--- expected error
{"code": "E-INTEGER-OVERFLOW", "message": "integer overflow in '-'", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-div-zero
operation: read
--- spec no-final-newline
let v=1/0
--- input
--- expected error
{"code": "E-DIVISION-ZERO", "message": "division by zero", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-mod-zero
operation: read
--- spec no-final-newline
let v=1%0
--- input
--- expected error
{"code": "E-DIVISION-ZERO", "message": "division by zero", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-negative-exponent
operation: read
--- spec no-final-newline
let v=2^-1
--- input
--- expected error
{"code": "E-EXPONENT", "message": "negative exponent in '^'", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-power-overflow
operation: read
--- spec no-final-newline
let v=2^63
--- input
--- expected error
{"code": "E-INTEGER-OVERFLOW", "message": "integer overflow in '^'", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-power-precedence-overflow
operation: read
--- spec no-final-newline
let v=-2^63
--- input
--- expected error
{"code": "E-INTEGER-OVERFLOW", "message": "integer overflow in '^'", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-sum-intermediate-overflow
operation: read
--- spec no-final-newline
let v=sum(9223372036854775807,1,-1)
--- input
--- expected error
{"code": "E-INTEGER-OVERFLOW", "message": "integer overflow in 'sum'", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

## 8.5 Floating point and mixed numeric values

Iris 0.1 has no binary float arithmetic. All of `+`, `-`, `*`, `/`, `%`,
and `^` MUST require integer scalars, including when a float is integral
or both operands are floats. Violations MUST report `E-INTEGER-TYPE` after
evaluating both operands. Thus float division by zero is a type failure.

Unary minus MUST accept a float scalar, retain its type, and flip its sign,
including signed zero. Float arrays MUST NOT be negated. The
[finite binary64 invariant](03-data-model.md#35-floating-point-values)
continues to hold: conversions reject non-finite values, and negation of a
finite float remains finite. Reading and offset conversion remain in
[chapter 6](06-types-and-values.md#67-offsets-and-evaluation-order).

```iris example
id: expressions-float-results
operation: read
--- spec
let precise=-1.2345678901234567 let negative_zero=-0.0 let positive_zero=-negative_zero
--- input
--- expected data
{"iris": "0.1", "entries": [
  {"name": "precise", "type": "float", "depth": 0, "value": -1.2345678901234567},
  {"name": "negative_zero", "type": "float", "depth": 0, "value": -0.0},
  {"name": "positive_zero", "type": "float", "depth": 0, "value": 0}
]}
```

```iris example
id: expressions-float-add
operation: read
--- spec no-final-newline
let v=1.0+2.0
--- input
--- expected error
{"code": "E-INTEGER-TYPE", "message": "'+' needs integers", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-mixed-add
operation: read
--- spec no-final-newline
let v=1+2.0
--- input
--- expected error
{"code": "E-INTEGER-TYPE", "message": "'+' needs integers", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-float-div-zero
operation: read
--- spec no-final-newline
let v=1.0/0.0
--- input
--- expected error
{"code": "E-INTEGER-TYPE", "message": "'/' needs integers", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

## 8.6 Comparison

All six comparison operators MUST require scalar operands, including when
arrays are empty; arrays MUST report `E-COMPARISON-TYPE`. Each comparison
MUST return an int scalar 0 or 1 according to these rules:

| Operands | Comparison |
|---|---|
| int and int | Compare exact signed 64-bit values. |
| Two numeric scalars, at least one float | Convert any integer operand to binary64, then compare the binary64 values. |
| Two strings under `==` or `!=` | Compare exact contents without Unicode normalization. |
| String and numeric scalar under `==` or `!=` | Unequal: `==` returns 0 and `!=` returns 1. |
| Either operand a string under `<`, `>`, `<=`, or `>=` | `E-COMPARISON-TYPE`. |

```iris example
id: expressions-comparison-results
operation: read
--- spec
let exact=9007199254740993==9007199254740992
let rounded=9007199254740993==9007199254740992.0
let same="a"=="a" let mixed_equal="1"==1 let mixed_unequal="1"!=1
let unicode="é"=="é"
--- input
--- expected data
{"iris": "0.1", "entries": [
  {"name": "exact", "type": "int", "depth": 0, "value": 0},
  {"name": "rounded", "type": "int", "depth": 0, "value": 1},
  {"name": "same", "type": "int", "depth": 0, "value": 1},
  {"name": "mixed_equal", "type": "int", "depth": 0, "value": 0},
  {"name": "mixed_unequal", "type": "int", "depth": 0, "value": 1},
  {"name": "unicode", "type": "int", "depth": 0, "value": 0}
]}
```

```iris example
id: expressions-string-order
operation: read
--- spec no-final-newline
let v="a"<"b"
--- input
--- expected error
{"code": "E-COMPARISON-TYPE", "message": "'<' compares numbers", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-array-comparison
operation: read
--- spec no-final-newline
a[0]:int let v=a==a
--- input
--- expected error
{"code": "E-COMPARISON-TYPE", "message": "'==' compares single values, not arrays", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

## 8.7 Boolean operators

`!` MUST apply §8.3's truth conversion and return its inverse as int.
`&&` MUST evaluate its left operand first, then evaluate the right only
when the left is true. `||` MUST evaluate the right only when the left is
false. Both MUST return int 0 or 1, rather than an operand value.
Grouping follows [§5.10](05-reader-specs.md#510-expression-syntax-and-precedence).

```iris example
id: expressions-boolean-results
operation: read
--- spec
let yes=1||missing let no=0&&(1/0)
--- input
--- expected data
{"iris": "0.1", "entries": [
  {"name": "yes", "type": "int", "depth": 0, "value": 1},
  {"name": "no", "type": "int", "depth": 0, "value": 0}
]}
```

Runtime short circuit MUST NOT suppress parse-time validation or record-scope
resolution. The [unreachable cross-repeat example](#812-record-names-and-field-access)
therefore fails before evaluation.

## 8.8 Functions

Implementations MUST recognize exactly these ordinary functions. Every
result is an int scalar. A record list below is a structural denotation,
not a stored value (§8.12).

| Form | Signature and result |
|---|---|
| `defined(name)` | Exactly one argument parsed as a name expression; 1 if its value binding/definition is available, or its `as` record list has been established, otherwise 0. It MUST NOT evaluate an unavailable ordinary name. |
| `size(array)`, `count(array)` | Exactly one array argument, of any value type/depth; return its outer length. |
| `size(record-list)`, `count(record-list)` | Exactly one record-list argument; count logical records, including those with no ordinary fields. |
| `min(v1, ..., vn)`, n ≥ 1 | Minimum over all integer leaves of all arguments at any depth; no leaves gives `E-FUNCTION-EMPTY`. |
| `max(v1, ..., vn)`, n ≥ 1 | Maximum with the same leaf and empty rules. |
| `sum(v1, ..., vn)`, n ≥ 1 | Sum all integer leaves, starting at 0, using §8.4's checked addition. No leaves gives 0. |
| `distinct(v1, ..., vn)`, n ≥ 1 | 1 exactly when all integer leaves across all arguments differ. Zero or one leaf gives 1. |

For aggregates, arguments MUST be evaluated and validated from left to
right; leaves MUST be visited with the last array level varying fastest.
A nonempty float or string argument MUST report `E-FUNCTION-TYPE`. Empty
arrays of those types MUST contribute no leaves and MUST be accepted.
`distinct` MUST evaluate all arguments before deciding whether duplicates
exist; an argument failure still stops evaluation. Scalar arguments to
`size` or `count` MUST report `E-FUNCTION-TYPE`.

The examples show flattening versus outer length, empty-array identities,
and min/max ignoring empty arrays when an integer leaf exists.

```iris example
id: expressions-functions
operation: read
--- spec no-final-newline
a[2][2]:int let lo=min(a,0) let hi=max(a,7) let total=sum(a,5) let yes=distinct(a) let no=distinct(a,2) let n=size(a) let c=count(a)
--- input no-final-newline
1 2 3 4
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 2, "value": [[1, 2], [3, 4]]},
  {"name": "lo", "type": "int", "depth": 0, "value": 0},
  {"name": "hi", "type": "int", "depth": 0, "value": 7},
  {"name": "total", "type": "int", "depth": 0, "value": 15},
  {"name": "yes", "type": "int", "depth": 0, "value": 1},
  {"name": "no", "type": "int", "depth": 0, "value": 0},
  {"name": "n", "type": "int", "depth": 0, "value": 2},
  {"name": "c", "type": "int", "depth": 0, "value": 2}
]}
```

```iris example
id: expressions-function-empties
operation: read
--- spec no-final-newline
a[0]:int f[0]:float s[0]:word let v=sum(a,f,s) let w=distinct(a,f,s) let n=size(a) let k=count(a)
--- input
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 1, "value": []},
  {"name": "f", "type": "float", "depth": 1, "value": []},
  {"name": "s", "type": "string", "depth": 1, "value": []},
  {"name": "v", "type": "int", "depth": 0, "value": 0},
  {"name": "w", "type": "int", "depth": 0, "value": 1},
  {"name": "n", "type": "int", "depth": 0, "value": 0},
  {"name": "k", "type": "int", "depth": 0, "value": 0}
]}
```

```iris example
id: expressions-function-empty-ignored
operation: read
--- spec no-final-newline
f[0]:float s[0]:word let lo=min(f,s,7) let hi=max(f,s,-2) let n=distinct(f,s,1)
--- input
--- expected data
{"iris": "0.1", "entries": [
  {"name": "f", "type": "float", "depth": 1, "value": []},
  {"name": "s", "type": "string", "depth": 1, "value": []},
  {"name": "lo", "type": "int", "depth": 0, "value": 7},
  {"name": "hi", "type": "int", "depth": 0, "value": -2},
  {"name": "n", "type": "int", "depth": 0, "value": 1}
]}
```

```iris example
id: expressions-min-empty
operation: read
--- spec no-final-newline
a[0]:float let v=min(a)
--- input
--- expected error
{"code": "E-FUNCTION-EMPTY", "message": "'min' of no integers", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-max-empty
operation: read
--- spec no-final-newline
a[0]:word let v=max(a)
--- input
--- expected error
{"code": "E-FUNCTION-EMPTY", "message": "'max' of no integers", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-function-float
operation: read
--- spec no-final-newline
let v=sum(1.0)
--- input
--- expected error
{"code": "E-FUNCTION-TYPE", "message": "'sum' takes integers", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-size-scalar
operation: read
--- spec no-final-newline
let v=size(1)
--- input
--- expected error
{"code": "E-FUNCTION-TYPE", "message": "'size' takes an array", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-distinct-eager
operation: read
--- spec no-final-newline
let v=distinct(1,1,missing)
--- input
--- expected error
{"code": "E-NAME-UNDEFINED", "message": "'missing' is not defined yet", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

Invalid arity, unknown functions, ordinary calls to `all` or `any`, and a
`defined` argument other than a parsed name MUST report `E-SPEC-FUNCTION`
during parsing, including in unreachable bodies. Parentheses around a
name retain its name-expression form. `defined` is not general error
recovery. Direct names in `size` and `defined` have the whole-array rule in
§8.12; `count` does not.

```iris example
id: expressions-sum-no-args
operation: parse
--- spec no-final-newline
let v=sum()
--- expected error
{"code": "E-SPEC-FUNCTION", "message": "wrong arguments for function 'sum'", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-defined-expression
operation: parse
--- spec no-final-newline
let v=defined(1)
--- expected error
{"code": "E-SPEC-FUNCTION", "message": "wrong arguments for function 'defined'", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-all-normal-call
operation: parse
--- spec no-final-newline
let v=all(1)
--- expected error
{"code": "E-SPEC-FUNCTION", "message": "'all' needs the form all(x in a: e)", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-unknown-function
operation: parse
--- spec no-final-newline
let v=sqrt(1)
--- expected error
{"code": "E-SPEC-FUNCTION", "message": "unknown function 'sqrt'", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

## 8.9 Indexing

An array index MUST select an outer element, counted from 0, retaining its
value type and reducing depth by one. Rows MAY be ragged; each successive
index MUST use the selected row's own length.

```iris example
id: expressions-ragged-index
operation: read
--- spec no-final-newline
repeat 2 {list a:int until eol} let row=a[1] let v=a[1][0]
--- input
1 2
3
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 2, "value": [[1, 2], [3]]},
  {"name": "row", "type": "int", "depth": 1, "value": [3]},
  {"name": "v", "type": "int", "depth": 0, "value": 3}
]}
```

Implementations MUST evaluate the base first, then require an array before
evaluating the index. A scalar base, including a string, MUST report
`E-INDEX-SCALAR`. An index other than an integer scalar MUST report
`E-INDEX-TYPE`; an index below 0 or at least the outer length MUST report
`E-INDEX-RANGE`. Record-list indexing obeys the same integer and range
requirements but produces a record denotation (§8.12).

```iris example
id: expressions-index-negative
operation: read
--- spec no-final-newline
a[1]:int let v=a[-1]
--- input no-final-newline
5
--- expected error
{"code": "E-INDEX-RANGE", "message": "index -1 is out of range for 'a' (size 1, indices start at 0)", "input_location": {"origin": "input.bin", "line": 1, "column": 2}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-index-high
operation: read
--- spec no-final-newline
a[1]:int let v=a[1]
--- input no-final-newline
5
--- expected error
{"code": "E-INDEX-RANGE", "message": "index 1 is out of range for 'a' (size 1, indices start at 0)", "input_location": {"origin": "input.bin", "line": 1, "column": 2}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-index-float
operation: read
--- spec no-final-newline
a[1]:int let v=a[0.0]
--- input no-final-newline
5
--- expected error
{"code": "E-INDEX-TYPE", "message": "an index must be an integer", "input_location": {"origin": "input.bin", "line": 1, "column": 2}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-index-string
operation: read
--- spec no-final-newline
let v="abc"[0]
--- input
--- expected error
{"code": "E-INDEX-SCALAR", "message": "only arrays can be indexed, not the array", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-index-base-first
operation: read
--- spec no-final-newline
let v=1[missing]
--- input
--- expected error
{"code": "E-INDEX-SCALAR", "message": "only arrays can be indexed, not the array", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-record-list-index-range
operation: read
--- spec no-final-newline
repeat 1 as rows {x:int} let v=rows[1].x
--- input no-final-newline
1
--- expected error
{"code": "E-INDEX-RANGE", "message": "index 1 is out of range for 'rows' (size 1, indices start at 0)", "input_location": {"origin": "input.bin", "line": 1, "column": 2}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

## 8.10 Comprehensions and bindings

A comprehension MUST evaluate its domain once, then visit outer elements
in increasing index order. An array element is a scalar or row value; an
explicit record-list element is a record denotation requiring field access.
The binding MUST exist only in the body, not in its own domain or after
the comprehension.

| Form | Body requirement; int result | Empty domain |
|---|---|---|
| `all(x in a: e)` | Truth conversion; 1 exactly when every body is true. Stop at the first false body. | 1 |
| `any(x in a: e)` | Truth conversion; 1 exactly when some body is true. Stop at the first true body. | 0 |
| `sum(x in a: e)` | Integer scalar; checked sum in domain order starting at 0. | 0 |
| `count(x in a: e)` | Truth conversion; number of true bodies. Visit every element unless evaluation fails. | 0 |

An empty domain MUST NOT evaluate the body. A scalar domain MUST report
`E-FUNCTION-TYPE`; a noninteger or array body of comprehension `sum` MUST
report `E-INTEGER-TYPE`. Ordinary `sum` flattens arrays (§8.8);
comprehension `sum` requires each body result itself to be an integer scalar.

```iris example
id: expressions-comprehension-results
operation: read
--- spec no-final-newline
a[2][2]:int let s=sum(r in a:sum(r)) let c=count(r in a:r) let yes=all(r in a:any(x in r:x)) let no=any(r in a:0) let out=defined(r)
--- input no-final-newline
1 2 3 4
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 2, "value": [[1, 2], [3, 4]]},
  {"name": "s", "type": "int", "depth": 0, "value": 10},
  {"name": "c", "type": "int", "depth": 0, "value": 2},
  {"name": "yes", "type": "int", "depth": 0, "value": 1},
  {"name": "no", "type": "int", "depth": 0, "value": 0},
  {"name": "out", "type": "int", "depth": 0, "value": 0}
]}
```

```iris example
id: expressions-comprehension-empty
operation: read
--- spec no-final-newline
a[0]:float let yes=all(x in a:missing) let no=any(x in a:missing) let s=sum(x in a:missing) let c=count(x in a:missing)
--- input
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "float", "depth": 1, "value": []},
  {"name": "yes", "type": "int", "depth": 0, "value": 1},
  {"name": "no", "type": "int", "depth": 0, "value": 0},
  {"name": "s", "type": "int", "depth": 0, "value": 0},
  {"name": "c", "type": "int", "depth": 0, "value": 0}
]}
```

```iris example
id: expressions-comprehension-scalar
operation: read
--- spec no-final-newline
let v=all(x in 1:1)
--- input
--- expected error
{"code": "E-FUNCTION-TYPE", "message": "'all(x in ...)' needs an array", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-comprehension-sum-float
operation: read
--- spec no-final-newline
a[1]:float let v=sum(x in a:x)
--- input no-final-newline
1
--- expected error
{"code": "E-INTEGER-TYPE", "message": "'sum' needs integers", "input_location": {"origin": "input.bin", "line": 1, "column": 2}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-comprehension-sum-overflow
operation: read
--- spec no-final-newline
a[2]:int let v=sum(x in a:x)
--- input no-final-newline
9223372036854775807 1
--- expected error
{"code": "E-INTEGER-OVERFLOW", "message": "integer overflow in 'sum'", "input_location": {"origin": "input.bin", "line": 1, "column": 22}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-check-all-short-circuit
operation: read
--- spec no-final-newline
a[2]:int let v=all(x in a:x && missing)
--- input no-final-newline
0 1
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 1, "value": [0, 1]},
  {"name": "v", "type": "int", "depth": 0, "value": 0}
]}
```

```iris example
id: expressions-check-any-short-circuit
operation: read
--- spec no-final-newline
a[2]:int let v=any(x in a:x||missing)
--- input no-final-newline
1 0
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 1, "value": [1, 0]},
  {"name": "v", "type": "int", "depth": 0, "value": 1}
]}
```

Bindings MUST obey this table during parsing/resolution, including in
unreachable syntax. Ordinary name rules also apply under
[§5.4](05-reader-specs.md#54-names).

| Situation | Rule | Example below |
|---|---|---|
| Binding's own domain | The new binding is unavailable. | `binding-before-domain` |
| Ordinary value definition of the same spelling anywhere | Reject with `E-SPEC-NAME-CONFLICT`. | `binding-global-conflict` |
| Enclosing comprehension uses the spelling | Reject shadowing with `E-SPEC-NAME-CONFLICT`. | `binding-nested-conflict` |
| Binding is `_` or has reserved `gecode.` prefix | Reject with `E-SPEC-NAME`. | `binding-discard`; prefix rule in §5.4 |
| Explicit structural name has the spelling | Reject with `E-SPEC-NAME-CONFLICT`. | `binding-record-conflict` |
| Separate, nonnested comprehensions reuse the spelling | Permit. | `binding-reuse` |
| `defined` in the body | The binding is available. | `binding-defined` |
| Named type alone has the spelling | Permit the binding. | `binding-type-name` |

```iris example
id: expressions-binding-before-domain
operation: read
--- spec no-final-newline
a[1]:int let v=all(x in x:1)
--- input no-final-newline
1
--- expected error
{"code": "E-NAME-UNDEFINED", "message": "'x' is not defined yet", "input_location": {"origin": "input.bin", "line": 1, "column": 2}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-binding-global-conflict
operation: parse
--- spec no-final-newline
a[0]:int check all(x in a:1) if 0 {let x=1}
--- expected error
{"code": "E-SPEC-NAME-CONFLICT", "message": "'x' is bound by a comprehension but is also a name in the spec", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-binding-nested-conflict
operation: parse
--- spec no-final-newline
a[0]:int check all(x in a:all(x in a:1))
--- expected error
{"code": "E-SPEC-NAME-CONFLICT", "message": "'x' is already bound by an enclosing comprehension", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-binding-discard
operation: parse
--- spec no-final-newline
a[0]:int check all(_ in a:1)
--- expected error
{"code": "E-SPEC-NAME", "message": "'_' cannot be bound", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-binding-record-conflict
operation: parse
--- spec no-final-newline
repeat 0 as rows {x:int} check all(rows in rows:1)
--- expected error
{"code": "E-SPEC-NAME-CONFLICT", "message": "'rows' is bound by a comprehension but names records", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-binding-reuse
operation: read
--- spec no-final-newline
a[1]:int let v=sum(x in a:x)+sum(x in a:x)
--- input no-final-newline
2
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 1, "value": [2]},
  {"name": "v", "type": "int", "depth": 0, "value": 4}
]}
```

```iris example
id: expressions-binding-defined
operation: read
--- spec no-final-newline
a[1]:int let v=all(x in a:defined(x)) let w=defined(x)
--- input no-final-newline
3
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 1, "value": [3]},
  {"name": "v", "type": "int", "depth": 0, "value": 1},
  {"name": "w", "type": "int", "depth": 0, "value": 0}
]}
```

```iris example
id: expressions-binding-type-name
operation: read
--- spec no-final-newline
type t=int a[1]:int let v=all(t in a:t>0)
--- input no-final-newline
1
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 1, "value": [1]},
  {"name": "v", "type": "int", "depth": 0, "value": 1}
]}
```

## 8.11 Host constants

Implementations MUST supply exactly these host constants as int scalars:

| Name | Value |
|---|---:|
| `gecode.int_max` | 2147483646 |
| `gecode.int_min` | -2147483646 |

These are not the limits of Iris's 64-bit integers. An unknown constant
MUST report `E-SPEC-CONSTANT` during parsing. Definitions with the reserved
prefix MUST fail under [§5.4](05-reader-specs.md#54-names).

```iris example
id: expressions-constants
operation: read
--- spec no-final-newline
let hi=gecode.int_max let lo=gecode.int_min
--- input
--- expected data
{"iris": "0.1", "entries": [
  {"name": "hi", "type": "int", "depth": 0, "value": 2147483646},
  {"name": "lo", "type": "int", "depth": 0, "value": -2147483646}
]}
```

```iris example
id: expressions-constant-unknown
operation: parse
--- spec no-final-newline
let v=gecode.no
--- expected error
{"code": "E-SPEC-CONSTANT", "message": "unknown Gecode constant 'gecode.no' (known: gecode.int_max, gecode.int_min)", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

## 8.12 Record names and field access

Record structure and column construction remain in
[§§7.5–7.6](07-structure.md#75-column-construction-and-missing-rows).
Resolution MUST traverse syntax before execution, including definitions and
expressions in unreachable bodies and operands.

For a syntactic value definition d, let R(d) be its enclosing repeat
statements, outermost first. Let C be the repeat statements enclosing the
expression's statement. Let B(d,C) be R(d) with its longest common prefix
with C removed. Repeat identity, not iteration count, determines equality.
If/else, sections, and instances add no repeat levels.

For an unbound ordinary name n in a check, outside a whole-array position,
implementations MUST collect B(d,C) for every definition d of n anywhere in
the reader spec. All sequences MUST be identical, or resolution MUST report
`E-SPEC-CHECK-SCOPE`. An empty sequence means ordinary frame lookup; a
nonempty sequence means implicit quantification over those remaining repeat
levels. With no definition, n retains ordinary runtime lookup. Every lifted
sequence in a check MUST be a prefix of one deepest sequence, or resolution
MUST report `E-SPEC-CHECK-SCOPE`.

Definition collection MUST ignore the name `_`. A `store _` still creates
a runtime entry named `_`
([§7.4](07-structure.md#74-repeat-forms-and-termination)), but it is not a
definition for these judgments or for the structural-name conflicts below.
A check that reads `_` therefore uses ordinary runtime lookup.

The following table applies these judgments. Each cited example appears
below with id prefix `expressions-`; §8.2 supplies the current-frame example.

| Situation | Required interpretation | Example |
|---|---|---|
| Ordinary use outside a check | A completed repeated field is its whole column; no implicit record quantification in `let`, `if`, or counts. | `ordinary-outside` |
| Current iteration | Local value if available, otherwise enclosing lookup; no previous-iteration value. | `lookup-frame` (§8.2) |
| Direct ordinary name in `size` | Whole-array lookup; suppress implicit lifting. | `whole-vs-record` |
| Direct ordinary name after `in` | Whole domain; free names in the body can still lift. | `whole-vs-record`; `check-mixed-witness` (§8.13) |
| Direct ordinary name in `defined` | Availability of the whole name, without lifting. | `guard-absent` |
| Direct ordinary name in `count` | No exception: lift the field in a check. | `whole-vs-record` |
| Whole-array positions containing indexing or field access | The exception is shallow: resolve the base normally, so it can lift. | `size-index-not-whole`, `size-field-lifts` |
| Fields of the same repeat | Select corresponding fields of one record, not a Cartesian product. | `same-repeat` |
| Nested repeats | Visit each outer record's inner records with its own outer fields. | `nested-repeat` |
| Check inside an active outer repeat | Lift only the suffix beyond the active common prefix. | `nested-completed-current`; `check-context-path` (§8.13) |
| Empty completed records | Vacuous truth; do not evaluate a condition for nonexistent records. | `empty-records`, `check-zero-empty-lift` |
| Definitions in different completed repeats | Reject ambiguous definitions of one name or incompatible chains of different names. | `ambiguous-repeat`, `cross-repeat` |
| Name defined inside and outside a completed repeat | Reject implicit lifting; whole-array uses can remain valid. | `inside-outside`, `ambiguous-whole` |
| All definition suffixes empty | Ordinary lookup, including local checks with enclosing definitions of that spelling. | `inside-outside-local` |
| Multiple definitions in one repeat | Permit when remaining repeat sequences agree. | `branch-definitions` |
| Indexing a lifted scalar | Index that record's scalar and report `E-INDEX-SCALAR`. | `record-subscript` |
| Potential record collection not yet defined | No collection to traverse; a reached name fails, but a guard can skip it. | `absent-not-guarded`, `guard-absent`, `check-before-empty` |
| Unreachable operand | Static repeat-chain restrictions still apply. | `cross-repeat-unreachable` |
| `_` stored by `store _` in several repeats | Not a definition; ordinary runtime lookup, without lifting or `E-SPEC-CHECK-SCOPE`. | `store-underscore-lookup` |

```iris example
id: expressions-store-underscore-lookup
operation: read
--- spec no-final-newline
repeat until "END" store _ {} repeat 1 {repeat until "STOP" store _ {}} check _=="END"
--- input no-final-newline
END STOP
--- expected data
{"iris": "0.1", "entries": [
  {"name": "_", "type": "string", "depth": 0, "value": "END"}
]}
```

```iris example
id: expressions-ordinary-outside
operation: read
--- spec no-final-newline
repeat 2 {x:int} let total=sum(x) let first=x[0] if count(x)==2 {let yes=1}
--- input no-final-newline
3 4
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "int", "depth": 1, "value": [3, 4]},
  {"name": "total", "type": "int", "depth": 0, "value": 7},
  {"name": "first", "type": "int", "depth": 0, "value": 3},
  {"name": "yes", "type": "int", "depth": 0, "value": 1}
]}
```

```iris example
id: expressions-whole-vs-record
operation: read
--- spec no-final-newline
repeat 2 {a[3]:int} check size(a)==2 && count(a)==3 && sum(r in a:sum(r))==21 && defined(a)
--- input no-final-newline
1 2 3 4 5 6
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 2, "value": [[1, 2, 3], [4, 5, 6]]}
]}
```

```iris example
id: expressions-guard-absent
operation: read
--- spec no-final-newline
if 0 {repeat 2 {x:int}} check !defined(x)||x>0
--- input
--- expected data
{"iris": "0.1", "entries": []}
```

```iris example
id: expressions-absent-not-guarded
operation: read
--- spec no-final-newline
if 0 {repeat 2 {x:int}} check x>0
--- input
--- expected error
{"code": "E-NAME-UNDEFINED", "message": "'x' is not defined yet", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-size-index-not-whole
operation: read
--- spec no-final-newline
repeat 2 {a[3]:int} check size(a[0])==3
--- input no-final-newline
1 2 3 4 5 6
--- expected error
{"code": "E-FUNCTION-TYPE", "message": "'size' takes an array", "input_location": {"origin": "input.bin", "line": 1, "column": 12}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-size-field-lifts
operation: read
--- spec no-final-newline
repeat 2 as rows {each row a[3]:int} check size(row.a)==3
--- input no-final-newline
1 2 3 4 5 6
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 2, "value": [[1, 2, 3], [4, 5, 6]]}
]}
```

```iris example
id: expressions-same-repeat
operation: read
--- spec no-final-newline
repeat 2 {lo:int hi:int} check lo<=hi "window"
--- input no-final-newline
1 2 4 3
--- expected error
{"code": "E-CHECK-FAILED", "message": "check failed for record 2 of 'lo': window", "input_location": {"origin": "input.bin", "line": 1, "column": 5}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": [1]}
```

```iris example
id: expressions-nested-repeat
operation: read
--- spec no-final-newline
repeat 2 {k:int repeat 2 {x:int}} check x<k "limit"
--- input no-final-newline
5 1 2 3 1 4
--- expected error
{"code": "E-CHECK-FAILED", "message": "check failed for record 2 of record 2 of 'x': limit", "input_location": {"origin": "input.bin", "line": 1, "column": 11}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": [1, 1]}
```

```iris example
id: expressions-nested-completed-current
operation: read
--- spec no-final-newline
repeat 2 {repeat 2 {x:int} check size(x)==2 && x>0}
--- input no-final-newline
1 2 3 4
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "int", "depth": 2, "value": [[1, 2], [3, 4]]}
]}
```

```iris example
id: expressions-empty-records
operation: read
--- spec no-final-newline
repeat 0 {x:int} check x>0 && 0
--- input
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "int", "depth": 1, "value": []}
]}
```

```iris example
id: expressions-check-zero-empty-lift
operation: read
--- spec no-final-newline
repeat 0 {x:int} check 0&&x>0
--- input
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "int", "depth": 1, "value": []}
]}
```

```iris example
id: expressions-ambiguous-repeat
operation: parse
--- spec no-final-newline
repeat 1 {x:int} repeat 1 {x:int} check x>0
--- expected error
{"code": "E-SPEC-CHECK-SCOPE", "message": "'x' is defined in several repeats, or inside and outside a repeat, so a check here cannot range over its records", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-cross-repeat
operation: parse
--- spec no-final-newline
repeat 1 {x:int} repeat 1 {y:int} check x==y
--- expected error
{"code": "E-SPEC-CHECK-SCOPE", "message": "a check can only range over the records of one repeat, but 'x' and 'y' are defined in different repeats", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-inside-outside
operation: parse
--- spec no-final-newline
let x=1 repeat 1 {x:int} check x>0
--- expected error
{"code": "E-SPEC-CHECK-SCOPE", "message": "'x' is defined in several repeats, or inside and outside a repeat, so a check here cannot range over its records", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-ambiguous-whole
operation: read
--- spec no-final-newline
let x=1 repeat 1 {x:int} check size(x)==1 && defined(x) && all(t in x:t>0)
--- input no-final-newline
2
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "int", "depth": 1, "value": [2]}
]}
```

```iris example
id: expressions-inside-outside-local
operation: read
--- spec no-final-newline
let x=9 repeat 2 {x:int check x>0} let x=3
--- input no-final-newline
1 2
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "int", "depth": 0, "value": 3}
]}
```

```iris example
id: expressions-branch-definitions
operation: read
--- spec no-final-newline
repeat 2 {k:int if k {let x=1} else {let x=2}} check x>0
--- input no-final-newline
1 0
--- expected data
{"iris": "0.1", "entries": [
  {"name": "k", "type": "int", "depth": 1, "value": [1, 0]},
  {"name": "x", "type": "int", "depth": 1, "value": [1, 2]}
]}
```

```iris example
id: expressions-record-subscript
operation: read
--- spec no-final-newline
repeat 2 as rows {x:int} check x[0]==1
--- input no-final-newline
1 2
--- expected error
{"code": "E-INDEX-SCALAR", "message": "only arrays can be indexed, not 'x', which is one value of each record here; to index the records, use 'rows[0].x'", "input_location": {"origin": "input.bin", "line": 1, "column": 4}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-check-before-empty
operation: read
--- spec no-final-newline
check x>0 repeat 0 {x:int}
--- input
--- expected error
{"code": "E-NAME-UNDEFINED", "message": "'x' is not defined yet", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-cross-repeat-unreachable
operation: parse
--- spec no-final-newline
repeat 0 {x:int} repeat 0 {y:int} check 1||x==y
--- expected error
{"code": "E-SPEC-CHECK-SCOPE", "message": "a check can only range over the records of one repeat, but 'x' and 'y' are defined in different repeats", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

Explicit structural names MUST obey this second table. A record or record
list MUST NOT be treated as a stored value or JSON object.

| Situation | Required interpretation or failure | Example |
|---|---|---|
| `as rows` | Record list; permit size/count, indexing, comprehension domains, and `defined`. Count logical records even without fields or when empty. Availability begins when the repeat establishes its list. | `record-fields`, `record-defined-list`, `zero-record-list`, `defined-before-repeat` |
| `each row` inside its repeat | Current record; field access adds no automatic quantification over that repeat. | `record-fields` |
| `each row` outside its repeat | Permit only in checks, implicitly quantifying over the remaining repeat chain; otherwise `E-SPEC-RECORD-SCOPE`. | `each-outside`, `mixed-nested-each` |
| `as` name inside its own repeat | `E-SPEC-RECORD-SCOPE`, including size/defined and nested expressions. | `self-list` |
| Nested list outside its enclosing repeat | Reach it through an enclosing record, such as `job.steps` or `jobs[1].steps`; reject the unqualified name with `E-SPEC-RECORD-SCOPE`. | `nested-fields`, `nested-list-unqualified` |
| Selecting a nested list across an intermediate repeat | Require the intervening list name; a skipped level gives `E-SPEC-RECORD-SCOPE`. | `nested-unnamed-gap` |
| Field access | Require a record base and a syntactically defined field belonging to its repeat. A nested ordinary field can be a column within an outer record. | `nested-fields` |
| Ordinary value.field, record-list.field, unknown field | `E-SPEC-FIELD`. | `field-not-record`, `field-list`, `field-unknown` |
| Field spelling also defined outside its owning repeat | `E-SPEC-FIELD`, even if execution might appear unambiguous. | `field-outside-definition` |
| Outer record's unfinished inner field | `E-SPEC-RECORD-SCOPE` while an active inner repeat is reading it. | `field-incomplete` |
| Valid current-record field before execution | Static membership does not supply a value; a reached unavailable field gives `E-NAME-UNDEFINED`. | `current-record-before-field` |
| Record/list used as a value, record indexed again, record used as a domain | `E-SPEC-RECORD-VALUE`. | `record-not-value`, `list-not-value`, `record-not-indexable`, `record-not-domain` |
| Structural name repeated or overlapping a value definition, or identical `as` and `each` names | `E-SPEC-NAME-CONFLICT` globally, including unreachable syntax. | `record-name-repeated`, `record-name-conflict`, `record-name-same` |
| Structural name `_` with `store _` | Permit; `store _` is not a value definition. In an expression, `_` names the records. A repeated structural `_` still conflicts. | `store-underscore-records` |
| Structural name also a named type | Permit, under [§6.3](06-types-and-values.md#63-names-discarded-reads-and-replacement). | `as-type-name` |
| `defined(each-name)` | `E-SPEC-RECORD-VALUE`, including inside its repeat; a single record is not a value. | `record-defined-each`, `defined-each-inside` |

```iris example
id: expressions-store-underscore-records
operation: read
--- spec no-final-newline
repeat until "END" store _ as _ {} let n=size(_)
--- input no-final-newline
END
--- expected data
{"iris": "0.1", "entries": [
  {"name": "_", "type": "string", "depth": 0, "value": "END"},
  {"name": "n", "type": "int", "depth": 0, "value": 0}
]}
```

To evaluate an ordinary field selection, implementations MUST resolve the
record path, obtain the field's column through value lookup, then select
that path from the column. The result MAY be an array of nested fields.
An unavailable column MUST report `E-NAME-UNDEFINED`. If a selected record
path cannot be located in an available field column, implementations MUST
report `E-FIELD-UNDEFINED`. This is a conditional reference guard; no public
reproducer is known. Ordinary missing array rows follow
[§7.5](07-structure.md#75-column-construction-and-missing-rows).

```iris example
id: expressions-record-fields
operation: read
--- spec no-final-newline
repeat 2 as rows {each row x:int let y=row.x+1} check row.x<row.y let first=rows[0].x let total=sum(r in rows:r.y) let n=count(rows)
--- input no-final-newline
4 5
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "int", "depth": 1, "value": [4, 5]},
  {"name": "y", "type": "int", "depth": 1, "value": [5, 6]},
  {"name": "first", "type": "int", "depth": 0, "value": 4},
  {"name": "total", "type": "int", "depth": 0, "value": 11},
  {"name": "n", "type": "int", "depth": 0, "value": 2}
]}
```

```iris example
id: expressions-record-defined-list
operation: read
--- spec no-final-newline
repeat 0 as rows {x:int} check defined(rows) && size(rows)==0
--- input
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "int", "depth": 1, "value": []}
]}
```

```iris example
id: expressions-zero-record-list
operation: read
--- spec no-final-newline
repeat 0 as rows {x:int} let n=count(rows) let v=sum(r in rows:r.x)
--- input
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "int", "depth": 1, "value": []},
  {"name": "n", "type": "int", "depth": 0, "value": 0},
  {"name": "v", "type": "int", "depth": 0, "value": 0}
]}
```

```iris example
id: expressions-defined-before-repeat
operation: read
--- spec no-final-newline
let before=defined(rows) repeat 0 as rows {x:int} let after=defined(rows)
--- input
--- expected data
{"iris": "0.1", "entries": [
  {"name": "before", "type": "int", "depth": 0, "value": 0},
  {"name": "x", "type": "int", "depth": 1, "value": []},
  {"name": "after", "type": "int", "depth": 0, "value": 1}
]}
```

```iris example
id: expressions-each-outside
operation: parse
--- spec no-final-newline
repeat 1 {each row x:int} let v=row.x
--- expected error
{"code": "E-SPEC-RECORD-SCOPE", "message": "'row' is each record of a repeat, so only checks can use it outside that repeat", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-mixed-nested-each
operation: read
--- spec no-final-newline
repeat 2 as jobs {each job repeat 2 as steps {each step x:int}} check step.x>0
--- input no-final-newline
1 2 3 0
--- expected error
{"code": "E-CHECK-FAILED", "message": "check failed for record 2 of 'steps' in record 2 of 'jobs'", "input_location": {"origin": "input.bin", "line": 1, "column": 7}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": [1, 1]}
```

```iris example
id: expressions-self-list
operation: parse
--- spec no-final-newline
repeat 1 as rows {x:int let n=size(rows)}
--- expected error
{"code": "E-SPEC-RECORD-SCOPE", "message": "'rows' cannot be used inside its own repeat", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-nested-fields
operation: read
--- spec no-final-newline
repeat 2 as jobs {each job repeat 2 as steps {each step x:int}} check size(job.steps)==2 && all(s in job.steps:s.x>0) let v=jobs[1].steps[0].x let total=sum(j in jobs:sum(j.x))
--- input no-final-newline
1 2 3 4
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "int", "depth": 2, "value": [[1, 2], [3, 4]]},
  {"name": "v", "type": "int", "depth": 0, "value": 3},
  {"name": "total", "type": "int", "depth": 0, "value": 10}
]}
```

```iris example
id: expressions-nested-list-unqualified
operation: parse
--- spec no-final-newline
repeat 1 {each job repeat 1 as steps {x:int}} check size(steps)==1
--- expected error
{"code": "E-SPEC-RECORD-SCOPE", "message": "'steps' is a list in each record of the repeat around it; use 'job.steps'", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-nested-unnamed-gap
operation: parse
--- spec no-final-newline
repeat 1 as jobs {repeat 1 {repeat 1 as steps {x:int}}} let v=jobs[0].steps[0].x
--- expected error
{"code": "E-SPEC-RECORD-SCOPE", "message": "'steps' is inside a repeat without a list name in the records of 'jobs'; name that list with 'as' to reach it", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-field-not-record
operation: parse
--- spec no-final-newline
let x=1 let v=x.y
--- expected error
{"code": "E-SPEC-FIELD", "message": "only records have fields such as '.y'; to name each record of a repeat, write 'repeat n as nodes { each node ... }' and use 'node.y' or 'nodes[0].y'", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-field-list
operation: parse
--- spec no-final-newline
repeat 1 as rows {x:int} let v=rows.x
--- expected error
{"code": "E-SPEC-FIELD", "message": "'rows' is a list of records, so '.x' needs one of them: use 'rows[i].x', or name each record with 'each' in the repeat and use its field", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-field-unknown
operation: parse
--- spec no-final-newline
repeat 1 as rows {x:int} let v=rows[0].y
--- expected error
{"code": "E-SPEC-FIELD", "message": "the records of 'rows' have no field 'y' (they have x)", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-field-outside-definition
operation: parse
--- spec no-final-newline
let x=1 repeat 1 as rows {x:int} let v=rows[0].x
--- expected error
{"code": "E-SPEC-FIELD", "message": "'x' is defined inside and outside the repeat of 'rows', so it cannot be a field of its records", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-field-incomplete
operation: parse
--- spec no-final-newline
repeat 1 {each job repeat 1 {x:int let v=job.x}}
--- expected error
{"code": "E-SPEC-RECORD-SCOPE", "message": "the field 'x' cannot be used inside the repeat that reads it", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-current-record-before-field
operation: read
--- spec no-final-newline
let outside=9 repeat 1 {each row let v=row.a a:int}
--- input no-final-newline
4
--- expected error
{"code": "E-NAME-UNDEFINED", "message": "'a' is not defined yet", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": "repeat [1 of 1]", "record_path": null}
```

```iris example
id: expressions-record-not-value
operation: parse
--- spec no-final-newline
repeat 1 as rows {each row x:int} check row
--- expected error
{"code": "E-SPEC-RECORD-VALUE", "message": "a record of 'rows' is not a value; use one of its fields, as in 'row.x'", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-list-not-value
operation: parse
--- spec no-final-newline
repeat 1 as rows {x:int} let v=rows
--- expected error
{"code": "E-SPEC-RECORD-VALUE", "message": "'rows' is a list of records, not a value; use size(rows), an index, or a field of a record, as in 'rows[0].x'", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-record-not-indexable
operation: parse
--- spec no-final-newline
repeat 1 as rows {x:int} let v=rows[0][0]
--- expected error
{"code": "E-SPEC-RECORD-VALUE", "message": "a record of 'rows' cannot be indexed; use one of its fields, as in 'rows[0].x'", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-record-not-domain
operation: parse
--- spec no-final-newline
repeat 1 as rows {each row x:int} check all(t in row:1)
--- expected error
{"code": "E-SPEC-RECORD-VALUE", "message": "a record of 'rows' is not a list; use one of its fields", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-record-name-repeated
operation: parse
--- spec no-final-newline
repeat 1 as rows {x:int} repeat 1 as rows {y:int}
--- expected error
{"code": "E-SPEC-NAME-CONFLICT", "message": "'rows' already names records", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-record-name-conflict
operation: parse
--- spec no-final-newline
repeat 1 as rows {rows:int}
--- expected error
{"code": "E-SPEC-NAME-CONFLICT", "message": "'rows' names records, so it cannot also be defined", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-record-name-same
operation: parse
--- spec no-final-newline
repeat 1 as rows {each rows x:int}
--- expected error
{"code": "E-SPEC-NAME-CONFLICT", "message": "'rows' cannot name both the list and each record", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-as-type-name
operation: read
--- spec no-final-newline
type rows=int repeat 0 as rows {x:rows} let n=size(rows)
--- input
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "int", "depth": 1, "value": []},
  {"name": "n", "type": "int", "depth": 0, "value": 0}
]}
```

```iris example
id: expressions-record-defined-each
operation: parse
--- spec no-final-newline
repeat 0 {each row x:int} check defined(row)
--- expected error
{"code": "E-SPEC-RECORD-VALUE", "message": "a record of 'row' is not a value; use one of its fields, as in 'row.x'", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: expressions-defined-each-inside
operation: parse
--- spec no-final-newline
repeat 1 {each row x:int check defined(row)}
--- expected error
{"code": "E-SPEC-RECORD-VALUE", "message": "a record of 'row' is not a value; use one of its fields, as in 'row.x'", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

## 8.13 Checks and failure witnesses

A check MUST evaluate where written against values available at that point,
consume no input itself, and apply §8.3's truth conversion. A false result
MUST report `E-CHECK-FAILED`, with its message when nonempty. A check without
lifted records evaluates once. A later zero-iteration repeat MUST NOT turn
an earlier unavailable reference into vacuous truth.

```iris example
id: expressions-check-bare
operation: read
--- spec no-final-newline
check 0 "no"
--- input
--- expected error
{"code": "E-CHECK-FAILED", "message": "check failed: no", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": []}
```

```iris example
id: expressions-check-timing
operation: read
--- spec no-final-newline
check x>0 x:int
--- input no-final-newline
1
--- expected error
{"code": "E-NAME-UNDEFINED", "message": "'x' is not defined yet", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

After static resolution (§8.12), a lifted check MUST traverse available
record collections in increasing outer-to-inner record order, aligning
fields of the same repeat. At each deepest record it MUST evaluate the
condition; the first false result is the witness. An unavailable potential
collection supplies no traversal level, and a reached use still fails.
An empty available collection supplies no records, so its condition is not
evaluated. If runtime collections to be aligned are scalar or have different
outer lengths, implementations MUST report `E-CHECK-RECORDS`. This is a
conditional reference guard; no public reproducer is known.

An error evaluating a condition MUST remain a hard expression error with
profile R and null `record_path`, not a false check. False-check deferral is
defined in [§9.5](09-params-and-instances.md#95-deferred-failures-in-skipped-instances)
and [§11.2.1](errors.md#1121-deferred-instance-failures-and-catalog).

Diagnostics SHOULD describe witness indices starting at 1, innermost first.
When every lifted repeat has an `as` name, each level names its list with
`in` between levels. Otherwise the fallback is `record i of record j of
'label'`, using the selected deepest lifted name's label; partially named
levels MUST NOT be substituted into that fallback. Among equally deep available
lifted names, the first encountered during expression resolution supplies
the label.

A failed `all` MUST add the first false element as a witness. Ordinary array
domains use `element i of 'name'`, `element i of 'record.field'` for a direct
field on a name, or `element i of the array` otherwise. A domain statically
recognized as repeated records MUST use `record` instead of `element`,
naming its `as` list when present. Recognition applies to explicit record
lists and to ordinary name/field domains whose definitions agree on the
outermost repeat beyond the current context or selected record. A bound
ordinary-array name or an indexed ordinary array is not recognized this way.
This labeling MUST NOT turn an ordinary array binding into a record.

Witness propagation MUST follow this table:

| Expression failing the check | Retained comprehension witness |
|---|---|
| Failed `all`, including nested `all` | First false body and its enclosing failed `all` elements |
| Conjunction | First failing operand's witness |
| Disjunction with both operands false | Last evaluated operand's witness |
| Failed `any`, arithmetic, comparisons around `all`, or unary operators | No inner `all` witness |

On false-check failure, implementations MUST use the innermost available
saved record-witness start, then the lifted record start if needed,
otherwise the current input cursor. Ordinary array elements have no saved
leaf position. [§7.6](07-structure.md#76-logical-records-and-names) defines
record starts; [§4.6](04-input.md#46-input-positions) defines coordinates.
Implementations MUST NOT move an array witness's reported cursor to its
failing token's start.

Canonical `record_path` MUST follow
[§3.7](03-data-model.md#37-canonical-error-document): lifted record indices
outermost first, followed by comprehension witness indices outermost first,
all starting at 0. It MUST NOT include active execution-context indices,
explicitly selected record indices, or indices inferred from input positions.
A false check with no indexed witness MUST have `[]`.

| Example below | Human witness; input (line, column) | Context | `record_path` |
|---|---|---|---|
| `check-local-path` | No indexed witness; (1, 4) | `repeat [2 of 2]` | `[]` |
| `check-collected-path` | Record 2 of rows; (1, 3) | null | `[1]` |
| `check-context-path` | Record 2 of inner; (1, 7) | `repeat [2 of 2]` | `[1]` |
| `check-unnamed-asymmetric` | Record 2 of record 3 of x; (1, 11) | null | `[2, 1]` |
| `check-nested-named` | Record 3 of steps in record 2 of jobs; (2, 5) | null | `[1, 2]` |
| `check-element` | Element 3 of r in element 2 of a; (2, 6) | null | `[1, 2]` |
| `check-mixed-witness` | Element 2 of lim in record 2 of x; (1, 7) | null | `[1, 1]` |
| `check-all-records` | Record 2 of steps in record 2 of jobs; (2, 3) | null | `[1, 1]` |
| `indexed-record-witness` | Record 2 of steps selected through jobs[1]; (1, 7) | null | `[1]` |
| `indexed-record-direct-check` | No indexed witness; (1, 4) | null | `[]` |

```iris example
id: expressions-check-local-path
operation: read
--- spec
repeat 2 {x:int check x>0 "positive"}
--- input
1 0
--- expected error
{"code": "E-CHECK-FAILED", "message": "check failed: positive", "input_location": {"origin": "input.bin", "line": 1, "column": 4}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": "repeat [2 of 2]", "record_path": []}
```

```iris example
id: expressions-check-collected-path
operation: read
--- spec
repeat 2 as rows {x:int} check x>0 "positive"
--- input
1 0
--- expected error
{"code": "E-CHECK-FAILED", "message": "check failed for record 2 of 'rows': positive", "input_location": {"origin": "input.bin", "line": 1, "column": 3}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": [1]}
```

```iris example
id: expressions-check-context-path
operation: read
--- spec
repeat 2 as outer {repeat 2 as inner {x:int} check x>0 "positive"}
--- input
1 2 3 0
--- expected error
{"code": "E-CHECK-FAILED", "message": "check failed for record 2 of 'inner': positive", "input_location": {"origin": "input.bin", "line": 1, "column": 7}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": "repeat [2 of 2]", "record_path": [1]}
```

```iris example
id: expressions-check-unnamed-asymmetric
operation: read
--- spec no-final-newline
repeat 3 {repeat 2 {x:int}} check x>0
--- input no-final-newline
1 2 3 4 5 0
--- expected error
{"code": "E-CHECK-FAILED", "message": "check failed for record 2 of record 3 of 'x'", "input_location": {"origin": "input.bin", "line": 1, "column": 11}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": [2, 1]}
```

```iris example
id: expressions-check-nested-named
operation: read
--- spec no-final-newline
repeat 2 as jobs {repeat 3 as steps {x:int}} check x>0 "positive"
--- input
1 2 3
4 5 0
--- expected error
{"code": "E-CHECK-FAILED", "message": "check failed for record 3 of 'steps' in record 2 of 'jobs': positive", "input_location": {"origin": "input.bin", "line": 2, "column": 5}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": [1, 2]}
```

```iris example
id: expressions-check-element
operation: read
--- spec no-final-newline
a[2][3]:int check all(r in a:all(x in r:x>0)) "positive"
--- input
1 2 3
4 5 0
--- expected error
{"code": "E-CHECK-FAILED", "message": "check failed for element 3 of 'r' in element 2 of 'a': positive", "input_location": {"origin": "input.bin", "line": 2, "column": 6}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": [1, 2]}
```

```iris example
id: expressions-check-mixed-witness
operation: read
--- spec no-final-newline
lim[2]:int repeat 2 {x:int} check all(l in lim:x<l) "below"
--- input no-final-newline
5 3 1 4
--- expected error
{"code": "E-CHECK-FAILED", "message": "check failed for element 2 of 'lim' in record 2 of 'x': below", "input_location": {"origin": "input.bin", "line": 1, "column": 7}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": [1, 1]}
```

```iris example
id: expressions-check-all-records
operation: read
--- spec no-final-newline
repeat 2 as jobs {each job repeat 2 as steps {x:int}} check all(j in jobs:all(s in j.steps:s.x>0)) "positive"
--- input
1 2
3 0
--- expected error
{"code": "E-CHECK-FAILED", "message": "check failed for record 2 of 'steps' in record 2 of 'jobs': positive", "input_location": {"origin": "input.bin", "line": 2, "column": 3}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": [1, 1]}
```

```iris example
id: expressions-indexed-record-witness
operation: read
--- spec no-final-newline
repeat 2 as jobs {repeat 2 as steps {x:int}} check all(s in jobs[1].steps:s.x>0)
--- input no-final-newline
1 2 3 0
--- expected error
{"code": "E-CHECK-FAILED", "message": "check failed for record 2 of 'steps'", "input_location": {"origin": "input.bin", "line": 1, "column": 7}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": [1]}
```

```iris example
id: expressions-indexed-record-direct-check
operation: read
--- spec no-final-newline
repeat 2 as rows {x:int} check rows[1].x>0
--- input no-final-newline
1 0
--- expected error
{"code": "E-CHECK-FAILED", "message": "check failed", "input_location": {"origin": "input.bin", "line": 1, "column": 4}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": []}
```

```iris example
id: expressions-check-nested-mixed-names
operation: read
--- spec no-final-newline
repeat 2 as jobs {repeat 2 {x:int}} check x>0
--- input no-final-newline
1 2 3 0
--- expected error
{"code": "E-CHECK-FAILED", "message": "check failed for record 2 of record 2 of 'x'", "input_location": {"origin": "input.bin", "line": 1, "column": 7}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": [1, 1]}
```

```iris example
id: expressions-unnamed-record-all
operation: read
--- spec no-final-newline
repeat 2 {x:int} check all(t in x:t>0)
--- input no-final-newline
1 0
--- expected error
{"code": "E-CHECK-FAILED", "message": "check failed for record 2 of 'x'", "input_location": {"origin": "input.bin", "line": 1, "column": 3}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": [1]}
```

```iris example
id: expressions-check-any-no-witness
operation: read
--- spec no-final-newline
a[2]:int check any(x in a:x>5) "big"
--- input no-final-newline
1 2
--- expected error
{"code": "E-CHECK-FAILED", "message": "check failed: big", "input_location": {"origin": "input.bin", "line": 1, "column": 4}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": []}
```

```iris example
id: expressions-check-comparison-no-witness
operation: read
--- spec no-final-newline
a[2]:int check all(x in a:x>1)==1 "big"
--- input no-final-newline
1 2
--- expected error
{"code": "E-CHECK-FAILED", "message": "check failed: big", "input_location": {"origin": "input.bin", "line": 1, "column": 4}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": []}
```

```iris example
id: expressions-check-not-all-no-witness
operation: read
--- spec no-final-newline
a[2]:int check !all(x in a:x>0)
--- input no-final-newline
1 2
--- expected error
{"code": "E-CHECK-FAILED", "message": "check failed", "input_location": {"origin": "input.bin", "line": 1, "column": 4}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": []}
```

```iris example
id: expressions-check-and-first-witness
operation: read
--- spec no-final-newline
a[2]:int check all(x in a:x>1)&&all(y in a:y>2)
--- input no-final-newline
1 2
--- expected error
{"code": "E-CHECK-FAILED", "message": "check failed for element 1 of 'a'", "input_location": {"origin": "input.bin", "line": 1, "column": 4}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": [0]}
```

```iris example
id: expressions-check-or-last-witness
operation: read
--- spec no-final-newline
a[2]:int check all(x in a:x>2)||all(y in a:y>1) "big"
--- input no-final-newline
2 1
--- expected error
{"code": "E-CHECK-FAILED", "message": "check failed for element 2 of 'a': big", "input_location": {"origin": "input.bin", "line": 1, "column": 4}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": [1]}
```

## 8.14 Errors and related chapters

Implementations MUST use the existing [parse-time](errors.md#114-parse-time-codes)
and [read-time](errors.md#115-read-time-codes) codes, in
[error order](errors.md#112-error-order). Static binding, function,
constant, field, and scope failures use profile P. Runtime expression
failures use profile R: current input cursor, active reader-spec line,
null reader-spec column, active context when present, and null path.
Only a false check uses profile C, with §8.13's saved witness and path.
[§11.3.2](errors.md#1132-location-profiles) defines availability in full.

`E-CHECK-RECORDS` and `E-FIELD-UNDEFINED` remain conditional guards under
§§8.12–8.13; their presence in the registry does not establish a public
reader spec that reaches them.

[Chapter 5](05-reader-specs.md) owns syntax and precedence;
[chapter 6](06-types-and-values.md) owns type visibility, range-bound timing,
and numeric reading/offset conversion; [chapter 7](07-structure.md) owns
frames and column construction. [§9.2](09-params-and-instances.md#92-parameter-evaluation-and-conversion)
defines parameter conversion, [§9.4](09-params-and-instances.md#94-instance-selection)
instance selection, and [§9.6](09-params-and-instances.md#96-catalog-results)
catalog results. Expression traversal, including lifted checks and comprehensions,
MUST obey the work budget in chapter 10, `10-work-budgets.md` (planned),
and report `E-BUDGET` when exhausted. Appendix B, `appendix-b-dzn.md`
(planned), defines dzn rendering.
