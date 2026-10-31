# 10 Work budgets

This chapter defines the exact virtual charges for idle work and expression
effort. These charges MUST determine budget exhaustion independently of an
implementation's allocation, value representation, copying, optimization,
sorting strategy, or elapsed time. They are not a wall-clock limit.
Value structure follows [chapter 3](03-data-model.md); statement execution
and records follow [chapter 7](07-structure.md); expression evaluation follows
[chapter 8](08-expressions-and-checks.md).

## 10.1 Counters and input byte length

Let B = 1048576 = 2^20. Let N be the byte length of the entire original
supplied input, before scanning, filtering, stopping, selection, or ignored
trailing input. N MUST count UTF-8 encoding bytes, not characters, tokens,
lines, retained data, or consumed bytes. CR LF contributes two bytes.
Comments, separators, a BOM, bytes after a stop, and bytes after a selected
instance all contribute. Reader-spec bytes and caller parameter strings
MUST NOT contribute.

Each read or catalog operation MUST initialize two independent counters,
idle work I = 0 and expression effort E = 0, with these limits:

| Counter | Limit |
|---|---:|
| I | B |
| E | B + 16*N |

At each charge event below, implementations MUST add the entire charge to
its counter and fail immediately if and only if the new total is strictly
greater than its limit. Equality MUST be allowed. Each charge is atomic;
an implementation MUST NOT postpone part of an argument's charge until
after processing its leaves.

