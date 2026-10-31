# 9 Parameters, instances, and catalog

This chapter defines caller parameters, execution and selection of instances,
deferred rejections, and catalog data. [Chapter 5](05-reader-specs.md)
owns syntax and placement; [chapter 7](07-structure.md) owns structural
statements; [chapter 8](08-expressions-and-checks.md) owns expression and
check evaluation. Results use [chapter 3's canonical documents](03-data-model.md)
and [chapter 2's example transport](02-notation.md).

## 9.1 Caller parameters

The caller supplies parameter names and string values. Implementations MUST
match names exactly and case-sensitively. A parameter value is not an
already typed Iris value; §9.2 defines its conversion. The example `params`
part is a JSON string map, including when a value looks numeric.

Before execution, implementations MUST collect declared parameter names
recursively from all syntax, including unexecuted branches, repeat and
instances bodies, and sections-arm bodies. An actual `instances` statement
MUST also admit the caller key `instance`. A declaration merely named
`instances` MUST NOT admit that key. A known parameter MAY remain unused
when its declaration does not execute.

Implementations MUST reject the first unknown key in supplied parameter
order with `E-PARAM-UNKNOWN`. The combined phase order is
[§11.2](errors.md#112-error-order): validate reader-spec UTF-8, lex, parse,
check global conflicts, resolve, reject unknown parameters, validate
input-wide UTF-8, then execute. Parse-only operations stop before parameter
admission and input validation. Unknown parameters use
[profile U](errors.md#1132-location-profiles): no input location, reader-spec
origin with null line and column, and null context and path, as in
[§3.7.2](03-data-model.md#372-unknown-parameter).

Repeated host assignments MUST replace the preceding string for that key
without changing its original supplied position. Repeated `param` statements
are ordinary redefinitions: each execution MUST evaluate its own default
and convert the retained caller string again. There is no duplicate-parameter
language error. Duplicate JSON object members remain forbidden in examples.

A `param` MUST consume no input and introduce no frame. Its definition MUST
follow the [name rules](05-reader-specs.md#54-names),
[current-frame replacement rules](08-expressions-and-checks.md#82-redefinition),
and [repeat-column rules](07-structure.md#75-column-construction-and-missing-rows).
In particular, `_` and the reserved `gecode.` prefix are forbidden definition
names. Syntax and permitted positions remain in
[§§5.6–5.11](05-reader-specs.md#56-statements-declarations-and-blocks).
Host assignment-list parsers and command-line options are outside the language.

```iris example
id: params-unknown-name
operation: read
--- spec
param p=1
--- input
--- params
{"other": "1"}
--- expected error
{"code": "E-PARAM-UNKNOWN", "message": "the spec has no parameter 'other'", "input_location": null, "spec_location": {"origin": "spec.iris", "line": null, "column": null}, "context": null, "record_path": null}
```

## 9.2 Parameter evaluation and conversion

Executing a `param` MUST perform these steps:

1. If it has a default expression, evaluate it under [chapter 8](08-expressions-and-checks.md),
   even when the caller supplied a value. An expression error MUST precede
   conversion and retain its own code and location. This includes the
   [work charges in §10.5](10-work-budgets.md#105-evaluation-instances-and-catalog).
2. If no caller value exists, store the default unchanged, including its
   value type and depth. With neither a caller value nor a default, report
   `E-PARAM-MISSING`.
3. Otherwise convert the entire caller string by the following table and
   store the resulting scalar in the current frame. This step MUST NOT
   read or skip input.

| Default | Required supplied-string conversion | Conversion failure |
|---|---|---|
| None | Try signed 64-bit integer, then decimal binary64, then retain the string. | None; failed numeric conversions fall through. |
| Any value with value type `int` | Signed 64-bit integer only. | `E-PARAM-TYPE`; recommended message: `parameter 'x' must be an integer, not 'VALUE'`. |
| Any value with value type `float` | Decimal binary64, including integer-looking strings; produce float. | `E-PARAM-TYPE`; recommended message: `parameter 'x' must be a number, not 'VALUE'`. |
| Any value with value type `string` | Retain the string exactly. | None. |

The default MAY be an array, including an empty array. An omitted caller
value MUST preserve that array's full shape. An override MUST use only its
value type, the type of its scalar leaves, and produce depth 0.

Integer conversion MUST use [§4.4's complete decimal conversion](04-input.md#44-integer-tokens):
ASCII `[+-]?[0-9]+`, within -9223372036854775808..9223372036854775807.
Leading zeros remain decimal; `+007` becomes 7, `-0` becomes 0, and `4.0`
does not convert to int. Float conversion MUST use
[§4.5's complete decimal conversion and binary64 rounding](04-input.md#45-floating-point-tokens),
including its overflow and underflow rules. With a float default, `2`
produces float 2. Parameters MUST preserve negative zero: they do not
perform a declaration's offset addition.

Infinity, NaN, hexadecimal forms, incomplete exponents, bare signs,
embedded NUL, surrounding whitespace, and extra characters MUST fail numeric
conversion. Nonzero decimals rounding to subnormal or zero MUST fail;
exact `0e-9999` and decimals rounding to the smallest normal value MUST
succeed. Without a default, integer overflow MUST fall through to float
conversion; failed float conversion MUST fall through to string. These conversion
failures MUST NOT report `E-INTEGER-OVERFLOW` or `E-EXPECTED-VALUE`.

Conversion MUST operate on the supplied string independently of `tokens`,
`separators`, `comment`, and `stop`. Implementations MUST NOT trim it,
unquote it, evaluate it as an expression, interpret Iris string escapes,
split commas, or tokenize it. Quotes, backslashes, spaces, commas, `=`,
and Unicode characters remain literal string contents. An empty string is
valid under inference or a string default. With a string default, `3`
remains a string.

The first example combines inference, typed defaults, an exact string
containing spaces, a comma and `=`, and negative zero. The following examples
show numeric fallbacks, default reexecution, and array defaults. The known
`unused` parameter does not execute.

```iris example
id: params-supplied-values
operation: read
--- spec
param i param f param text param empty param large
param integer=1 param real=1.0 param string="default" param zero=0.0
--- input
--- params
{"i": "+007", "f": "1.5", "text": "alpha", "empty": "", "large": "9223372036854775808", "integer": "-07", "real": "2", "string": " a,b=2; ", "zero": "-0.0"}
--- expected data
{"iris": "0.1", "entries": [
  {"name": "i", "type": "int", "depth": 0, "value": 7},
  {"name": "f", "type": "float", "depth": 0, "value": 1.5},
  {"name": "text", "type": "string", "depth": 0, "value": "alpha"},
  {"name": "empty", "type": "string", "depth": 0, "value": ""},
  {"name": "large", "type": "float", "depth": 0, "value": 9223372036854776000},
  {"name": "integer", "type": "int", "depth": 0, "value": -7},
  {"name": "real", "type": "float", "depth": 0, "value": 2},
  {"name": "string", "type": "string", "depth": 0, "value": " a,b=2; "},
  {"name": "zero", "type": "float", "depth": 0, "value": -0.0}
]}
```

```iris example
id: params-conversion-fallbacks
operation: read
--- spec
param overflow param underflow param hex param leading param trailing
--- input
--- params
{"overflow": "1e309", "underflow": "5e-324", "hex": "0x10", "leading": " 7", "trailing": "7 "}
--- expected data
{"iris": "0.1", "entries": [
  {"name": "overflow", "type": "string", "depth": 0, "value": "1e309"},
  {"name": "underflow", "type": "string", "depth": 0, "value": "5e-324"},
  {"name": "hex", "type": "string", "depth": 0, "value": "0x10"},
  {"name": "leading", "type": "string", "depth": 0, "value": " 7"},
  {"name": "trailing", "type": "string", "depth": 0, "value": "7 "}
]}
```

```iris example
id: params-default-evaluation
operation: read
--- spec
param a=2 param b=a+1 param a=b+1
if 0 {param unused}
repeat 2 {v:int param row=v+1}
--- input
1 3
--- params
{"unused": "ignored"}
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 0, "value": 4},
  {"name": "b", "type": "int", "depth": 0, "value": 3},
  {"name": "v", "type": "int", "depth": 1, "value": [1, 3]},
  {"name": "row", "type": "int", "depth": 1, "value": [2, 4]}
]}
```

```iris example
id: params-array-defaults
operation: read
--- spec
a[2]:int f[0]:float s[0]:word
param keep=a param changed=a
param keep_float=f param changed_float=f
param keep_string=s param changed_string=s
--- input
1 2
--- params
{"changed": "7", "changed_float": "8", "changed_string": "9"}
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 1, "value": [1, 2]},
  {"name": "f", "type": "float", "depth": 1, "value": []},
  {"name": "s", "type": "string", "depth": 1, "value": []},
  {"name": "keep", "type": "int", "depth": 1, "value": [1, 2]},
  {"name": "changed", "type": "int", "depth": 0, "value": 7},
  {"name": "keep_float", "type": "float", "depth": 1, "value": []},
  {"name": "changed_float", "type": "float", "depth": 0, "value": 8},
  {"name": "keep_string", "type": "string", "depth": 1, "value": []},
  {"name": "changed_string", "type": "string", "depth": 0, "value": "9"}
]}
```

Missing and incompatible parameters MUST use
[profile R](errors.md#1132-location-profiles): current input cursor,
executing `param` line, null reader-spec column, active enclosing context,
and null path. The parameter name MUST NOT itself add a value-read context.
Default-expression errors also retain profile R.

```iris example
id: params-required
operation: read
--- spec
param p
--- input
--- expected error
{"code": "E-PARAM-MISSING", "message": "parameter 'p' must be given", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: params-integer-rejection
operation: read
--- spec
param p=1
--- input
--- params
{"p": "1.5"}
--- expected error
{"code": "E-PARAM-TYPE", "message": "parameter 'p' must be an integer, not '1.5'", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: params-float-rejection
operation: read
--- spec
param p=1.0
--- input
--- params
{"p": "1e309"}
--- expected error
{"code": "E-PARAM-TYPE", "message": "parameter 'p' must be a number, not '1e309'", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: params-eager-default-error
operation: read
--- spec
param p=1/0
--- input
--- params
{"p": "7"}
--- expected error
{"code": "E-DIVISION-ZERO", "message": "division by zero", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

## 9.3 Instance blocks and frames

Instance-block syntax and placement MUST follow
[§5.11](05-reader-specs.md#511-placement-restrictions); subsequent format
metadata remains permitted by [§5.9](05-reader-specs.md#59-format-metadata).
The examples show the two placement failures.

```iris example
id: instances-nested-block
operation: parse
--- spec
repeat 1 {instances {n:int}}
--- expected error
{"code": "E-SPEC-PLACEMENT", "message": "'instances' must be at the top level of the spec", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: instances-following-statement
operation: parse
--- spec
instances {n:int} n:int
--- expected error
{"code": "E-SPEC-PLACEMENT", "message": "'instances' must be the last statement of the spec", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

Implementations MUST execute the prefix before the block once, then perform
[normal skipping](04-input.md#47-comment-and-stop-prefixes) at block entry
and between passes. While data remains, they MUST execute the body once
per instance until traversal ends or explicit selection returns.

Each pass MUST have a fresh instance value frame enclosing its repeats.
Initially, prefix values are available by [enclosing lookup](08-expressions-and-checks.md#81-values-and-value-lookup).
Skipped frames MUST be discarded; their local definitions MUST NOT become
enclosing values for later passes. Without a selector, retaining instance 1
MUST publish its entries into the outer frame before later passes execute.
Those later passes MUST see that updated outer frame. Catalog MUST NOT
publish instance entries into the outer frame. The instances statement adds
no repeat level or logical record-list entry and supplies no iteration for
`index`.

After a body completes, its cursor MUST have advanced beyond its start
after initial skipping. Otherwise implementations MUST report
`E-NO-PROGRESS` before selection or catalog collection. This error occurs
outside the instance execution context, at the block's line, with null path.

```iris example
id: instances-no-progress
operation: read
--- spec
instances {let x=1}
--- input
1
--- expected error
{"code": "E-NO-PROGRESS", "message": "an instance must read some input", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

For a retained instance, implementations MUST copy its local entries into
the outer frame in local definition order, using ordinary replacement:
existing prefix entries retain their positions and new entries append.
Read data MUST include prefix and retained local entries without an
instance-array wrapper or a synthetic `instance` entry. In this example,
`before` finds the prefix's 9 in both passes; the selected pass replaces
`shared` and `name` without moving them. Selection leaves `BAD` unread.

```iris example
id: instances-frame-selection
operation: read
--- spec
let shared=9 let name="outer"
instances {n:int let before=shared let shared=n name:word}
--- input
1 a 2 b BAD
--- params
{"instance": "+002"}
--- expected data
{"iris": "0.1", "entries": [
  {"name": "shared", "type": "int", "depth": 0, "value": 2},
  {"name": "name", "type": "string", "depth": 0, "value": "b"},
  {"name": "n", "type": "int", "depth": 0, "value": 2},
  {"name": "before", "type": "int", "depth": 0, "value": 9}
]}
```

Without an instances block, read MUST execute the ordinary reader spec
once. Caller key `instance` is unknown unless admitted by an ordinary
`param instance` declaration; that declaration alone MUST NOT select input.

## 9.4 Instance selection

Selection MUST use the caller string named `instance`, independently of any
value stored by `param instance`. Implementations MUST apply these rules:

1. If the caller supplied `instance`, at block entry, before executing any
   body, try §9.2's complete signed 64-bit integer conversion on that string.
   Success selects that
   1-based ordinal. A value below 1 MUST report `E-INSTANCE-NUMBER` before
   checking whether input is empty. Leading `+` and zeros are accepted;
   float-to-ordinal conversion is forbidden.
2. A string that fails integer conversion MUST select by exact name. This
   includes `1.0`, `3x`, surrounding whitespace, an overflowing integer
   spelling, and the empty string.
3. Derive the usable name solely from the local instance frame. A scalar
   string contributes itself; a scalar int contributes its ordinary decimal
   spelling. Absent, array-valued, and float-valued `name` contribute `""`.
   Implementations MUST NOT look up an outer `name` for this purpose.
   Final local replacement determines the completed instance's name.
4. Duplicate names are permitted. Retain the first completed matching
   instance, subject to §9.5's rejections. The empty selector MUST match an
   empty derived name, including an absent or unusable name.

Integer-looking names are subject to ordinal priority: `2` selects ordinal
2 even if a different instance has name `"2"`; `-3` is an invalid ordinal
even if an instance has that name. A float-valued name is unusable, whereas
a string name `"1.0"` is usable.

```iris example
id: instances-ordinal-before-name
operation: read
--- spec
instances {n:int name:word}
--- input no-final-newline
1 2 7 other
--- params
{"instance": "2"}
--- expected data
{"iris": "0.1", "entries": [{"name": "n", "type": "int", "depth": 0, "value": 7}, {"name": "name", "type": "string", "depth": 0, "value": "other"}]}
```

```iris example
id: instances-single
operation: read
--- spec
instances {n:int}
--- input
7
--- expected data
{"iris": "0.1", "entries": [
  {"name": "n", "type": "int", "depth": 0, "value": 7}
]}
```

```iris example
id: instances-text-selection
operation: read
--- spec
instances {n:int name:word}
--- input
1 1.0 2 9223372036854775808
--- params
{"instance": "9223372036854775808"}
--- expected data
{"iris": "0.1", "entries": [
  {"name": "n", "type": "int", "depth": 0, "value": 2},
  {"name": "name", "type": "string", "depth": 0, "value": "9223372036854775808"}
]}
```

```iris example
id: instances-empty-name-selection
operation: read
--- spec
instances {n:int}
--- input
1 2
--- params
{"instance": ""}
--- expected data
{"iris": "0.1", "entries": [
  {"name": "n", "type": "int", "depth": 0, "value": 1}
]}
```

```iris example
id: instances-duplicate-names
operation: read
--- spec
instances {n:int name:word}
--- input
1 a 2 a
--- params
{"instance": "a"}
--- expected data
{"iris": "0.1", "entries": [
  {"name": "n", "type": "int", "depth": 0, "value": 1},
  {"name": "name", "type": "string", "depth": 0, "value": "a"}
]}
```

Explicit selection MUST return after the selected instance, bypassing the
[final trailing-input check](04-input.md#48-end-of-input-and-trailing-input)
and all later instance executions. Input-wide UTF-8 validation has already
examined the whole supplied input, including that unread tail.

Without a selector, implementations MUST retain the first instance and
traverse later instances to determine whether there is exactly one.
`ignore trailing` MUST NOT stop this traversal. A rejection in the first
instance is immediate; §9.5 permits eligible rejections in later instances
to be deferred. A later hard error MUST precede an eventual multiple-instance
error.

After traversal, read MUST report the first applicable result below:

| Condition | Result |
|---|---|
| Zero completed instances | `E-INSTANCE-EMPTY`, even with an unmatched selector |
| Nonempty input traversal with an unmatched explicit selector | `E-INSTANCE-NOT-FOUND` |
| More than one instance without a selector | `E-INSTANCE-SELECTION` |
| Exactly one instance without a selector | Return its data |

Catalog instead follows §9.6. The last example shows that a defaulted
`param instance=2` supplies an ordinary value but no caller selection.

```iris example
id: instances-no-instance
operation: read
--- spec
instances {n:int}
--- input
--- expected error
{"code": "E-INSTANCE-EMPTY", "message": "the file contains no instance", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: instances-invalid-number
operation: read
--- spec
instances {n:int}
--- input
1
--- params
{"instance": "0"}
--- expected error
{"code": "E-INSTANCE-NUMBER", "message": "instance numbers start at 1", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: instances-not-found
operation: read
--- spec
instances {n:int}
--- input
1
--- params
{"instance": "3"}
--- expected error
{"code": "E-INSTANCE-NOT-FOUND", "message": "there is no instance '3' (the file has 1 instance)", "input_location": {"origin": "input.bin", "line": 2, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: instances-selection-required
operation: read
--- spec
param instance=2
instances {n:int}
--- input
1 2
--- expected error
{"code": "E-INSTANCE-SELECTION", "message": "the file has 2 instances; select one with the parameter instance (a number from 1, or a name)", "input_location": {"origin": "input.bin", "line": 2, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 2, "column": null}, "context": null, "record_path": null}
```

Retaining the first instance updates the enclosing frame before the
unselected traversal continues (§9.3). Here instance 2 finds `shared=1`
from the retained first instance, so its division by zero precedes the
eventual multiple-instance error.

```iris example
id: instances-retained-frame-visible
operation: read
--- spec
let shared=9
instances {n:int let before=shared let v=1/(shared-1) let shared=n}
--- input
1 2
--- expected error
{"code": "E-DIVISION-ZERO", "message": "division by zero", "input_location": {"origin": "input.bin", "line": 1, "column": 4}, "spec_location": {"origin": "spec.iris", "line": 2, "column": null}, "context": "instance [2]", "record_path": null}
```

## 9.5 Deferred failures in skipped instances

[§8.13](08-expressions-and-checks.md#813-checks-and-failure-witnesses)
evaluates a check and constructs a false-check diagnostic and witness.
This section determines whether that rejection is immediate or deferred.
It MUST NOT suppress condition evaluation: an error evaluating the condition
is a hard expression error with null path.

Only three events MAY be deferred: a false check (`E-CHECK-FAILED`), a
repeated sections arm (`E-SECTIONS-REPEATED`), and a missing required sections
arm (`E-LINE-MATCH`). An ordinary failed `line` also uses `E-LINE-MATCH`
but MUST remain hard. Implementations MUST distinguish the rejection event,
not decide deferral by error code alone.

At each rejection, implementations MUST decide whether the current instance
may be skipped using this table. They MUST defer exactly when it says yes,
subject to the missing-arm exception below.

| Execution situation | Defer? |
|---|---|
| Outside an active instance body, including catalog's prefix or a reader spec without a block | No |
| Catalog inside an instance body | Yes |
| Read without a selector | Only after instance 1 |
| Read with ordinal selection | Exactly when the current ordinal differs |
| Read with name selection | Exactly when the current local derived name differs from the selector |

Name derivation follows §9.4, including `""` for absent or unusable names.
Implementations MUST recompute the predicate at every rejection. A nonempty
selected name may still be unknown at an early check, while an empty
selector already matches an absent name.

On deferral, implementations MUST retain only the first diagnostic and
summary for the instance and continue execution and input consumption.
A deferred false check proceeds to the following statement. A repeated
sections arm still commits and validates its second captures and executes
its body; these actions can replace `name`. A deferred missing-arm rejection
returns from that sections statement, after which later statements execute.
Normal sections behavior and saved locations remain in
[§7.8](07-structure.md#78-sections).

Before rejecting a missing required arm, implementations MUST perform normal
skipping and test whether any data remains at that point. If none remains,
they MUST throw the missing-arm diagnostic immediately, even during catalog
or a skipped instance. Data for a later statement of the same instance is
sufficient to permit deferral. The diagnostic MUST retain §7.8's saved
terminator, last-line, or EOF position even if this skipping moves the cursor.

If an instance completes with its final name matching the selector,
implementations MUST rethrow its stored first diagnostic, if any, preserving
the original locations, context, and witness. Assigning the selected name
MUST NOT itself rethrow it. A later rejection after the current name matches
MUST throw immediately and become primary; the stored diagnostic is rethrown
only if execution reaches matching completion.

The first two examples share a lifted-check fixture: selecting `b` skips the
rejected `a`; selecting `a` preserves the earlier witness `[1]`, input
position (1, 5), reader-spec line 3, and context `instance [1]`. The third
example reports `second` immediately after `name` becomes selected.

```iris example
id: instances-skip-record-failure
operation: read
--- spec
instances {
  n:int repeat n {x:int}
  check x>0 "positive"
  name:word
}
--- input
2 1 0 a
1 5 b
--- params
{"instance": "b"}
--- expected data
{"iris": "0.1", "entries": [
  {"name": "n", "type": "int", "depth": 0, "value": 1},
  {"name": "x", "type": "int", "depth": 1, "value": [5]},
  {"name": "name", "type": "string", "depth": 0, "value": "b"}
]}
```

```iris example
id: instances-selected-record-failure
operation: read
--- spec
instances {
  n:int repeat n {x:int}
  check x>0 "positive"
  name:word
}
--- input
2 1 0 a
1 5 b
--- params
{"instance": "a"}
--- expected error
{"code": "E-CHECK-FAILED", "message": "check failed for record 2 of 'x': positive", "input_location": {"origin": "input.bin", "line": 1, "column": 5}, "spec_location": {"origin": "spec.iris", "line": 3, "column": null}, "context": "instance [1]", "record_path": [1]}
```

```iris example
id: instances-name-becomes-selected
operation: read
--- spec
instances {
 n:int check n>0 "first"
 name:word check 0 "second"
}
--- input
0 a
--- params
{"instance": "a"}
--- expected error
{"code": "E-CHECK-FAILED", "message": "check failed: second", "input_location": {"origin": "input.bin", "line": 1, "column": 4}, "spec_location": {"origin": "spec.iris", "line": 3, "column": null}, "context": "instance [1]", "record_path": []}
```

All other errors MUST abort reading, including conversion, range, shape,
undefined-name, expression, work-budget, unexpected-line, and value-level
UTF-8 failures. [§10.6](10-work-budgets.md#106-exhaustion-and-error-locations)
defines hard budget exhaustion and its locations. An error thrown during
body execution after a deferred rejection MUST remain primary. The
reference's secondary text,
`after an earlier error in the instance: SUMMARY`, MUST NOT change its code
or path. This also applies to a later immediate rejection. Post-body errors,
such as no progress, occur outside that wrapper.

Instance context MUST use `instance [N]` with ordinals counted from 1.
It MUST NOT add an index to `record_path`. These examples distinguish a
hard read failure after a rejection, a hard check-evaluation error, and an
ordinary line mismatch.

```iris example
id: instances-skipped-hard-error
operation: read
--- spec
instances {n:int check n>0 "positive" x:int name:word}
--- input
0 bad a 5 7 b
--- params
{"instance": "b"}
--- expected error
{"code": "E-EXPECTED-VALUE", "message": "expected int, found 'bad'", "input_location": {"origin": "input.bin", "line": 1, "column": 3}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": "instance [1] > x", "record_path": null}
```

```iris example
id: instances-skipped-check-expression
operation: read
--- spec
instances {n:int check n/0 name:word}
--- input
0 a 5 b
--- params
{"instance": "b"}
--- expected error
{"code": "E-DIVISION-ZERO", "message": "division by zero", "input_location": {"origin": "input.bin", "line": 1, "column": 2}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": "instance [1]", "record_path": null}
```

```iris example
id: instances-line-error-is-hard
operation: read
--- spec
instances {line "A"}
--- input
B
A
--- params
{"instance": "2"}
--- expected error
{"code": "E-LINE-MATCH", "message": "expected a line matching \"A\"", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": "instance [1]", "record_path": null}
```

## 9.6 Catalog results

Catalog MUST parse and resolve, admit parameters, validate input, and execute
under the same phase order as read (§9.1). It MUST execute the prefix and
all instance bodies, performing actual reads, expressions, checks, progress
checks, and [work accounting](10-work-budgets.md#105-evaluation-instances-and-catalog).
Required ordinary parameters remain required, including inside traversed bodies.

A valid caller `instance` selector MUST NOT restrict traversal and need not
match an instance. Catalog MUST still validate its integer form at block
entry: zero and negative integer selectors report `E-INSTANCE-NUMBER`.

On success, catalog MUST return only these entries, in the order shown,
using the [ordinary data document](03-data-model.md#33-canonical-data-document)
and unchanged [data schema](../schema/data.schema.json):

| Entry | Value type; depth | Presence and value |
|---|---|---|
| `instances` | int; 0 | Always: the number of completed passes. Zero is valid with a block. Without a block, execute the ordinary reader spec and report 1, even for empty input it accepts. |
| `name` | string; 1 | Only for a positive pass count when every derived local name is nonempty. One name per instance in input order, using §9.4's int normalization; duplicate names remain. Omit the entire entry for any absent, empty, float, or array name. Without a block, omit it even if ordinary data defines `name`. |
| `failed` | string; 1 | Only when at least one instance has a retained rejection summary. One string per pass in input order, with `""` for a valid instance. Omit it when all instances are valid, and without a block. |

Catalog MUST NOT include prefix or instance data, selected input, canonical
error objects, or an extra catalog wrapper in a successful result.

Retained summary strings MUST have these exact contents:

| First rejection | Summary |
|---|---|
| False check with a nonempty message | That message exactly |
| False check without a message, or with an empty message | `check on line N of the spec`, where N is the check's reader-spec line |
| Repeated sections arm | `a second line matching "PATTERN"`, without the first-line parenthesis |
| Missing required sections arm | `no line matching "PATTERN"` |

`PATTERN` is the decoded pattern text; quotation marks in the two templates
are literal data characters. Summaries MUST NOT include locations, context,
check-witness paths, or error-code wrappers. Only the first rejection's
summary is retained. These are data strings, so their contents are normative
even though diagnostic message wording is recommended.

The first catalog exercises both check-summary forms, first-failure retention,
repeated capture commitment that changes `b` to `c`, and a missing required
arm. Caller selector `99` does not limit its five passes. The second uses
§9.5's lifted-check fixture.

```iris example
id: catalog-failure-summaries
operation: catalog
--- spec
instances {
 sections {
  "NAME {name}" optional
  "X {x:int}"
  "END" last
 }
 check !defined(x)||x>0 "positive"
 check !defined(x)||(x!=0 && x!=2)
}
--- input
NAME a
X 0
END
NAME b
NAME c
X 1
END
NAME d
END
NAME e
X 2
END
NAME f
X 3
END
--- params
{"instance": "99"}
--- expected data
{"iris": "0.1", "entries": [
  {"name": "instances", "type": "int", "depth": 0, "value": 5},
  {"name": "name", "type": "string", "depth": 1, "value": ["a", "c", "d", "e", "f"]},
  {"name": "failed", "type": "string", "depth": 1, "value": ["positive", "a second line matching \"NAME {name}\"", "no line matching \"X {x:int}\"", "check on line 8 of the spec", ""]}
]}
```

```iris example
id: catalog-record-failure
operation: catalog
--- spec
instances {
  n:int repeat n {x:int}
  check x>0 "positive"
  name:word
}
--- input
2 1 0 a
1 5 b
--- expected data
{"iris": "0.1", "entries": [
  {"name": "instances", "type": "int", "depth": 0, "value": 2},
  {"name": "name", "type": "string", "depth": 1, "value": ["a", "b"]},
  {"name": "failed", "type": "string", "depth": 1, "value": ["positive", ""]}
]}
```

The next examples show an empty block input, ordinary execution without a
block, normalized integer and duplicate string names, and unusable local
names. An outer `name` supplies none of the missing local names.

```iris example
id: catalog-empty-file
operation: catalog
--- spec
instances {n:int}
--- input
--- expected data
{"iris": "0.1", "entries": [
  {"name": "instances", "type": "int", "depth": 0, "value": 0}
]}
```

```iris example
id: catalog-without-block
operation: catalog
--- spec
let name="outside"
--- input
--- expected data
{"iris": "0.1", "entries": [
  {"name": "instances", "type": "int", "depth": 0, "value": 1}
]}
```

```iris example
id: catalog-usable-names
operation: catalog
--- spec
instances {k:int if k {name:int} else {name:word}}
--- input
1 -03 0 same 0 same
--- expected data
{"iris": "0.1", "entries": [
  {"name": "instances", "type": "int", "depth": 0, "value": 3},
  {"name": "name", "type": "string", "depth": 1, "value": ["-3", "same", "same"]}
]}
```

```iris example
id: catalog-unusable-name
operation: catalog
--- spec
let name="outer"
instances {k:int if k==1 {let name=1.0} else if k==2 {name[0]:word} else if k==3 {let name=""}}
--- input
0 1 2 3
--- expected data
{"iris": "0.1", "entries": [
  {"name": "instances", "type": "int", "depth": 0, "value": 4}
]}
```

Catalog MUST abort on every hard error, every rejection outside an active
instance body, and §9.5's missing-arm EOF exception. It MUST NOT return a
partial catalog. Failures use the ordinary complete `expected error`
document. The last example retains the `END` line's position (2, 1), rather
than the cursor after skipping to EOF (3, 1).

```iris example
id: catalog-bad-selector
operation: catalog
--- spec
instances {n:int}
--- input
1
--- params
{"instance": "0"}
--- expected error
{"code": "E-INSTANCE-NUMBER", "message": "instance numbers start at 1", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: catalog-no-block-check
operation: catalog
--- spec
check 0 "no"
--- input
--- expected error
{"code": "E-CHECK-FAILED", "message": "check failed: no", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": []}
```

```iris example
id: catalog-missing-arm-at-end
operation: catalog
--- spec
instances {sections {"A {n:int}" "B {m:int}" "END" last}}
--- input
A 1
END
--- expected error
{"code": "E-LINE-MATCH", "message": "expected a line matching \"B {m:int}\" before the line matching \"END\"", "input_location": {"origin": "input.bin", "line": 2, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": "instance [1]", "record_path": null}
```

## 9.7 Errors and related chapters

Implementations MUST use the existing [error registry](errors.md), its
[location profiles](errors.md#1132-location-profiles), and
[error order](errors.md#112-error-order). No parameter, selection, or catalog
operation introduces a new code or canonical document shape.

| Code | Use and location |
|---|---|
| `E-PARAM-UNKNOWN` | Unknown caller key; U, no input location and null reader-spec coordinates (§9.1). |
| `E-PARAM-MISSING`, `E-PARAM-TYPE` | Executed parameter; R, current cursor and param line, enclosing context, null path (§9.2). |
| `E-INSTANCE-NUMBER` | Selector below 1; R, cursor at block entry and instances line, null context and path (§9.4). |
| `E-INSTANCE-EMPTY`, `E-INSTANCE-NOT-FOUND`, `E-INSTANCE-SELECTION` | Traversal completion; R, final skipped cursor and instances line, null context and path (§9.4). |
| `E-NO-PROGRESS` | Body consumed no input; R, post-body cursor and instances line, null context and path (§9.3). |
| `E-CHECK-FAILED` | False check; C, original check locations and witness under [§8.13](08-expressions-and-checks.md#813-checks-and-failure-witnesses). |
| `E-SECTIONS-REPEATED`, `E-LINE-MATCH` | Structural rejection; R, original saved locations, null path under [§7.8](07-structure.md#78-sections). Ordinary line mismatches remain hard. |

[§3.8](03-data-model.md#38-error-comparison) distinguishes primary compared
coordinates from secondary reader-spec coordinates. Other expression errors
remain in [§8.14](08-expressions-and-checks.md#814-errors-and-related-chapters),
reading errors in [§6.11](06-types-and-values.md#611-errors-and-related-chapters),
and UTF-8 errors in [§4.10](04-input.md#410-utf-8-validity) and
[§5.1](05-reader-specs.md#51-reader-specs-and-parsing).
[§10.1](10-work-budgets.md#101-counters-and-input-byte-length) defines
operation-wide counters and input byte length;
[§10.5](10-work-budgets.md#105-evaluation-instances-and-catalog) defines
accounting across instance selection and catalog execution.
[Appendix B.7](appendix-b-dzn.md#b7-catalog-data-and-executable-examples)
owns dzn rendering of catalog data; retained read data uses the same rules.
