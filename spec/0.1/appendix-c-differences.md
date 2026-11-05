# C Differences from Gecode 6.5.0

## C.1 Unicode strings

Iris 0.1 strings MUST be Unicode scalar sequences encoded as UTF-8. The
whole supplied input MUST be valid UTF-8, including comments, skipped lines,
literal pattern text, bytes after a stop or
[selected instance](09-params-and-instances.md#94-instance-selection), and
ignored trailing input. Implementations MUST validate it after parsing/resolution
and unknown-parameter checks, before executing statements. Each `word` or
`rest` value actually read MUST also be valid UTF-8, including discarded
reads and committed captures: byte-wise token settings can split a character
in valid input. These requirements also apply to
[catalog](09-params-and-instances.md#96-catalog-results). Either failure MUST report
[`E-UTF8`](errors.md#116-iris-only-errors), with the scope and order defined
in [§4.10](04-input.md#410-utf-8-validity). This deliberate difference follows
[ADR 0011](../../docs/adr/0011-utf8-strings.md).

The pinned Gecode 6.5.0 reader stores raw bytes: for example, it accepts
`ff0a` (hexadecimal) as a one-byte word followed by LF. Iris rejects that
input before reading the word. It also rejects malformed bytes that Gecode
ignores in comments or unread tails. The
[NUL example](02-notation.md#25-complete-examples) is accepted by
both: NUL is valid Unicode and canonical JSON escapes it as `\u0000`.

Input-wide failures have no reader-spec location or execution context;
value-level failures retain the active reader-spec line and execution
context when present. Both locate the first ill-formed subsequence using
[byte-counted coordinates](04-input.md#46-input-positions). Gecode supplies
no corresponding error or coordinates.

## C.2 Reader-spec source

The whole reader-spec source MUST be valid UTF-8, including strings and
comments. Implementations MUST validate it before lexing or any other
parse-time check and report [`E-SPEC-UTF8`](errors.md#116-iris-only-errors)
at the first byte of the first ill-formed subsequence. Its reader-spec
line and byte column are defined in
[§5.1](05-reader-specs.md#51-reader-specs-and-parsing); input location,
context, and record path are null. This extends the Unicode decision in
[ADR 0011](../../docs/adr/0011-utf8-strings.md) to reader specs.

Gecode accepts arbitrary bytes inside reader-spec strings and comments.
Outside them, it reports `E-SPEC-CHARACTER` for an ill-formed byte. Iris
instead reports the earlier `E-SPEC-UTF8` for ill-formed bytes anywhere.
Valid UTF-8 non-ASCII characters outside strings and comments still fail
lexing with `E-SPEC-CHARACTER`, as in Gecode; valid UTF-8 does not make
Unicode names or a leading BOM legal syntax. Literal CR and NUL inside a
string remain accepted because they are valid Unicode scalar values.

## C.3 Exact budget arithmetic

The [work budgets](10-work-budgets.md#101-counters-and-input-byte-length)
MUST use exact mathematical nonnegative integers for limits, charges,
accumulation, and comparisons. Overflow MUST NOT change acceptance or the
point of exhaustion.

This is a temporary difference from the pinned reference commit
`599f977b7855dba2a1a5f67db1892722372067d3`, which has signed-overflow
undefined behavior in budget arithmetic. In `gecode/instance/eval.cpp`,
lines 360, 613, 676, and 770 compute `1 + leaves()` in `int` before passing
the result to the wider charge function. `Value::leaves()` returns `int`
at `gecode/instance/impl.hpp:302`. A permitted row with 2147483647 leaves
therefore makes this addition overflow on the reference's 32-bit `int`.
The evaluator constructor at `gecode/instance/eval.cpp:160` also computes
`base_effort + 16 * static_cast<long long int>(text.size())` in signed
arithmetic, which can overflow for sufficiently large input. Accumulation
must likewise avoid overflow before comparing a charge with its limit.

These sites were identified from source and an isolated arithmetic probe;
no full reader/input reproduction at those sizes was run. No particular
acceptance result for the undefined reference arithmetic is specified.
Ordinary-sized budget examples in chapter 10 agree with the reference.

The overflow is a Gecode 6.5.0 bug expected to be fixed before release.
This entry is to be removed once the reference pin includes the fix.

## C.4 dzn floats just below 1e17

Float leaves MUST render using the exact algorithm in
[appendix B.3](appendix-b-dzn.md#b3-floating-point-text), including when
99999999999999600 ≤ |f| < 100000000000000000. These are 25 positive
binary64 values spaced by 16, and their negatives. For example, the first
positive value MUST render `99999999999999600.0`.

This is a temporary difference from the pinned reference commit
`599f977b7855dba2a1a5f67db1892722372067d3`, which renders each of those
float leaves as empty text. In `print_real` at
`gecode/instance/data.cpp:427`, the calculation
`floor(log10(abs(f))) + 1` rounds the logarithm to 17 and starts the
precision search at 18. Its loop ends at 17, so no iteration emits text.
Surrounding assignment or array punctuation is still emitted: an affected
scalar entry becomes `edge = ;` followed by LF.

The [Iris-only gap example](appendix-b-dzn.md#b3-floating-point-text) uses
exact decimal arithmetic to check the required output independently of
the reference. The other dzn examples agree with the pinned reference.

The omission is a Gecode 6.5.0 bug expected to be fixed before release.
This entry is to be removed once the reference pin includes the fix.

## C.5 Direct names in size

In a check, a direct ordinary name in `size` MUST resolve like any other
use of the name: a field of a completed repeat denotes one record's value,
exactly as in `count`
([§8.12](08-expressions-and-checks.md#812-record-names-and-field-access)).
`size` and `count` are therefore interchangeable. A check counts the
records of a column with a comprehension, as in `count(r in a:1)`, or with
`size` of a record list named by `as`. This deliberate difference follows
[ADR 0016](../../docs/adr/0016-size-lifts-like-count.md).

The pinned reference gives a direct name in `size` the whole-array rule
that `defined` and comprehension domains have. After `repeat 2 {list
c:int until 0}`, it reads `check size(c)>0` as "there are clauses", so an
empty clause passes; Iris checks each clause and reports
`E-CHECK-FAILED` for the empty one
([`expressions-size-lifts`](08-expressions-and-checks.md#812-record-names-and-field-access)).
The two functions differed only in this position, which made `size` an
easy mistake for a per-record test.
