# 11 Error reporting and code registry

## 11.1 Reporting and comparison

This chapter assigns stable codes to language errors. Implementations MUST
use the registered code for the corresponding failure. The canonical error
document, location nulls, witness paths, and compared fields are defined in
[§§3.7–3.8](03-data-model.md#37-canonical-error-document). Conformance compares
exactly the code, primary line/column pair, and `record_path`; message
wording, origins, context text, and secondary coordinates are not compared.

The pinned Gecode reference reports text through `Instance::Error`. A runtime
diagnostic starts with `origin:line:column: message`, then an input excerpt
and a caret line. The excerpt has a right-aligned line number, ` | `, and
at most the first 100 bytes of the line followed by `...` when truncated.
The caret marks the reported column. Active execution frames follow on a
`while reading: ` line, outermost first, separated by ` > `; iterations use
human-facing numbers starting at 1. A `spec: origin:line` suffix identifies
the reader-spec line when available. Parse diagnostics instead start with
`origin:line: message` and have no input excerpt or caret. Unknown-parameter
diagnostics supply only the reader-spec origin.

The Gecode conformance driver MUST separate this envelope from the main
message before applying the phase's patterns in §§11.4–11.5. An
`after an earlier error in the instance` note is secondary material: it
MUST NOT replace the primary code or create a check witness. Dynamic strings
can contain quotes, newlines, and diagnostic-looking text without unambiguous
escaping; the driver MUST NOT assume that every apparent suffix inside such
text is an envelope boundary. [§4.6](04-input.md#46-input-positions) defines
input coordinate counting. Implementations MUST preserve reported coordinates
and MUST NOT move a check's location to the final input cursor.

## 11.2 Error order

Implementations MUST report the first error encountered in the order below,
subject to the deferral rules. This preserves the pinned parse, resolve,
and execution order while adding Iris source and input validation.
Implementations MUST NOT choose an error by sorting candidate input coordinates.

1. Parse the reader spec before reading input. First validate its whole
   source as UTF-8 under [§5.1](05-reader-specs.md#51-reader-specs-and-parsing);
   `E-SPEC-UTF8` precedes every other parse-time error. Then lex the complete
   reader spec, parse statements, check global name conflicts, then resolve
   names. A later lexical error can precede an earlier syntax error. Resolver prechecks
   precede the expression traversal they guard; earliest reader-spec line
   alone does not determine the error.
2. Before statement execution, reject unknown parameters in supplied parameter
   order, then validate the whole input as UTF-8 under
   [§4.10](04-input.md#410-utf-8-validity). Input-wide `E-UTF8` precedes every
   execution error, even one at an earlier input coordinate. Parse-only
   operations do not validate input.
3. Execute statements in order along the selected control-flow path.
   Reads encounter tokens in consumption order; checks run where written.
   Value-level UTF-8 checks follow this execution order and are hard errors.
4. Evaluate expression operands from left to right, with short-circuit `&&`
   and `||`. For a scalar subscript, check that the base is an array before
   evaluating the index. Visit repeat iterations and check record/element
   indices in increasing traversal order, stopping at the first failing
   witness.
5. Try sections arms in declaration order; the first matching arm handles the
   line. Check missing required arms in declaration order when sections ends.
6. Check trailing input only after successful execution, unless explicit
   instance selection already returned or `ignore trailing` applies.

### 11.2.1 Deferred instance failures and catalog

Only a false check, a repeated sections line, or a missing required sections
line can reject an instance without immediately stopping reading. An error
while evaluating a check is a hard error, not a false check.

A rejection MUST be deferred when the current instance may be skipped: during
catalog, for a different selected number, for a different or not-yet-known
selected name, or after the first instance when no selection was supplied.
Store only the first rejection of each instance and continue reading.
A repeated sections line still supplies captures and executes its arm body;
a deferred missing-arm rejection returns from that sections statement.
If name selection later identifies the rejected instance as selected,
report its stored diagnostic with its original coordinates and witness.

A hard error MUST stop reading even in a skipped instance. If that instance
already has a deferred rejection, the hard error remains primary and Gecode
appends an `after an earlier error in the instance` summary. A missing required
sections line MUST be reported immediately if no more data remains, even
during catalog or in an otherwise skipped instance. Its saved location is
the sections terminator, last matching line, or end of input.

Explicit selection returns after the selected instance, without executing
statements against later input. Input-wide UTF-8 validation has already
examined all supplied bytes. Without selection, reading continues to
determine whether the file contains more than one instance, and reports
`E-INSTANCE-SELECTION` when necessary.

Catalog reads all instances and returns the first rejection summary for each
in the string array `failed`, using an empty string for a valid instance.
It MUST omit `failed` if all summaries are empty. A check summary is its
explicit message, or `check on line N of the spec` when absent. A repeated-line
summary omits the first-line parenthesis; a missing-line summary is
`no line matching PATTERN`. These are data strings, not canonical error
documents. Catalog still aborts on a hard error or the end-of-input missing-arm
exception. It emits `name` only when every instance has a nonempty usable
name. Chapter 9 (`09-params-and-instances.md`, planned) defines instance
selection and catalog data in full.

## 11.3 Registry conventions

Codes use `E-` followed by uppercase ASCII letters or digits in
hyphen-separated groups, with a letter first. Parse-time codes start with
`E-SPEC-`; read-time codes name the failed operation or invariant. Sites
with the same failure meaning share a code. The
[error schema](../schema/error.schema.json) checks code syntax, not registry
membership. `E-UNMAPPED` is an adapter failure sentinel and MUST NOT be
reported as a language error code.

### 11.3.1 Message patterns and recommended wording

Each table row gives a meaning, recommended message templates, location
profile, defining chapter/topic, and all pinned source origins. The displayed
message templates also describe Gecode's main messages for the adapter;
semicolon-separated templates are alternatives. Message wording is
RECOMMENDED, not required: an implementation MAY use different wording.
Substitute the actual values and retain the variant for the failed operation.

To expand a template, escape every fixed character as a regular-expression
literal and substitute the placeholders below. Use `std::regex` with
`ECMAScript` syntax and `std::regex_match` against the complete extracted main
message. Apply only the table for the diagnostic's phase. Restrictions stated
in a row narrow its placeholders. A message MUST map to exactly one code;
several matching alternatives of that same code are harmless. An unmapped
message is an adapter failure.

| Placeholder | Expansion or meaning |
|---|---|
| `{N}` | `[A-Za-z_][A-Za-z0-9_]*`, a reader-spec identifier. |
| `{I}` | `-?[0-9]+`, a signed integer. |
| `{U}` | `[0-9]+`, an unsigned integer. |
| `{A}` | `[\s\S]*`, arbitrary dynamic bytes, including newlines. |
| `{Q}` | Apostrophe, `{A}`, apostrophe: Gecode's single-quoted text. |
| `{P}` | Double quote, `{A}`, double quote: a rendered line pattern. |
| `{T}` | One of `end of spec`, `{Q}`, or `{P}`. |
| `{F}` | One of `end of input` or `{Q}`. |
| `{B}` | One of `int`, `float`, `a number`, `a word`, `the rest of the line`, `a symbol`, or `an int in {I?}..{I?}`; either integer bound can be absent. |
| `{D}` | `{B}` or `{N} ({B})`, including a named reading type. |

These placeholders describe diagnostics, not the language grammar. Unknown
statement and field tokens use `{A}` because the lexer also accepts tokens
such as `gecode.foo` in those positions. Quoted dynamic text can itself
contain quotes; `{A}` MUST NOT be implemented as `.*` or as a negated-quote character class.

### 11.3.2 Location profiles

Registered codes use these location profiles. For Gecode-backed errors,
availability follows the reference; §11.6 defines Iris-only availability.
Origins come from the supplied source identities and are null when
unavailable. Implementations MUST follow
[chapter 3](03-data-model.md#37-canonical-error-document) for canonical
representation and primary-location selection.

| Profile | Location availability; null fields |
|---|---|
| P — parse-time | No input location. Reader-spec origin and line, with null column. Null context and path. |
| R — read-time | Input origin, positive line and column; reader-spec origin and line, with null column. Active execution-context text when present, otherwise null. Null path. |
| C — failed check | Locations and context as R. Path is the reported witness converted to outermost-first indices starting at 0, or `[]` without witness indices. |
| U — unknown parameter | No input location. Reader-spec origin, with null line and column. Null context and path. |
| T — input only | Input origin, positive line and column. Null reader-spec location, context, and path. Used for trailing input and Iris input-wide UTF-8 validation. |

Preserve the reported reader-spec line, including a nested statement's line.
Only `E-CHECK-FAILED` uses C; errors evaluating its condition use R.
`Interpreter::run` resets the reader-spec line before `E-TRAILING-INPUT`.
For `E-UTF8`, Iris defines availability using T or R as specified in §11.6;
Gecode supplies no corresponding diagnostic.

### 11.3.3 Adding a code

Authors MUST search existing meanings first and reuse a code for the same
failure. A new meaning MUST have one authoritative row here, with its phase,
location availability, recommended message, defining chapter/topic, pinned
source sites, and every adapter pattern. Verify a focused example when
reachable, and mark deliberate differences and no-pattern exceptions
explicitly. Later chapter tasks MUST update this registry rather than define
codes locally. References to planned chapters use filenames and topics;
those tasks replace them with links after choosing their own headings.

## 11.4 Parse-time codes

All rows use profile P. Source abbreviations are defined in §11.7.

| Code | Meaning; recommended messages and Gecode patterns | Location | Source origins | Defining place |
|---|---|---|---|---|
| `E-SPEC-STRING` | Invalid reader-spec string. `unterminated string`; `unknown escape '{A}' in string (use {A})` | P | s:122,124,176,187 | [§5.3](05-reader-specs.md#53-strings): strings |
| `E-SPEC-NUMBER` | Invalid or unrepresentable reader-spec numeric literal. `bad number {A}` | P | s:204 | [§5.5](05-reader-specs.md#55-numbers): numbers |
| `E-SPEC-CHARACTER` | Unrecognized reader-spec character. `unexpected character '{A}'` | P | s:222 | [§5.2](05-reader-specs.md#52-white-space-and-comments): source lexing |
| `E-SPEC-PATTERN` | Malformed pattern/capture or invalid capture type. `unterminated capture in pattern {P}`; `missing or bad name in pattern {P}`; `capture name {Q} in pattern {P} is not a name`; `'...' must end the pattern {P}`; `a capture cannot be of an alphabet type`; `a capture needs a type after ':'`; `unexpected {T} in the capture type {Q}` | P | s:299,306,314,320,495,505,511 | chapter 7 (`07-structure.md`, planned): patterns |
| `E-SPEC-SYNTAX` | Required syntactic item absent or unrecognized statement. `expected {A}, found {T}`; `missing '}'`; `unknown statement '{A}'`; `'..<' needs an upper bound`; `a range needs a lower or an upper bound` | P | s:369,455,460,465,477,529,632,640,749,758,760,777,918,1172 | [§§5.6–5.7](05-reader-specs.md#56-statements-declarations-and-blocks): statement and type syntax; [§6.4](06-types-and-values.md#64-integer-ranges): ranges |
| `E-SPEC-PLACEMENT` | Statement/setting used in a prohibited position. `'instances' must be the last statement of the spec`; `the setting '{N}' must be at the top level of the spec`; `'each' must be the first statement of a repeat, and comes at most once`; `'instances' must be at the top level of the spec` | P | s:378,642,894,900 | [§5.11](05-reader-specs.md#511-placement-restrictions): placement |
| `E-SPEC-NAME` | Reserved or discard name used where a real definition/binding is required. `'{A}' cannot be defined: names starting with 'gecode.' are Gecode constants`; `'_' cannot be defined here`; `'_' cannot be bound` | P | s:472,667,1108 | [§5.4](05-reader-specs.md#54-names): names; chapter 8 (`08-expressions-and-checks.md`, planned): comprehensions |
| `E-SPEC-NAME-CONFLICT` | Conflicting type, value, record, or comprehension names. `'{N}' is bound by a comprehension but is also a name in the spec`; `'{N}' names a type, so it cannot also name a value`; `'{N}' is a base type`; `the type '{N}' is already defined`; `'{N}' is already bound by an enclosing comprehension`; `'{N}' already names records`; `'{N}' names records, so it cannot also be defined`; `'{N}' cannot name both the list and each record`; `'{N}' is bound by a comprehension but names records` | P | s:388,392,835,837,1111; r:208,211,220,550 | [§6.5](06-types-and-values.md#65-named-types-and-visibility): named types; chapter 8 (`08-expressions-and-checks.md`, planned): comprehensions, record-name resolution |
| `E-SPEC-UNKNOWN-TYPE` | Reading type name is not known at this point. `unknown type '{N}'` | P | s:748 | [§6.5](06-types-and-values.md#65-named-types-and-visibility): named types |
| `E-SPEC-TYPE` | Reading type incompatible with a declaration/list/option. `'offset' needs numbers`; `'rest' values cannot be arrays`; `an alphabet type needs a two-dimensional array`; `'layout' needs a two-dimensional array of numbers`; `a list cannot be of 'rest' or of an alphabet type` | P | s:793,807,809,812,823 | [§6.1](06-types-and-values.md#61-reading-types-and-resulting-entries): permitted forms; [§6.6](06-types-and-values.md#66-fixed-size-arrays-and-empty-rows): arrays; [§6.7](06-types-and-values.md#67-offsets-and-evaluation-order): offsets; [§6.8](06-types-and-values.md#68-matrix-layouts): layouts; [§6.9](06-types-and-values.md#69-sentinel-and-end-of-line-lists): lists; [§6.10](06-types-and-values.md#610-alphabets): alphabets |
| `E-SPEC-NESTING` | Reader-spec structural/expression nesting limit exceeded. `blocks are nested too deeply`; `expressions are nested too deeply` | P | s:519 | [§5.12](05-reader-specs.md#512-parser-limits): parser limits |
| `E-SPEC-FUNCTION` | Unknown function or invalid call form/arity. `'{N}' needs the form {N}(x in a: e)`; `unknown function '{N}'`; `wrong arguments for function '{N}'` | P | s:1069,1071,1073 | chapter 8 (`08-expressions-and-checks.md`, planned): functions, comprehensions |
| `E-SPEC-CONSTANT` | Unknown Gecode host constant. `unknown Gecode constant '{A}' (known: gecode.int_max, gecode.int_min)` | P | s:1089 | chapter 8 (`08-expressions-and-checks.md`, planned): value lookup |
| `E-SPEC-RECORD-SCOPE` | Record/list/field reference outside its permitted repeat scope. `'{N}' is inside a repeat without a list name in the records of '{N}'; name that list with 'as' to reach it`; `'{N}' cannot be used inside its own repeat`; `the field '{N}' cannot be used inside the repeat that reads it`; `'{N}' is a list in each record of the repeat around it; {A}`; `'{N}' is each record of a repeat, so only checks can use it outside that repeat` | P | r:256,259,323,459,464,479 | chapter 8 (`08-expressions-and-checks.md`, planned): record-name resolution |
| `E-SPEC-FIELD` | Invalid field selection or ambiguous field definition. `the records of '{N}' have no field '{A}' ({A})`; `'{N}' is defined inside and outside the repeat of '{N}', so it cannot be a field of its records`; `only records have fields such as '.{A}'; {A}`; `'{N}' is a list of records, so '.{A}' needs one of them: {A}` | P | r:315,319,491,494 | chapter 8 (`08-expressions-and-checks.md`, planned): record-name resolution |
| `E-SPEC-RECORD-VALUE` | Record/list used as a value, indexable record, or comprehension list where prohibited. `a record of '{N}' is not a value; {A}`; `'{N}' is a list of records, not a value; {A}`; `a record of '{N}' cannot be indexed; {A}`; `a record of '{N}' is not a list; use one of its fields` | P | r:331,333,512,554 | chapter 8 (`08-expressions-and-checks.md`, planned): record-name resolution |
| `E-SPEC-CHECK-SCOPE` | Check attempts to range over incompatible or ambiguous repeat definitions. `a check can only range over the records of one repeat, but '{N}' and '{N}' are defined in different repeats`; `'{N}' is defined in several repeats, or inside and outside a repeat, so a check here cannot range over its records` | P | r:396,589 | chapter 8 (`08-expressions-and-checks.md`, planned): checks |

## 11.5 Read-time codes

The location column selects the profile in §11.3.2.

| Code | Meaning; recommended messages and Gecode patterns | Location | Source origins | Defining place |
|---|---|---|---|---|
| `E-BUDGET` | A work budget is exhausted. `too many repetitions or empty rows without reading input`; `the expressions do too much work for the size of the input` | R | i:230; e:210 | chapter 10 (`10-work-budgets.md`, planned): idle work, expression effort |
| `E-CHECK-FAILED` | A check condition is false. `check failed`; `check failed: {A}`; `check failed for {A}` | C | e:826 | chapter 8 (`08-expressions-and-checks.md`, planned): checks; chapter 9 (`09-params-and-instances.md`, planned): deferred failures |
| `E-CHECK-RECORDS` | Runtime record collections cannot be aligned for a check. `'{N}' has no records`; `'{N}' and '{N}' have different numbers of records` | R | e:752,757 | chapter 8 (`08-expressions-and-checks.md`, planned): checks |
| `E-COMPARISON-TYPE` | Comparison operands violate scalar/numeric requirements. `'{A}' compares single values, not arrays`; `'{A}' compares numbers` | R | e:476,480 | chapter 8 (`08-expressions-and-checks.md`, planned): comparisons |
| `E-COUNT` | A size/count/base expression is not a permitted count. `{A} must be an integer`; `{A} is negative ({I})`; `{A} is too large ({I})`, restricting the initial description to `array size`, `number of lines`, `index base`, or `repeat count` | R | e:708,710,712 | [§6.6](06-types-and-values.md#66-fixed-size-arrays-and-empty-rows): arrays; chapter 7 (`07-structure.md`, planned): skip and expect, repeats, index |
| `E-DIVISION-ZERO` | Integer division/remainder by zero. `division by zero` | R | e:462 | chapter 8 (`08-expressions-and-checks.md`, planned): arithmetic |
| `E-EXPECTED-TOKEN` | Required literal token absent or different. `expected {Q}, found {F}` | R | i:812,857,862 | [§6.9](06-types-and-values.md#69-sentinel-and-end-of-line-lists): lists; chapter 7 (`07-structure.md`, planned): skip and expect |
| `E-EXPECTED-VALUE` | Input cannot satisfy the required reading type/domain, including a range, or the required line of values is absent. Includes overflow on numeric input conversion and rejected float underflow. `expected {D}, found {F}`; `expected a line of values, found end of input` | R | i:259,260,318,765 | [§4.4](04-input.md#44-integer-tokens), [§4.5](04-input.md#45-floating-point-tokens): numeric conversion; [§6.2](06-types-and-values.md#62-scalar-reading-and-numeric-promotion): reading types; [§6.4](06-types-and-values.md#64-integer-ranges): ranges; [§6.6](06-types-and-values.md#66-fixed-size-arrays-and-empty-rows): arrays; [§6.9](06-types-and-values.md#69-sentinel-and-end-of-line-lists): lists |
| `E-EXPONENT` | Integer exponent is negative. `negative exponent in '^'` | R | e:456 | chapter 8 (`08-expressions-and-checks.md`, planned): arithmetic |
| `E-FIELD-UNDEFINED` | A field is unavailable in the selected runtime record. `'{N}' is not defined in this record of '{N}'` | R | e:358 | chapter 8 (`08-expressions-and-checks.md`, planned): value lookup |
| `E-FIND-MATCH` | No remaining line matches a find pattern. `no line matching {P} found` | R | i:840 | chapter 7 (`07-structure.md`, planned): line and find |
| `E-FUNCTION-EMPTY` | min/max receives no integer leaves. `'{N}' of no integers`, restricting the function to `min` or `max` | R | e:534 | chapter 8 (`08-expressions-and-checks.md`, planned): functions |
| `E-FUNCTION-TYPE` | Function/comprehension operand has the wrong type or shape. `'{N}' takes an array`; `'{N}' takes integers`; `'{N}({N} in ...)' needs an array` | R | e:509,521,555,664 | chapter 8 (`08-expressions-and-checks.md`, planned): functions, comprehensions |
| `E-INDEX-CONTEXT` | index statement executes outside a repeat. `'index' must be inside a repeat` | R | i:869 | chapter 7 (`07-structure.md`, planned): index |
| `E-INDEX-MISMATCH` | Input row index differs from the expected repeat index. `expected index {I}, found {I}` | R | i:876 | chapter 7 (`07-structure.md`, planned): index |
| `E-INDEX-RANGE` | Array/record-list index is outside its bounds. `index {I} is out of range for {A} (size {U}, indices start at 0)` | R | e:334,610 | chapter 8 (`08-expressions-and-checks.md`, planned): indexing |
| `E-INDEX-SCALAR` | Attempt to index a scalar. `only arrays can be indexed, not {A}` | R | e:607 | chapter 8 (`08-expressions-and-checks.md`, planned): indexing |
| `E-INDEX-TYPE` | Index expression is not an integer scalar. `an index must be an integer` | R | e:585 | chapter 8 (`08-expressions-and-checks.md`, planned): indexing |
| `E-INSTANCE-EMPTY` | Reading requires an instance but none exists. `the file contains no instance` | R | i:1046 | chapter 9 (`09-params-and-instances.md`, planned): instance selection |
| `E-INSTANCE-NOT-FOUND` | Requested instance number/name does not exist. `there is no instance {Q} (the file has {U} instance{A})`, with the final suffix restricted to empty or `s` | R | i:1048 | chapter 9 (`09-params-and-instances.md`, planned): instance selection |
| `E-INSTANCE-NUMBER` | Numeric instance selection is below one. `instance numbers start at 1` | R | i:999 | chapter 9 (`09-params-and-instances.md`, planned): instance selection |
| `E-INSTANCE-SELECTION` | Multiple instances require an explicit selection. `the file has {U} instances; select one with the parameter instance (a number from 1, or a name)` | R | i:1051 | chapter 9 (`09-params-and-instances.md`, planned): instance selection |
| `E-INTEGER-OVERFLOW` | Checked integer arithmetic or value adjustment overflows. `integer overflow in '{A}'`; `integer overflow in the upper bound of a range`; `integer overflow when applying offset`; `integer overflow in the value of a symbol` | R | e:426,469,526,687; i:356,369,489,1228 | [§6.4](06-types-and-values.md#64-integer-ranges): ranges; [§6.7](06-types-and-values.md#67-offsets-and-evaluation-order): offsets; [§6.10](06-types-and-values.md#610-alphabets): alphabets; chapter 8 (`08-expressions-and-checks.md`, planned): arithmetic, functions, comprehensions |
| `E-INTEGER-TYPE` | Integer arithmetic requires integer scalar operands. `'{A}' needs integers`, restricting the dynamic operation to the arithmetic operators or `sum` | R | e:367 | chapter 8 (`08-expressions-and-checks.md`, planned): arithmetic, comprehensions |
| `E-LAYOUT` | Layout value, name, or matrix shape is invalid. `matrix layout must be a string`; `layout {A} needs a square matrix`; `unknown matrix layout {Q}` | R | i:438,450,725 | [§6.8](06-types-and-values.md#68-matrix-layouts): layouts |
| `E-LINE-MATCH` | A required line-pattern match is unavailable, either for line or for a required sections arm. `expected a line matching {P}`; `expected a line matching {P}, found end of input`; `expected a line matching {P} before {A}`. This grouping is required because line and missing-sections failures can have identical complete diagnostics; see below. | R | i:833,1193 | chapter 7 (`07-structure.md`, planned): line and find, sections; chapter 9 (`09-params-and-instances.md`, planned): deferred failures |
| `E-LIST-TERMINATOR` | List terminator has an invalid type/shape. `list terminator must be a single value`; `a list of words needs a string or integer terminator` | R | i:797,802 | [§6.9](06-types-and-values.md#69-sentinel-and-end-of-line-lists): lists |
| `E-NAME-UNDEFINED` | Referenced value/record collection is not defined yet. `'{N}' is not defined yet` | R | e:259,280,312,347,353; i:615 | chapter 8 (`08-expressions-and-checks.md`, planned): value lookup; chapter 7 (`07-structure.md`, planned): patterns |
| `E-NO-PROGRESS` | An instance or indefinite repeat body consumes no input. `an instance must read some input`; `the body of a repeat until read no input` | R | i:1008,1089 | chapter 7 (`07-structure.md`, planned): progress; chapter 9 (`09-params-and-instances.md`, planned): instance selection |
| `E-OFFSET-TYPE` | Offset expression is not an integer scalar. `offset must be an integer` | R | i:500 | [§6.7](06-types-and-values.md#67-offsets-and-evaluation-order): offsets |
| `E-PARAM-MISSING` | Required parameter was not supplied. `parameter '{N}' must be given` | R | i:691 | chapter 9 (`09-params-and-instances.md`, planned): parameters |
| `E-PARAM-TYPE` | Supplied parameter cannot convert to its default's numeric type. `parameter '{N}' must be an integer, not {Q}`; `parameter '{N}' must be a number, not {Q}` | R | i:702 | chapter 9 (`09-params-and-instances.md`, planned): parameters |
| `E-PARAM-UNKNOWN` | Caller supplied a parameter not declared by the reader spec. `the spec has no parameter '{A}'` | U | i:146 | chapter 9 (`09-params-and-instances.md`, planned): parameters |
| `E-PATTERN-REFERENCE` | Current-value pattern reference is neither a string nor an integer scalar. `'{N}' must be a string or an integer` | R | i:620 | chapter 7 (`07-structure.md`, planned): patterns |
| `E-RANGE-BOUNDS` | Range bound is not an integer scalar. `the bounds of a range must be integers` | R | i:375 | [§6.4](06-types-and-values.md#64-integer-ranges): ranges |
| `E-REPEAT-DEPTH` | Repeated values have inconsistent depths. `'{N}' has rows of different depths` | R | d:154 | chapter 7 (`07-structure.md`, planned): repeats |
| `E-REPEAT-MISSING` | Required scalar is absent from some/all nonempty repeat iterations. `'{N}' is defined in only some repetitions`; `'{N}' is defined in no repetition` | R | d:133,143,187 | chapter 7 (`07-structure.md`, planned): repeats |
| `E-REPEAT-TERMINATOR` | Required terminating token is absent before input ends. `expected {Q}{A} before end of input`, restricting the middle part to zero or more ` or {Q}` alternatives | R | i:1080 | chapter 7 (`07-structure.md`, planned): repeats |
| `E-SECTIONS-REPEATED` | A sections arm matches a second line. `a second line matching {P} (the first is on line {U})` | R | i:1175–1176 | chapter 7 (`07-structure.md`, planned): sections; chapter 9 (`09-params-and-instances.md`, planned): deferred failures |
| `E-SECTIONS-UNEXPECTED` | No sections arm matches the current line and skip other is absent. `unexpected line; expected one of {A}` | R | i:1171 | chapter 7 (`07-structure.md`, planned): sections |
| `E-SIZE-LIMIT` | Array/matrix cell count exceeds the supported structural bound. `array is too large`; `matrix is too large` | R | i:386,453,1205 | [§6.6](06-types-and-values.md#66-fixed-size-arrays-and-empty-rows): arrays; [§6.8](06-types-and-values.md#68-matrix-layouts): layouts; [§6.10](06-types-and-values.md#610-alphabets): alphabets |
| `E-SKIP-EOF` | A requested line cannot be skipped at end of input. `cannot skip a line at end of input` | R | i:849 | chapter 7 (`07-structure.md`, planned): skip and expect |
| `E-SYMBOL` | Alphabet reading encounters end of input or an unrecognized symbol. `expected a symbol for row {U}, column {U}, found end of input`; `unexpected symbol {Q}, expected one of {Q}` | R | i:1215,1231 | [§6.10](06-types-and-values.md#610-alphabets): alphabets |
| `E-TRAILING-INPUT` | Disallowed input remains after execution finishes. `unexpected input after the end of the spec: {Q}` | T | i:152 | [§4.8](04-input.md#48-end-of-input-and-trailing-input): ignore trailing |
| `E-TYPE-UNDEFINED` | A parsed named type has not been evaluated on the executed path. `the type '{N}' is not defined yet` | R | i:344 | [§6.5](06-types-and-values.md#65-named-types-and-visibility): named types |
| `E-VALUE-TYPE-MIX` | Repeated/list values mix strings and numbers. `'{N}' mixes strings and numbers`; `'{N}': strings and numbers are mixed` | R | d:161; h:328 | [§6.11](06-types-and-values.md#611-errors-and-related-chapters): value-construction guard; chapter 7 (`07-structure.md`, planned): repeats |

`E-EXPECTED-VALUE` includes conversion failure, absent input, and range
rejection. The same helper reports `expected an int in 1..3, found ...` for
`4`, `oops`, or end of input; the adapter need not reinterpret the token.

`E-LINE-MATCH` covers both `line` and a missing required sections arm because
their diagnostics can be indistinguishable. With input `B` followed by LF,
the reader specs `line "A\" before the line matching \"B"` and
`sections { "A" "B" last }` both report
`expected a line matching "A" before the line matching "B"`, at input `(1,1)`,
reader-spec line 1, with no context. Pattern precedence cannot distinguish
them. A distinct sections diagnostic is an [Iris 1.0 idea](../../docs/1.0-ideas.md).

The `ShapeError` origin in h:328 is a defensive value-construction failure.
It remains covered even where reading-type restrictions may make a route
unreachable. Source coverage does not imply a public reproducer for every
branch.

## 11.6 Iris-only errors

| Code | Meaning and recommended message | Phase and locations | Defining place | Gecode pattern |
|---|---|---|---|---|
| `E-SPEC-UTF8` | The reader-spec source contains invalid UTF-8. `reader spec is not valid UTF-8` | Source validation before lexing or any other parse-time check. Reader-spec origin, LF-counted line and 1-based byte column at the first byte of the first ill-formed subsequence. Null input location, context, and path. Availability is defined by Iris; this extends profile P with a byte column. | [§5.1](05-reader-specs.md#51-reader-specs-and-parsing), [appendix C.2](appendix-c-differences.md#c2-reader-spec-source) | None — Iris-only. |
| `E-UTF8` | The supplied input or an actually read word/rest value contains invalid UTF-8. `input is not valid UTF-8`; `string value is not valid UTF-8` | Input validation after parsing/resolution and unknown parameters, before execution: T. Value-level validation during execution: R. Both locate the first byte of the first ill-formed subsequence. Availability is defined by Iris; Gecode locations are not applicable. | [§4.10](04-input.md#410-utf-8-validity), [§3.6](03-data-model.md#36-strings), [appendix C.1](appendix-c-differences.md#c1-unicode-strings) | None — Iris-only. |

A source-wide `E-SPEC-UTF8` locates the initial byte of a truncated
sequence as well as other ill-formed subsequences. Source columns count
bytes, including tabs as one byte, and reset to 1 after LF. Unlike
Gecode-backed parse errors, this Iris-only parse-time error has a column.
Gecode accepts ill-formed bytes inside reader-spec strings and comments;
its adapter does not emulate `E-SPEC-UTF8`.

Gecode accepts `x: word` with input hex `ff0a` and preserves the byte; it
raises no corresponding error. Iris validates all input bytes, including
ignored bytes, in its input-validation phase. Its value-level check also
covers discarded reads and committed captures because byte-wise settings
can split valid UTF-8. Failed speculative pattern captures do not cause
value-level errors. A value-level `E-UTF8` is a hard error, including in
skipped instances. For a truncated sequence, locate its initial byte.

Input-wide failures use T: the input origin and byte-counted line/column,
with null reader-spec location, context, and path. Value-level failures use
R: the invalid subsequence's input position, active reader-spec origin and
line with null column, current execution context when present, and null
path. [§4.6](04-input.md#46-input-positions) defines coordinate counting.
No Gecode error object is defined for either case. The Gecode adapter does
not emulate this error; if a result cannot be serialized as Unicode JSON,
it reports a distinct adapter diagnostic.

## 11.7 Exclusions and source coverage

This section is informative. The pin is
`5e7bbd5c26b0702c6fd652356b2289da0710ac3c`. Sites are relative to its `src/`:
`s` = `gecode/instance/spec.cpp`, `r` = `gecode/instance/resolve.cpp`,
`e` = `gecode/instance/eval.cpp`, `i` = `gecode/instance/reader.cpp`,
`d` = `gecode/instance/data.cpp`, `h` = `gecode/instance/impl.hpp`.
The source-origin columns in §§11.4–11.5 list every language origin per code.

Count each executable throw, fail call, or diagnostic-origin builder once
at its first source line. Count wrappers separately; omit declarations,
definitions, comments, assertions, and `std::istream::fail()`. The three
builder origins beyond the throw/fail search are e:826, i:1175 (completed at
1176), and i:1193. Helper callers that supply message arguments do not
count the same underlying origin twice.

| File | Language origins | Host/API | Wrapper/rethrow | Total |
|---|---:|---:|---:|---:|
| spec.cpp | 50 | 2 | 3 | 55 |
| resolve.cpp | 20 | 0 | 1 | 21 |
| eval.cpp | 31 | 0 | 2 | 33 |
| reader.cpp | 46 | 0 | 9 | 55 |
| data.cpp | 5 | 12 | 0 | 17 |
| impl.hpp | 1 | 0 | 0 | 1 |
| eval.hpp | 0 | 0 | 0 | 0 |
| instance.hh | 0 | 0 | 0 | 0 |
| **Total** | **153** | **14** | **15** | **182** |

The 153 language origins map to 63 Gecode-backed codes. With `E-UTF8`
and `E-SPEC-UTF8`, the registry has 65 codes. All 182 sites are assigned exactly once.

### 11.7.1 Excluded host/API sites

These 14 sites receive no language code.

| Sites | Reason |
|---|---|
| s:52,55 | Directory path / cannot open file: host filesystem acquisition, outside supplied text. |
| d:89 | Malformed Params assignment string: conformance passes a string map to `Params::set(name,value)`, bypassing this parser. |
| d:112 | Params indexed-access misuse. |
| d:250 | Data lookup of an unknown entry. |
| d:265 | Data scalar/array accessor shape mismatch; two conditional messages at one site. |
| d:270 | Data accessor path index out of bounds. |
| d:280 | Narrowing a 64-bit value to a host int. |
| d:290,300,310 | Wrong scalar accessor type: integer, number, or string. |
| d:328,343 | Wrong numeric array accessor type. |
| d:365 | Data entry index out of bounds. |

The five Column `ShapeError` origins in data.cpp are language errors and
remain registered.

### 11.7.2 Wrappers and rethrows

These 15 sites preserve the originating code.

| Sites | Purpose |
|---|---|
| s:85 | Attach reader-spec origin and line. |
| s:432 | Forward parser failure to fail_at. |
| s:488 | Attach the pattern's reader-spec line to a compilation error. |
| r:134 | Forward resolver failure with the current statement line. |
| e:198 | Attach the runtime envelope and throw. |
| e:203 | Add a value name to ShapeError and forward. |
| i:407,753 | Forward array/list ShapeError with the value name. |
| i:930 | Throw an already constructed rejection immediately. |
| i:958 | Rethrow a hard error unchanged. |
| i:959 | Preserve a hard error and append an earlier deferred rejection summary. |
| i:1017 | Throw a stored rejection when name selection identifies the instance. |
| i:1105,1122 | Add the repeated entry name to Column ShapeError. |
| i:1196 | Throw an already constructed missing-sections diagnostic at end of input. |

### 11.7.3 Helper variants and supplemental exclusions

The audit includes argument variants as well as emission sites:

- s:455,460 receive required punctuation and keywords; s:465 receives name,
  parameter-name, type-name, records-name, each-record-name, and field-name
  descriptions. s:477 receives line-pattern, literal-token, line-prefix,
  token-character, separator-character, alphabet, blank-symbol,
  stripped-symbol, and terminating-token descriptions. s:519 receives
  `blocks` or `expressions`.
- i:259,260 receive the reading-type/domain descriptions at i:67–110,
  through scalar/int-scalar callers; i:873 requests an integer for `index`.
  i:318 covers domain rejection for scalar, captured, and list values.
- e:708,710,712 receive `array size`, `number of lines`, `index base`, or
  `repeat count` from i:719,846,870,1066. e:367 receives arithmetic operators
  or `sum` from e:424,448,686.
- e:826 combines record witnesses (e:798–809) and comprehension witnesses
  (e:636–648); e:814–824 determines their order and saved position.
  i:941 supplies the check's catalog summary. i:1175–1176 adds the first
  matching line to a repeated-line diagnostic; i:1193 builds a missing-arm
  diagnostic, and i:1197 supplies its catalog summary.
- e:43–51 (`quote`), e:165–193 (`located`), i:105–110 (`Domain::describe`),
  and s:434–438 (found-token description) format existing errors.

Outside the 182-site core total, `tools/instance/gecode-instance.cpp`
has four CLI usage origins at lines 180,197,203,220: missing option value,
unknown option, no files, and no reader spec. Line 189 wraps a malformed
Params assignment as usage; line 229 presents `Instance::Error`; line 233
presents an external `std::exception`. None adds a language code.
`gecode/instance.hh:329–334` documents allocation exceptions and host-thrown
`Instance::Error`. Arbitrary host errors, allocation failures, and assertions
are excluded. `eval.hpp` adds no independent diagnostic origin.

## 11.8 Executable examples

The parse example has no input coordinate. The range example uses the same
code as failed integer conversion. The final check reports an earlier
witness position, and reverses the human-facing inner-first record numbers
into an outermost-first path starting at 0.

```iris example
id: errors-parse-syntax
operation: parse
--- spec
n: int
a[2: int
--- expected error
{"code":"E-SPEC-SYNTAX","message":"unknown statement 'a'","input_location":null,"spec_location":{"origin":"spec.iris","line":2,"column":null},"context":null,"record_path":null}
```

```iris example
id: errors-reading-domain
operation: read
--- spec
x: 1..3
--- input
4
--- expected error
{"code":"E-EXPECTED-VALUE","message":"expected an int in 1..3, found '4'","input_location":{"origin":"input.bin","line":1,"column":1},"spec_location":{"origin":"spec.iris","line":1,"column":null},"context":"x","record_path":null}
```

```iris example
id: errors-check-witness
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
{"code":"E-CHECK-FAILED","message":"check failed for record 3 of 'steps' in record 2 of 'jobs': positive","input_location":{"origin":"input.bin","line":3,"column":5},"spec_location":{"origin":"spec.iris","line":6,"column":null},"context":null,"record_path":[1,2]}
```