All budget formulas, charges, additions, and comparisons MUST use exact
mathematical nonnegative integers, without overflow, wrapping, or rounding.
An implementation MAY use any representation or safe early-excess check
that gives the same result and failure point. The temporary pinned-reference
bug is recorded in [appendix C.3](appendix-c-differences.md#c3-exact-budget-arithmetic).

The counters MUST NOT reset within an operation, including when input
advances, a value is stored or replaced, a repeat ends, an instance is
discarded or retained, or a rejection is deferred. A separate read or
catalog operation MUST start fresh. Parse-only operations MUST NOT run
these budgets. Rendering dzn MUST add no charges beyond those of the read
or catalog whose data it renders.

[Input-wide UTF-8 validation](04-input.md#410-utf-8-validity) examines the
same original bytes without normalization or removal. It MUST add no work
charge. It follows parsing/resolution and unknown-parameter checks and
precedes execution; invalid input therefore fails with `E-UTF8` before any
budget error. Value-level UTF-8 checks retain their execution order.

## 10.2 Idle work

Implementations MUST add idle charges at exactly these events, in the order
execution reaches them:

| Event | Charge and timing |
|---|---|
| A counted repeat iteration completes with its input cursor unchanged from immediately before its body | Add 1 after the entire body, before collecting columns or removing the active iteration context. |
| An ordinary fixed-size array has dimension product zero | After all dimensions and structural product checks, before constructing the empty structure, visit positive dimensions from the left up to the first zero. At each dimension add the product of all dimensions through that dimension, as a separate charge. |
| An alphabet array has zero columns | After its product-size check, before constructing the structure, add its row count once. |

For ordinary arrays, `[0][n]` costs 0, `[a][0]` costs a, and
`[a][b][0]` costs a + a*b for positive a and b. An ordinary array without
a zero dimension adds no idle charge. `FULL_MATRIX` MUST use this ordinary
array rule; other [matrix layouts](06-types-and-values.md#68-matrix-layouts)
MUST add no independent idle charge. Alphabet `[r][0]` costs r;
`[0][c]` costs 0. [Structural bounds and evaluation order](06-types-and-values.md#66-fixed-size-arrays-and-empty-rows)
still apply. Discarding a read with `_` MUST NOT avoid its charges.

A repeat's pre-body cursor is its actual cursor after that repeat's own
termination handling. Saving a [record start](07-structure.md#76-logical-records-and-names)
MUST NOT advance it. Any actual cursor advance during the body, including
skipped bytes, counts as progress; storing data is unnecessary. Nested
counted repeats MUST charge independently: an idle inner iteration can add
1, then an idle enclosing iteration can add another 1. A zero-iteration
repeat adds no iteration charge.

An [indefinite repeat](07-structure.md#74-repeat-forms-and-termination)
without progress MUST report `E-NO-PROGRESS` instead of adding a counted
iteration charge. An [instance](09-params-and-instances.md#93-instance-blocks-and-frames)
without progress MUST likewise report `E-NO-PROGRESS` after its body; the
body can already have exhausted a budget.

Column collection, missing-row filling, list termination, metadata, and
catalog-result construction MUST add no independent idle charges. Reads
and repeats executed within them remain subject to the table above.

## 10.3 Expression effort

Let L(v) be the number of scalar leaves of value v at all depths. A scalar
has one leaf; an entirely empty tree has zero. A string scalar counts as
one leaf regardless of its byte length. Depth and ragged structure follow
[§3.2](03-data-model.md#32-depth-arrays-and-paths).

Implementations MUST add expression charges at exactly these events:

| Event | Charge and timing |
|---|---|
| Successfully index an explicit record list to select a record | Add 1 after index evaluation, type validation, and bounds checking, before returning the record denotation. |
| Select an ordinary field through a nonempty selected record path | After successful lookup and path selection, add 1 + L(selected field value) before returning it. |
| Ordinary `min`, `max`, or `sum` | For each argument in order, evaluate it, reject an unacceptable nonempty type, then add L(argument) once before processing its integer leaves. |
| Ordinary `distinct` | For each argument in order, evaluate it, reject an unacceptable nonempty type, then add L(argument) once before gathering its leaves. Gather all arguments before detecting duplicates. |
| Successfully index an ordinary array | After base/index validation, bounds checking, and element selection, add 1 + L(selected element or row) before returning it. |
| Visit an ordinary-array comprehension element | Add 1 + L(element or row) before evaluating its body. |
| Visit an explicit-record-list comprehension element | Add 1 before evaluating that record's body. |
| Select a row for a lifted-check dependency | Add 1 + L(selected row), separately for each participating dependency at each selected level, before recursion or condition evaluation (§10.4). |

Executed operand, domain, and index expressions MUST incur their own
charges in addition to these charges. For example, `rows[0].x` costs 1
for selecting the record and 2 for selecting a scalar field. Ordinary
`a[0]` costs 2 for a scalar or 1 for an empty row. An aggregate consuming
a selected row adds its own leaf charge after the selection charge.
An empty aggregate argument adds 0, but evaluating it can still charge.

Every unlisted operation MUST add zero independent expression units.
This includes literals, host constants, ordinary name lookup, copying or
redefining a whole named value, truth conversion, unary and binary
arithmetic, comparison, integer exponentiation steps, scalar reads,
offsets, `defined`, and `size` or ordinary `count`. Charged subexpressions
and lifted-check traversal still apply. A field resolving directly to a
currently available value with no selected record path adds zero. A field
that navigates to a structural nested list adds no field-value charge by
itself, though evaluating its base can charge. A lifted ordinary name
already selected under §10.4 adds no name-lookup charge. Internal metadata
construction and lookup MUST NOT introduce extra charges.

## 10.4 Lifted-check traversal

The [record-resolution rules](08-expressions-and-checks.md#812-record-names-and-field-access)
determine which dependencies a check lifts and their remaining repeat
chains. For accounting, implementations MUST use this ordered traversal:

1. Collect dependencies during left-to-right syntax resolution, including
   operands later skipped by short circuit. Apply chapter 8's shallow
   whole-array exceptions and remaining-repeat rules. An ordinary-name
   dependency identifies that name and its remaining repeat chain. An
   explicit `each` name instead identifies the named repeat's logical
   record-start marker tree and its remaining chain. Deduplicate repeated
   occurrences of the same dependency, retaining first-encounter order.
   Distinct ordinary field names remain distinct dependencies even when
   they belong to the same records.
2. For the marker tree, use the enclosing array levels of that repeat's
   completed records and one scalar marker per record of the named repeat.
   Each marker contributes one leaf to L; its numeric value or input
   position does not affect the charge. No particular storage format is
   required.
3. Obtain available dependencies under chapter 8's lookup rules and choose
   the deepest available remaining repeat chain. An unavailable potential
   collection supplies no traversal level; a reached unavailable reference
   still fails. If no dependency is available, evaluate the condition once
   without lifted-row charges.
4. At each remaining repeat level, a dependency participates exactly when
   it is available and its chain extends beyond that level. Validate the
   participating arrays and their alignment before selecting any row at
   that level, under [§8.13](08-expressions-and-checks.md#813-checks-and-failure-witnesses).
   Visit row indices in increasing order. For each row, select the row of
   each participating dependency in dependency order, adding 1 + L(row)
   separately for each selection. Then recurse to the next level. Evaluate
   the condition only at the deepest selected level.
5. Stop at the first false condition or hard failure. An empty available
   collection or subcollection executes no condition. Restore outer
   dependency selections between sibling rows, retaining accumulated effort.

A nested row includes its descendant leaves: they can contribute to the
charge at that level and again at deeper levels. An explicit `each`
dependency charges marker rows; subsequent ordinary field selections also
incur §10.3's charges when executed.

For one record with empty-array fields, `check 1 || a || a` adds one
lifted-row unit, while `check 1 || a || b` adds two when a and b are
distinct fields. Neither right operand executes. With an explicit record
name, `check 1 || row.a || row.b` adds the marker-row charge of 2 for that
record, even though neither field selection executes. These are traversal
charges before the condition, so short circuit MUST NOT remove them.

## 10.5 Evaluation, instances, and catalog

Charges MUST follow [chapter 8's evaluation order](08-expressions-and-checks.md#81-values-and-value-lookup)
and [chapter 6's statement evaluation order](06-types-and-values.md#67-offsets-and-evaluation-order).
An unevaluated short-circuit operand adds no expression-event charges;
§10.4's preselection still occurs before evaluating a lifted condition.

[Comprehensions](08-expressions-and-checks.md#810-comprehensions-and-bindings)
evaluate their domain once and charge only reached outer elements, before
their bodies. `all` and `any` stop when decided; `sum` and `count` traverse
all elements unless a failure stops them. A visited empty row costs 1;
an empty domain visits nothing. Recognizing an ordinary-array domain as
records for a diagnostic MUST NOT change it to the explicit-record-list
charging rule.

An ordinary aggregate's complete argument charge precedes that argument's
arithmetic. Budget exhaustion can therefore precede arithmetic overflow
within the same argument. An unacceptable nonempty type fails before that
argument's leaf charge. `distinct` MUST NOT stop evaluating arguments when
duplicates could already be detected.

Every reached [parameter default](09-params-and-instances.md#92-parameter-evaluation-and-conversion)
MUST execute and charge even when the caller overrides it. Parameter
conversion adds no independent charge and does not increase N. Unexecuted
declarations and defaults add no charges. Repeated statements incur their
normal charges on every execution, including reads, discards, definitions,
and expressions for counts, indices, bounds, layouts, offsets, and committed
capture bounds. Traversing an instance without retaining its data MUST NOT
exempt any executed work.

One counter pair MUST span the prefix and every traversed instance.
The full input's N sets the effort limit once; selection MUST NOT substitute
the selected instance's length. [Explicit selection](09-params-and-instances.md#94-instance-selection)
ends future execution and charges after the selected body. Without a
selector, later traversal remains charged, and budget exhaustion can
precede `E-INSTANCE-SELECTION`.

[Catalog](09-params-and-instances.md#96-catalog-results) MUST execute the
prefix and all bodies with the same accounting. Constructing its returned
`instances`, `name`, and `failed` entries adds no independent units.
Budget exhaustion MUST abort catalog without a partial result.

A [deferred rejection](09-params-and-instances.md#95-deferred-failures-in-skipped-instances)
MUST NOT refund work or suppress later execution. Both budget failures
are hard errors, including during skipped instances and catalog. A reached
budget failure remains primary ahead of a prior deferred rejection; the
reference's secondary earlier-error summary MUST NOT change its code or
record path.

## 10.6 Exhaustion and error locations

Both budgets MUST report the existing [`E-BUDGET`](errors.md#115-read-time-codes)
with [profile R](errors.md#1132-location-profiles) and null `record_path`.
Recommended main messages are:

| Budget | Message |
|---|---|
| Idle work | `too many repetitions or empty rows without reading input` |
| Expression effort | `the expressions do too much work for the size of the input` |

The input location MUST be the actual current cursor when the excess charge
occurs, using [§4.6's byte coordinates](04-input.md#46-input-positions).
Implementations MUST NOT perform normal skipping solely to relocate this
error or substitute a saved record or check-witness start. They MUST retain
the active reader-spec line, null reader-spec column, and active
outer-to-inner execution context.

An array's read-name context remains active during its empty-row charge.
A counted iteration's context remains active during its post-body charge.
Its reader-spec line MUST be the last executed body statement's line, or
the repeat's current line when the body is empty; implementations MUST NOT
restore the repeat's line before this charge. Expression charges retain
their enclosing statement's line. Lifted-check preselection failure has no
check witness, even if some rows were selected successfully.

Failure timing MUST follow the charge tables and
[error order](errors.md#112-error-order). Parsing, unknown parameters, and
input-wide UTF-8 validation precede execution budgets. An earlier execution
error prevents later charges.

## 10.7 Executable boundary examples

These first six examples have zero-byte input. The idle totals are B−1,
B, and B+1. Success returns no entries; the first excessive iteration fails.

```iris example
id: budgets-idle-under
--- spec
repeat 1048575 {}
--- input
--- expected data
{"iris":"0.1","entries":[]}
```

```iris example
id: budgets-idle-equal
--- spec
repeat 1048576 {}
--- input
--- expected data
{"iris":"0.1","entries":[]}
```

```iris example
id: budgets-idle-over
--- spec
repeat 1048577 {}
--- input
--- expected error
{"code":"E-BUDGET","message":"too many repetitions or empty rows without reading input","input_location":{"origin":"input.bin","line":1,"column":1},"spec_location":{"origin":"spec.iris","line":1,"column":null},"context":"repeat [1048577 of 1048577]","record_path":null}
```

The common prefix below costs 1023*(1+1024) = 1048575 expression units
and 1023+1024 = 2047 idle units. The explicit record lists have no public
entries. Ordinary `sum(1)` adds 1; `sum(1,1)` adds two successive units,
with its second argument crossing the limit.

```iris example
id: budgets-effort-under
--- spec
repeat 1023 as xs {}
repeat 1024 as ys {}
check all(x in xs: all(y in ys: 1))
--- input
--- expected data
{"iris":"0.1","entries":[]}
```

```iris example
id: budgets-effort-equal
--- spec
repeat 1023 as xs {}
repeat 1024 as ys {}
check all(x in xs: all(y in ys: 1))
check sum(1)==1
--- input
--- expected data
{"iris":"0.1","entries":[]}
```

```iris example
id: budgets-effort-over
--- spec
repeat 1023 as xs {}
repeat 1024 as ys {}
check all(x in xs: all(y in ys: 1))
check sum(1,1)==2
--- input
--- expected error
{"code":"E-BUDGET","message":"the expressions do too much work for the size of the input","input_location":{"origin":"input.bin","line":1,"column":1},"spec_location":{"origin":"spec.iris","line":4,"column":null},"context":null,"record_path":null}
```

Reading a value does not reset idle work. The final repeat fails at the
cursor just after `0`, without skipping or moving to a saved record start.

```iris example
id: budgets-idle-no-reset
--- spec
repeat 1048576 {}
_:int
repeat 1 {}
--- input no-final-newline
0
--- expected error
{"code":"E-BUDGET","message":"too many repetitions or empty rows without reading input","input_location":{"origin":"input.bin","line":1,"column":2},"spec_location":{"origin":"spec.iris","line":3,"column":null},"context":"repeat [1 of 1]","record_path":null}
```

The positive prefixes below cost 1024 + 1024*1023 = B even though no
scalar leaves are read or stored.

```iris example
id: budgets-idle-empty-prefixes
--- spec
_[1024][1023][0]:int
--- input
--- expected data
{"iris":"0.1","entries":[]}
```

The ignored input below is two UTF-8 bytes. The effort limit is B+32;
the prefix costs B−1 and the 33 scalar arguments add 33, reaching equality.

```iris example
id: budgets-effort-raw-bytes
--- spec
ignore trailing
repeat 1023 as xs {}
repeat 1024 as ys {}
check all(x in xs: all(y in ys: 1))
check sum(1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1)==33
--- input hex
c3 a9
--- expected data
{"iris":"0.1","entries":[]}
```

The last example preserves the final executed body statement's line 3
for post-body idle exhaustion, although the repeat begins on line 2.

```iris example
id: budgets-idle-body-location
--- spec
repeat 1048576 {}
repeat 1 {
  if 1 {}
}
--- input
--- expected error
{"code":"E-BUDGET","message":"too many repetitions or empty rows without reading input","input_location":{"origin":"input.bin","line":1,"column":1},"spec_location":{"origin":"spec.iris","line":3,"column":null},"context":"repeat [1 of 1]","record_path":null}
```
