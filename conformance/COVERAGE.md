# Pinned-detail coverage

This maps each decided rule in [appendix D](../spec/0.1/appendix-d-pinned-details.md)
to case directories. Each boundary, token form, whitespace distinction,
coordinate rule, scope judgment and matrix layout has its own row. Paths
are relative to this directory. Existing cases include the ported Gecode
cases, smoke cases and extractor-owned Specification examples.

Hand-written supplements live in [cases/0.1/pinned/](cases/0.1/pinned/).
That area belongs to [PLAN.md §7](../PLAN.md#7-conformance-suite), milestone
M2. It uses the ordinary [case contract](README.md#case-files-and-discovery).
The extractor continues to own `spec-examples/`; no generated case is edited.

Every new runnable expectation was captured from the pinned reference
`5e7bbd5c26b0702c6fd652356b2289da0710ac3c`, using `gecode-instance`
and the adapter. Successful reads were rendered through both tools and
compared byte for byte before retaining the adapter's canonical JSON.
Error codes and coordinates were checked against the raw reference
diagnostic and the Specification. Reader specs use `spec.iris` only when
valid UTF-8; the existing C.2 negative cases use `spec.bin`.

## Limits and diagnostic recovery

NUL numeric-token cases run as ordinary language errors. The adapter
recovers C-string-truncated messages only after checking the known template
and actual input bytes, as documented in its
[diagnostic rule](adapters/gecode/README.md#nul-in-a-quoted-input-value).
Additional ordinary cases cover [num](cases/0.1/pinned/number-nul/),
[named ranges and custom tokens](cases/0.1/pinned/named-range-nul/),
[expected words](cases/0.1/pinned/expected-word-nul/),
[alphabet symbols](cases/0.1/pinned/alphabet-nul/), and
[trailing words](cases/0.1/pinned/trailing-word-nul/).
No Specification/reference language disagreement was observed.

The exact-arithmetic rows exercise ordinary-sized equality and first-excess
accounting. As [C.3](../spec/0.1/appendix-c-differences.md#c3-exact-budget-arithmetic)
explicitly records, they do not reproduce integer overflow at the enormous
reference limits. No deterministic alternate outcome or overflow coverage
is claimed.

The existing six known-failure annotations remain unchanged: C.1 comment
and tail inputs have complete alternate data, C.2 invalid string and
comment source have complete alternate parse success, C.4 has complete
alternate dzn, and C.1 byte-split word uses the exact `unrepresentable`
serialization exception. See [the contract](README.md#known-failures).

Budget cases use B = 1048576. Most expression probes first spend
1024*(1+1022) = B−1024 units by visiting explicit record lists, then spend
1024−r units on scalar sums. This leaves r units for the event under test.
With N input bytes, another 16*N units fill the added allowance.
Equality and first excess use the same event with allowances differing
by one. Setup reads, definitions and empty structures add no expression
units unless the tested event says otherwise. Idle charges remain far
below B in expression probes.
The atomic sum probe leaves two units for a three-leaf argument whose
second addition would overflow: charging the whole argument must report
the budget error before that arithmetic.

## Counts

All 59 appendix D subitems appear below. There are 417 rule rows:
245 covered by existing cases and 172 by new runnable cases.
The new area contains 161 case directories, including five additional
NUL diagnostic cases beyond the individual appendix D rows.
The two C.3 rows retain the overflow limitation described above.

## D.1 Input lexing

| Appendix subitem | Individual decided rule | Case directories | Coverage |
|---|---|---|---|
| Integer token syntax | Reject fraction integer token | [`cases/0.1/pinned/integer-fraction`](cases/0.1/pinned/integer-fraction/) | New |
| Integer token syntax | Reject separator integer token | [`cases/0.1/pinned/integer-separator`](cases/0.1/pinned/integer-separator/) | New |
| Integer token syntax | Reject bare-plus integer token | [`cases/0.1/pinned/integer-bare-plus`](cases/0.1/pinned/integer-bare-plus/) | New |
| Integer token syntax | Reject bare-minus integer token | [`cases/0.1/pinned/integer-bare-minus`](cases/0.1/pinned/integer-bare-minus/) | New |
| Integer token syntax | Reject double-plus integer token | [`cases/0.1/pinned/integer-double-plus`](cases/0.1/pinned/integer-double-plus/) | New |
| Integer token syntax | Reject double-minus integer token | [`cases/0.1/pinned/integer-double-minus`](cases/0.1/pinned/integer-double-minus/) | New |
| Integer token syntax | Reject mixed-sign integer token | [`cases/0.1/pinned/integer-mixed-sign`](cases/0.1/pinned/integer-mixed-sign/) | New |
| Integer token syntax | Reject NUL integer token | [`cases/0.1/pinned/integer-nul`](cases/0.1/pinned/integer-nul/) | New |
| Integer token syntax | Reject underflow integer token | [`cases/0.1/pinned/integer-underflow`](cases/0.1/pinned/integer-underflow/) | New |
| Integer token syntax | Reject suffix integer token | [`cases/0.1/pinned/integer-suffix`](cases/0.1/pinned/integer-suffix/) | New |
| Float token syntax and exponents | Reject point float token | [`cases/0.1/pinned/float-point`](cases/0.1/pinned/float-point/) | New |
| Float token syntax and exponents | Reject exponent-no-significand float token | [`cases/0.1/pinned/float-exponent-no-significand`](cases/0.1/pinned/float-exponent-no-significand/) | New |
| Float token syntax and exponents | Reject exponent-no-digits float token | [`cases/0.1/pinned/float-exponent-no-digits`](cases/0.1/pinned/float-exponent-no-digits/) | New |
| Float token syntax and exponents | Reject exponent-sign-only float token | [`cases/0.1/pinned/float-exponent-sign-only`](cases/0.1/pinned/float-exponent-sign-only/) | New |
| Float token syntax and exponents | Reject misplaced-sign float token | [`cases/0.1/pinned/float-misplaced-sign`](cases/0.1/pinned/float-misplaced-sign/) | New |
| Float token syntax and exponents | Reject NUL float token | [`cases/0.1/pinned/float-nul`](cases/0.1/pinned/float-nul/) | New |
| Float token syntax and exponents | Reject suffix float token | [`cases/0.1/pinned/float-suffix`](cases/0.1/pinned/float-suffix/) | New |
| Float token syntax and exponents | Reject underflow-to-zero float token | [`cases/0.1/pinned/float-underflow-to-zero`](cases/0.1/pinned/float-underflow-to-zero/) | New |
| Float token syntax and exponents | Reject below-normal float token | [`cases/0.1/pinned/float-below-normal`](cases/0.1/pinned/float-below-normal/) | New |
| Float token syntax and exponents | Reject infinity-upper float token | [`cases/0.1/pinned/float-infinity-upper`](cases/0.1/pinned/float-infinity-upper/) | New |
| Float token syntax and exponents | Reject nan-payload float token | [`cases/0.1/pinned/float-nan-payload`](cases/0.1/pinned/float-nan-payload/) | New |
| Float token syntax and exponents | Round exact halfway decimals to even binary64 neighbors | [`cases/0.1/pinned/float-round-ties`](cases/0.1/pinned/float-round-ties/) | New |
| Default whitespace | SPACE separates ordinary tokens | [`cases/0.1/pinned/separator-space`](cases/0.1/pinned/separator-space/) | New |
| VT/FF versus line blanks | SPACE satisfies required pattern blanks | [`cases/0.1/pinned/pattern-blank-space`](cases/0.1/pinned/pattern-blank-space/) | New |
| Default whitespace | TAB separates ordinary tokens | [`cases/0.1/pinned/separator-tab`](cases/0.1/pinned/separator-tab/) | New |
| VT/FF versus line blanks | TAB satisfies required pattern blanks | [`cases/0.1/pinned/pattern-blank-tab`](cases/0.1/pinned/pattern-blank-tab/) | New |
| Default whitespace | LF separates ordinary tokens | [`cases/0.1/pinned/separator-lf`](cases/0.1/pinned/separator-lf/) | New |
| Default whitespace | CR separates ordinary tokens | [`cases/0.1/pinned/separator-cr`](cases/0.1/pinned/separator-cr/) | New |
| VT/FF versus line blanks | CR satisfies required pattern blanks | [`cases/0.1/pinned/pattern-blank-cr`](cases/0.1/pinned/pattern-blank-cr/) | New |
| Default whitespace | VT separates ordinary tokens | [`cases/0.1/pinned/separator-vt`](cases/0.1/pinned/separator-vt/) | New |
| VT/FF versus line blanks | VT cannot satisfy required pattern blanks | [`cases/0.1/pinned/pattern-blank-vt`](cases/0.1/pinned/pattern-blank-vt/) | New |
| Default whitespace | FF separates ordinary tokens | [`cases/0.1/pinned/separator-ff`](cases/0.1/pinned/separator-ff/) | New |
| VT/FF versus line blanks | FF cannot satisfy required pattern blanks | [`cases/0.1/pinned/pattern-blank-ff`](cases/0.1/pinned/pattern-blank-ff/) | New |
| VT/FF versus line blanks | VT and FF remain inside line-pattern word captures | [`cases/0.1/pinned/pattern-word-controls`](cases/0.1/pinned/pattern-word-controls/) | New |
| VT/FF versus line blanks | Added separator changes initial skipping but not trailing line trimming | [`cases/0.1/pinned/rest-added-separator`](cases/0.1/pinned/rest-added-separator/) | New |
| Leading `+` for integers | Leading + is accepted | [`cases/0.1/spec-examples/04-input--input-integer-spellings`](cases/0.1/spec-examples/04-input--input-integer-spellings/) | Existing |
| Integer token syntax | Leading zeros are decimal | [`cases/0.1/spec-examples/04-input--input-integer-spellings`](cases/0.1/spec-examples/04-input--input-integer-spellings/) | Existing |
| Integer token syntax | Minus sign is accepted | [`cases/0.1/spec-examples/04-input--input-integer-spellings`](cases/0.1/spec-examples/04-input--input-integer-spellings/) | Existing |
| Integer token syntax | Signed zero is zero | [`cases/0.1/spec-examples/04-input--input-integer-spellings`](cases/0.1/spec-examples/04-input--input-integer-spellings/) | Existing |
| Integer token syntax | Signed 64-bit minimum is accepted | [`cases/0.1/spec-examples/04-input--input-integer-spellings`](cases/0.1/spec-examples/04-input--input-integer-spellings/) | Existing |
| Integer token syntax | Signed 64-bit maximum is accepted | [`cases/0.1/spec-examples/04-input--input-integer-spellings`](cases/0.1/spec-examples/04-input--input-integer-spellings/) | Existing |
| Integer token syntax | One above the maximum fails as E-EXPECTED-VALUE | [`cases/0.1/spec-examples/04-input--input-integer-overflow`](cases/0.1/spec-examples/04-input--input-integer-overflow/) | Existing |
| Integer token syntax | Complete conversion rejects exponent tokens | [`cases/0.1/spec-examples/04-input--input-integer-token`](cases/0.1/spec-examples/04-input--input-integer-token/) | Existing |
| Hexadecimal integer rejection | 0x10 is not a decimal integer | [`cases/0.1/spec-examples/04-input--input-integer-hex`](cases/0.1/spec-examples/04-input--input-integer-hex/) | Existing |
| Float token syntax and exponents | Integer-looking decimal is accepted as float | [`cases/0.1/spec-examples/04-input--input-float-spellings`](cases/0.1/spec-examples/04-input--input-float-spellings/) | Existing |
| Float token syntax and exponents | Negative leading-zero decimal is accepted as float | [`cases/0.1/spec-examples/04-input--input-float-spellings`](cases/0.1/spec-examples/04-input--input-float-spellings/) | Existing |
| Float token syntax and exponents | Trailing decimal point is accepted as float | [`cases/0.1/spec-examples/04-input--input-float-spellings`](cases/0.1/spec-examples/04-input--input-float-spellings/) | Existing |
| Float token syntax and exponents | Leading decimal point is accepted as float | [`cases/0.1/spec-examples/04-input--input-float-spellings`](cases/0.1/spec-examples/04-input--input-float-spellings/) | Existing |
| Leading `+` for floats | Plus with leading decimal point is accepted as float | [`cases/0.1/spec-examples/04-input--input-float-spellings`](cases/0.1/spec-examples/04-input--input-float-spellings/) | Existing |
| Float token syntax and exponents | Decimal point followed by exponent is accepted as float | [`cases/0.1/spec-examples/04-input--input-float-spellings`](cases/0.1/spec-examples/04-input--input-float-spellings/) | Existing |
| Float token syntax and exponents | Uppercase exponent with minus sign is accepted as float | [`cases/0.1/spec-examples/04-input--input-float-spellings`](cases/0.1/spec-examples/04-input--input-float-spellings/) | Existing |
| Float token syntax and exponents | Overflow to infinity is rejected | [`cases/0.1/spec-examples/04-input--input-float-overflow`](cases/0.1/spec-examples/04-input--input-float-overflow/) | Existing |
| Float token syntax and exponents | Nonzero rounding to subnormal is rejected | [`cases/0.1/spec-examples/04-input--input-float-underflow`](cases/0.1/spec-examples/04-input--input-float-underflow/) | Existing |
| Float token syntax and exponents | Exact zero with a very small exponent is accepted | [`cases/0.1/spec-examples/04-input--input-float-normal-boundary`](cases/0.1/spec-examples/04-input--input-float-normal-boundary/) | Existing |
| Float token syntax and exponents | Decimal below the normal threshold that rounds to normal is accepted | [`cases/0.1/spec-examples/04-input--input-float-normal-boundary`](cases/0.1/spec-examples/04-input--input-float-normal-boundary/) | Existing |
| `inf` rejection | Lowercase inf is rejected | [`cases/0.1/spec-examples/04-input--input-float-inf`](cases/0.1/spec-examples/04-input--input-float-inf/) | Existing |
| `nan` rejection | Lowercase nan is rejected | [`cases/0.1/spec-examples/04-input--input-float-nan`](cases/0.1/spec-examples/04-input--input-float-nan/) | Existing |
| Hexadecimal float rejection | 0x1p2 is rejected | [`cases/0.1/spec-examples/04-input--input-float-hex`](cases/0.1/spec-examples/04-input--input-float-hex/) | Existing |
| VT/FF versus line blanks | rest trims trailing space, tab and CR | [`cases/0.1/spec-examples/04-input--input-line-blanks`](cases/0.1/spec-examples/04-input--input-line-blanks/) | Existing |
| VT/FF versus line blanks | rest retains interior CR | [`cases/0.1/spec-examples/04-input--input-line-blanks`](cases/0.1/spec-examples/04-input--input-line-blanks/) | Existing |
| VT/FF versus line blanks | rest retains VT and FF | [`cases/0.1/spec-examples/04-input--input-line-blanks`](cases/0.1/spec-examples/04-input--input-line-blanks/) | Existing |
| Default whitespace | Non-ASCII no-break space remains inside a word | [`cases/0.1/spec-examples/04-input--input-unicode-space`](cases/0.1/spec-examples/04-input--input-unicode-space/) | Existing |
| CR LF and lone CR | CR LF separates tokens without normalization | [`cases/0.1/spec-examples/04-input--input-line-endings`](cases/0.1/spec-examples/04-input--input-line-endings/) | Existing |
| CR LF and lone CR | Lone CR separates tokens without starting a line | [`cases/0.1/spec-examples/04-input--input-line-endings`](cases/0.1/spec-examples/04-input--input-line-endings/) | Existing |
| Final newline | Ordinary token read accepts absent final LF | [`cases/0.1/spec-examples/04-input--input-line-endings`](cases/0.1/spec-examples/04-input--input-line-endings/) | Existing |
| Final newline | Pattern match accepts absent final LF | [`cases/0.1/spec-examples/07-structure--structure-line-final-lf`](cases/0.1/spec-examples/07-structure--structure-line-final-lf/) | Existing |
| Final newline | Pattern match accepts final LF | [`cases/0.1/spec-examples/07-structure--structure-line-current-and-next`](cases/0.1/spec-examples/07-structure--structure-line-current-and-next/) | Existing |

## D.2 Positions and UTF-8

| Appendix subitem | Individual decided rule | Case directories | Coverage |
|---|---|---|---|
| Column unit | VT counts encoding bytes in columns | [`cases/0.1/pinned/column-vt`](cases/0.1/pinned/column-vt/) | New |
| Column unit | FF counts encoding bytes in columns | [`cases/0.1/pinned/column-ff`](cases/0.1/pinned/column-ff/) | New |
| Column unit | COMBINING counts encoding bytes in columns | [`cases/0.1/pinned/column-combining`](cases/0.1/pinned/column-combining/) | New |
| Column unit | EMOJI counts encoding bytes in columns | [`cases/0.1/pinned/column-emoji`](cases/0.1/pinned/column-emoji/) | New |
| Column unit | EOF without LF is one byte column past the final byte | [`cases/0.1/pinned/eof-without-lf`](cases/0.1/pinned/eof-without-lf/) | New |
| Column unit | Stop moves the cursor to physical EOF, including unread tail | [`cases/0.1/pinned/stop-eof-position`](cases/0.1/pinned/stop-eof-position/) | New |
| Input-wide UTF-8 | Unknown parameter admission precedes invalid-input validation and execution | [`cases/0.1/pinned/utf8-param-order`](cases/0.1/pinned/utf8-param-order/) | New |
| Column unit | Lines and columns start at 1; UTF-8 bytes count separately | [`cases/0.1/spec-examples/04-input--input-byte-columns`](cases/0.1/spec-examples/04-input--input-byte-columns/) | Existing |
| Tabs | A tab advances exactly one byte column | [`cases/0.1/spec-examples/04-input--input-byte-columns`](cases/0.1/spec-examples/04-input--input-byte-columns/) | Existing |
| Column unit | Lone CR advances one column without changing the line | [`cases/0.1/spec-examples/04-input--input-cr-columns`](cases/0.1/spec-examples/04-input--input-cr-columns/) | Existing |
| Column unit | Only LF in CR LF increments the line and resets the column | [`cases/0.1/spec-examples/04-input--input-crlf-columns`](cases/0.1/spec-examples/04-input--input-crlf-columns/) | Existing |
| Column unit | EOF after LF is next line, column 1 | [`cases/0.1/spec-examples/04-input--input-eof-columns`](cases/0.1/spec-examples/04-input--input-eof-columns/) | Existing |
| Input-wide UTF-8 | Invalid skipped comment bytes fail before statements execute (C.1) | [`cases/0.1/spec-examples/04-input--input-invalid-utf8-comment`](cases/0.1/spec-examples/04-input--input-invalid-utf8-comment/) | Existing |
| Input-wide UTF-8 | Invalid ignored-tail bytes are still validated (C.1) | [`cases/0.1/spec-examples/04-input--input-invalid-utf8-tail`](cases/0.1/spec-examples/04-input--input-invalid-utf8-tail/) | Existing |
| Read-value UTF-8 | Byte token settings split valid UTF-8; the actually read word is rejected (C.1) | [`cases/0.1/spec-examples/04-input--input-invalid-utf8-split`](cases/0.1/spec-examples/04-input--input-invalid-utf8-split/) | Existing |
| Reader-spec UTF-8 | Validate string bytes before parsing; first ill-formed byte position (C.2) | [`cases/0.1/spec-examples/05-reader-specs--reader-invalid-utf8-string`](cases/0.1/spec-examples/05-reader-specs--reader-invalid-utf8-string/) | Existing |
| Reader-spec UTF-8 | Validate comment bytes with byte-counted tab and UTF-8 columns (C.2) | [`cases/0.1/spec-examples/05-reader-specs--reader-invalid-utf8-comment`](cases/0.1/spec-examples/05-reader-specs--reader-invalid-utf8-comment/) | Existing |

## D.3 Comment and stop prefixes

| Appendix subitem | Individual decided rule | Case directories | Coverage |
|---|---|---|---|
| Prefix boundary class | FF indentation prevents prefix recognition | [`cases/0.1/pinned/prefix-indent-ff`](cases/0.1/pinned/prefix-indent-ff/) | New |
| Prefix boundary class | COMMA indentation prevents prefix recognition | [`cases/0.1/pinned/prefix-indent-comma`](cases/0.1/pinned/prefix-indent-comma/) | New |
| Prefix boundary class | digit prefix edge | [`cases/0.1/pinned/prefix-digit`](cases/0.1/pinned/prefix-digit/) | New |
| Prefix boundary class | punctuation prefix edge | [`cases/0.1/pinned/prefix-punctuation`](cases/0.1/pinned/prefix-punctuation/) | New |
| Prefix boundary class | case prefix edge | [`cases/0.1/pinned/prefix-case`](cases/0.1/pinned/prefix-case/) | New |
| Prefix boundary class | inline prefix edge | [`cases/0.1/pinned/prefix-inline`](cases/0.1/pinned/prefix-inline/) | New |
| Prefix boundary class | lone-cr prefix edge | [`cases/0.1/pinned/prefix-lone-cr`](cases/0.1/pinned/prefix-lone-cr/) | New |
| Prefix boundary class | empty prefix edge | [`cases/0.1/pinned/prefix-empty`](cases/0.1/pinned/prefix-empty/) | New |
| Stop boundary rule | stop-digit prefix edge | [`cases/0.1/pinned/prefix-stop-digit`](cases/0.1/pinned/prefix-stop-digit/) | New |
| Stop boundary rule | stop-underscore prefix edge | [`cases/0.1/pinned/prefix-stop-underscore`](cases/0.1/pinned/prefix-stop-underscore/) | New |
| Stop boundary rule | stop-nonascii prefix edge | [`cases/0.1/pinned/prefix-stop-nonascii`](cases/0.1/pinned/prefix-stop-nonascii/) | New |
| Prefix boundary class | Final ASCII letter requires boundary before another letter | [`cases/0.1/spec-examples/04-input--input-comment-boundaries`](cases/0.1/spec-examples/04-input--input-comment-boundaries/) | Existing |
| Prefix boundary class | Final ASCII letter requires boundary before a digit | [`cases/0.1/spec-examples/04-input--input-comment-boundaries`](cases/0.1/spec-examples/04-input--input-comment-boundaries/) | Existing |
| Prefix boundary class | Final ASCII letter requires boundary before underscore | [`cases/0.1/spec-examples/04-input--input-comment-boundaries`](cases/0.1/spec-examples/04-input--input-comment-boundaries/) | Existing |
| Prefix boundary class | Final underscore requires a boundary | [`cases/0.1/spec-examples/04-input--input-prefix-underscore`](cases/0.1/spec-examples/04-input--input-prefix-underscore/) | Existing |
| Prefix boundary class | Space/tab/CR indentation permits recognition | [`cases/0.1/spec-examples/04-input--input-prefix-indentation`](cases/0.1/spec-examples/04-input--input-prefix-indentation/) | Existing |
| Prefix boundary class | VT indentation prevents recognition | [`cases/0.1/spec-examples/04-input--input-prefix-indentation`](cases/0.1/spec-examples/04-input--input-prefix-indentation/) | Existing |
| Prefix boundary class | Comment wins when comment and stop both match | [`cases/0.1/spec-examples/04-input--input-comment-before-stop`](cases/0.1/spec-examples/04-input--input-comment-before-stop/) | Existing |
| Non-ASCII prefix/following bytes | Non-ASCII following byte permits the ASCII prefix boundary | [`cases/0.1/spec-examples/04-input--input-comment-boundaries`](cases/0.1/spec-examples/04-input--input-comment-boundaries/) | Existing |
| Non-ASCII prefix/following bytes | Non-ASCII final prefix byte imposes no boundary restriction | [`cases/0.1/spec-examples/04-input--input-prefix-nonascii`](cases/0.1/spec-examples/04-input--input-prefix-nonascii/) | Existing |
| Stop boundary rule | Stop rejects following ASCII letter or underscore and accepts following UTF-8 byte | [`cases/0.1/spec-examples/04-input--input-stop-boundaries`](cases/0.1/spec-examples/04-input--input-stop-boundaries/) | Existing |

## D.4 Work budgets

| Appendix subitem | Individual decided rule | Case directories | Coverage |
|---|---|---|---|
| Expression formula and charges | record-comprehension: charge 1; equality succeeds | [`cases/0.1/pinned/effort-record-comprehension-equal`](cases/0.1/pinned/effort-record-comprehension-equal/) | New |
| Expression formula and charges | record-comprehension: charge 1; first excess fails | [`cases/0.1/pinned/effort-record-comprehension-over`](cases/0.1/pinned/effort-record-comprehension-over/) | New |
| Expression formula and charges | record-field: charge 3; equality succeeds | [`cases/0.1/pinned/effort-record-field-equal`](cases/0.1/pinned/effort-record-field-equal/) | New |
| Expression formula and charges | record-field: charge 3; first excess fails | [`cases/0.1/pinned/effort-record-field-over`](cases/0.1/pinned/effort-record-field-over/) | New |
| Expression formula and charges | field-empty: charge 2; equality succeeds | [`cases/0.1/pinned/effort-field-empty-equal`](cases/0.1/pinned/effort-field-empty-equal/) | New |
| Expression formula and charges | field-empty: charge 2; first excess fails | [`cases/0.1/pinned/effort-field-empty-over`](cases/0.1/pinned/effort-field-empty-over/) | New |
| Expression formula and charges | array-scalar: charge 2; equality succeeds | [`cases/0.1/pinned/effort-array-scalar-equal`](cases/0.1/pinned/effort-array-scalar-equal/) | New |
| Expression formula and charges | array-scalar: charge 2; first excess fails | [`cases/0.1/pinned/effort-array-scalar-over`](cases/0.1/pinned/effort-array-scalar-over/) | New |
| Expression formula and charges | array-row: charge 3; equality succeeds | [`cases/0.1/pinned/effort-array-row-equal`](cases/0.1/pinned/effort-array-row-equal/) | New |
| Expression formula and charges | array-row: charge 3; first excess fails | [`cases/0.1/pinned/effort-array-row-over`](cases/0.1/pinned/effort-array-row-over/) | New |
| Expression formula and charges | array-empty: charge 1; equality succeeds | [`cases/0.1/pinned/effort-array-empty-equal`](cases/0.1/pinned/effort-array-empty-equal/) | New |
| Expression formula and charges | array-empty: charge 1; first excess fails | [`cases/0.1/pinned/effort-array-empty-over`](cases/0.1/pinned/effort-array-empty-over/) | New |
| Expression formula and charges | array-comprehension: charge 1; equality succeeds | [`cases/0.1/pinned/effort-array-comprehension-equal`](cases/0.1/pinned/effort-array-comprehension-equal/) | New |
| Expression formula and charges | array-comprehension: charge 1; first excess fails | [`cases/0.1/pinned/effort-array-comprehension-over`](cases/0.1/pinned/effort-array-comprehension-over/) | New |
| Expression formula and charges | array-row-comprehension: charge 3; equality succeeds | [`cases/0.1/pinned/effort-array-row-comprehension-equal`](cases/0.1/pinned/effort-array-row-comprehension-equal/) | New |
| Expression formula and charges | array-row-comprehension: charge 3; first excess fails | [`cases/0.1/pinned/effort-array-row-comprehension-over`](cases/0.1/pinned/effort-array-row-comprehension-over/) | New |
| Expression formula and charges | sum: charge 2; equality succeeds | [`cases/0.1/pinned/effort-sum-equal`](cases/0.1/pinned/effort-sum-equal/) | New |
| Expression formula and charges | sum: charge 2; first excess fails | [`cases/0.1/pinned/effort-sum-over`](cases/0.1/pinned/effort-sum-over/) | New |
| Expression formula and charges | min: charge 2; equality succeeds | [`cases/0.1/pinned/effort-min-equal`](cases/0.1/pinned/effort-min-equal/) | New |
| Expression formula and charges | min: charge 2; first excess fails | [`cases/0.1/pinned/effort-min-over`](cases/0.1/pinned/effort-min-over/) | New |
| Expression formula and charges | max: charge 2; equality succeeds | [`cases/0.1/pinned/effort-max-equal`](cases/0.1/pinned/effort-max-equal/) | New |
| Expression formula and charges | max: charge 2; first excess fails | [`cases/0.1/pinned/effort-max-over`](cases/0.1/pinned/effort-max-over/) | New |
| Expression formula and charges | distinct: charge 2; equality succeeds | [`cases/0.1/pinned/effort-distinct-equal`](cases/0.1/pinned/effort-distinct-equal/) | New |
| Expression formula and charges | distinct: charge 2; first excess fails | [`cases/0.1/pinned/effort-distinct-over`](cases/0.1/pinned/effort-distinct-over/) | New |
| Expression formula and charges | lift-deduplicate: charge 1; equality succeeds | [`cases/0.1/pinned/effort-lift-deduplicate-equal`](cases/0.1/pinned/effort-lift-deduplicate-equal/) | New |
| Expression formula and charges | lift-deduplicate: charge 1; first excess fails | [`cases/0.1/pinned/effort-lift-deduplicate-over`](cases/0.1/pinned/effort-lift-deduplicate-over/) | New |
| Expression formula and charges | lift-fields: charge 2; equality succeeds | [`cases/0.1/pinned/effort-lift-fields-equal`](cases/0.1/pinned/effort-lift-fields-equal/) | New |
| Expression formula and charges | lift-fields: charge 2; first excess fails | [`cases/0.1/pinned/effort-lift-fields-over`](cases/0.1/pinned/effort-lift-fields-over/) | New |
| Expression formula and charges | lift-each: charge 2; equality succeeds | [`cases/0.1/pinned/effort-lift-each-equal`](cases/0.1/pinned/effort-lift-each-equal/) | New |
| Expression formula and charges | lift-each: charge 2; first excess fails | [`cases/0.1/pinned/effort-lift-each-over`](cases/0.1/pinned/effort-lift-each-over/) | New |
| Expression formula and charges | lift-nested: charge 2; equality succeeds | [`cases/0.1/pinned/effort-lift-nested-equal`](cases/0.1/pinned/effort-lift-nested-equal/) | New |
| Expression formula and charges | lift-nested: charge 2; first excess fails | [`cases/0.1/pinned/effort-lift-nested-over`](cases/0.1/pinned/effort-lift-nested-over/) | New |
| Expression formula and charges | default-override: charge 1; equality succeeds | [`cases/0.1/pinned/effort-default-override-equal`](cases/0.1/pinned/effort-default-override-equal/) | New |
| Expression formula and charges | default-override: charge 1; first excess fails | [`cases/0.1/pinned/effort-default-override-over`](cases/0.1/pinned/effort-default-override-over/) | New |
| Empty-row charges | empty-prefix: total charge 1048576; equality succeeds | [`cases/0.1/pinned/idle-empty-prefix-equal`](cases/0.1/pinned/idle-empty-prefix-equal/) | New |
| Empty-row charges | empty-prefix: total charge 1048576; first excess fails | [`cases/0.1/pinned/idle-empty-prefix-over`](cases/0.1/pinned/idle-empty-prefix-over/) | New |
| Empty-row charges | alphabet: total charge 1048576; equality succeeds | [`cases/0.1/pinned/idle-alphabet-equal`](cases/0.1/pinned/idle-alphabet-equal/) | New |
| Empty-row charges | alphabet: total charge 1048576; first excess fails | [`cases/0.1/pinned/idle-alphabet-over`](cases/0.1/pinned/idle-alphabet-over/) | New |
| Empty-row charges | full-matrix: total charge 1048576; equality succeeds | [`cases/0.1/pinned/idle-full-matrix-equal`](cases/0.1/pinned/idle-full-matrix-equal/) | New |
| Empty-row charges | full-matrix: total charge 1048576; first excess fails | [`cases/0.1/pinned/idle-full-matrix-over`](cases/0.1/pinned/idle-full-matrix-over/) | New |
| Idle limit and iterations | nested-idle: total charge 1048576; equality succeeds | [`cases/0.1/pinned/idle-nested-idle-equal`](cases/0.1/pinned/idle-nested-idle-equal/) | New |
| Idle limit and iterations | nested-idle: total charge 1048576; first excess fails | [`cases/0.1/pinned/idle-nested-idle-over`](cases/0.1/pinned/idle-nested-idle-over/) | New |
| Empty-row charges | Zero first dimension and zero iterations add no charge | [`cases/0.1/pinned/idle-zero-prefix`](cases/0.1/pinned/idle-zero-prefix/) | New |
| Input byte length | crlf: original bytes set N; equality succeeds | [`cases/0.1/pinned/effort-bytes-crlf-equal`](cases/0.1/pinned/effort-bytes-crlf-equal/) | New |
| Input byte length | crlf: original bytes set N; first excess fails | [`cases/0.1/pinned/effort-bytes-crlf-over`](cases/0.1/pinned/effort-bytes-crlf-over/) | New |
| Input byte length | comment: original bytes set N; equality succeeds | [`cases/0.1/pinned/effort-bytes-comment-equal`](cases/0.1/pinned/effort-bytes-comment-equal/) | New |
| Input byte length | comment: original bytes set N; first excess fails | [`cases/0.1/pinned/effort-bytes-comment-over`](cases/0.1/pinned/effort-bytes-comment-over/) | New |
| Input byte length | stop: original bytes set N; equality succeeds | [`cases/0.1/pinned/effort-bytes-stop-equal`](cases/0.1/pinned/effort-bytes-stop-equal/) | New |
| Input byte length | stop: original bytes set N; first excess fails | [`cases/0.1/pinned/effort-bytes-stop-over`](cases/0.1/pinned/effort-bytes-stop-over/) | New |
| Input byte length | multibyte: original bytes set N; equality succeeds | [`cases/0.1/pinned/effort-bytes-multibyte-equal`](cases/0.1/pinned/effort-bytes-multibyte-equal/) | New |
| Input byte length | multibyte: original bytes set N; first excess fails | [`cases/0.1/pinned/effort-bytes-multibyte-over`](cases/0.1/pinned/effort-bytes-multibyte-over/) | New |
| Counter lifetime | read: idle counter spans prefix and instance body | [`cases/0.1/pinned/idle-instances-read`](cases/0.1/pinned/idle-instances-read/) | New |
| Counter lifetime | read: prefix and instance share expression allowance | [`cases/0.1/pinned/effort-instances-read`](cases/0.1/pinned/effort-instances-read/) | New |
| Counter lifetime | catalog: idle counter spans prefix and instance body | [`cases/0.1/pinned/idle-instances-catalog`](cases/0.1/pinned/idle-instances-catalog/) | New |
| Counter lifetime | catalog: prefix and instance share expression allowance | [`cases/0.1/pinned/effort-instances-catalog`](cases/0.1/pinned/effort-instances-catalog/) | New |
| Expression formula and charges | argument-atomic: failure order at the effort limit | [`cases/0.1/pinned/effort-order-argument-atomic`](cases/0.1/pinned/effort-order-argument-atomic/) | New |
| Expression formula and charges | type-before-charge: failure order at the effort limit | [`cases/0.1/pinned/effort-order-type-before-charge`](cases/0.1/pinned/effort-order-type-before-charge/) | New |
| Expression formula and charges | bounds-before-charge: failure order at the effort limit | [`cases/0.1/pinned/effort-order-bounds-before-charge`](cases/0.1/pinned/effort-order-bounds-before-charge/) | New |
| Expression formula and charges | scalar-before-charge: failure order at the effort limit | [`cases/0.1/pinned/effort-order-scalar-before-charge`](cases/0.1/pinned/effort-order-scalar-before-charge/) | New |
| Expression formula and charges | distinct-eager: failure order at the effort limit | [`cases/0.1/pinned/effort-order-distinct-eager`](cases/0.1/pinned/effort-order-distinct-eager/) | New |
| Expression formula and charges | Unlisted operators, empty aggregates/domains and unexecuted operands add zero units | [`cases/0.1/pinned/effort-zero-cost`](cases/0.1/pinned/effort-zero-cost/) | New |
| Counter lifetime | read: effort exhaustion in an instance after prefix work | [`cases/0.1/pinned/effort-instances-over-read`](cases/0.1/pinned/effort-instances-over-read/) | New |
| Counter lifetime | read: effort accumulates across discarded instance frames | [`cases/0.1/pinned/effort-instance-no-reset-read`](cases/0.1/pinned/effort-instance-no-reset-read/) | New |
| Counter lifetime | read: idle work accumulates across discarded instance frames | [`cases/0.1/pinned/idle-instance-no-reset-read`](cases/0.1/pinned/idle-instance-no-reset-read/) | New |
| Counter lifetime | catalog: effort exhaustion in an instance after prefix work | [`cases/0.1/pinned/effort-instances-over-catalog`](cases/0.1/pinned/effort-instances-over-catalog/) | New |
| Counter lifetime | catalog: effort accumulates across discarded instance frames | [`cases/0.1/pinned/effort-instance-no-reset-catalog`](cases/0.1/pinned/effort-instance-no-reset-catalog/) | New |
| Counter lifetime | catalog: idle work accumulates across discarded instance frames | [`cases/0.1/pinned/idle-instance-no-reset-catalog`](cases/0.1/pinned/idle-instance-no-reset-catalog/) | New |
| Counter lifetime | Deferred rejections do not refund or suppress idle charges; budget error stays primary | [`cases/0.1/pinned/budget-deferred-hard`](cases/0.1/pinned/budget-deferred-hard/) | New |
| Counter lifetime | Parse-only never executes budgets | [`cases/0.1/pinned/effort-parse-only`](cases/0.1/pinned/effort-parse-only/) | New |
| Expression formula and charges | Explicit record-list index costs 1; nested structural-list field selection adds zero (equal) | [`cases/0.1/pinned/effort-record-index-equal`](cases/0.1/pinned/effort-record-index-equal/) | New |
| Expression formula and charges | Explicit record-list index costs 1; nested structural-list field selection adds zero (over) | [`cases/0.1/pinned/effort-record-index-over`](cases/0.1/pinned/effort-record-index-over/) | New |
| Expression formula and charges | Nested each marker charges descendant leaf at both levels (4 units); equality succeeds | [`cases/0.1/pinned/effort-lift-nested-each-equal`](cases/0.1/pinned/effort-lift-nested-each-equal/) | New |
| Expression formula and charges | Nested each marker charges descendant leaf at both levels (4 units); first excess fails | [`cases/0.1/pinned/effort-lift-nested-each-over`](cases/0.1/pinned/effort-lift-nested-each-over/) | New |
| Input byte length | Selected instance still uses all original tail bytes in N (equal) | [`cases/0.1/pinned/effort-selected-tail-equal`](cases/0.1/pinned/effort-selected-tail-equal/) | New |
| Input byte length | Selected instance still uses all original tail bytes in N (over) | [`cases/0.1/pinned/effort-selected-tail-over`](cases/0.1/pinned/effort-selected-tail-over/) | New |
| Idle limit and iterations | B-1 idle iterations succeed | [`cases/0.1/spec-examples/10-work-budgets--budgets-idle-under`](cases/0.1/spec-examples/10-work-budgets--budgets-idle-under/) | Existing |
| Idle limit and iterations | B idle iterations succeed | [`cases/0.1/spec-examples/10-work-budgets--budgets-idle-equal`](cases/0.1/spec-examples/10-work-budgets--budgets-idle-equal/) | Existing |
| Idle limit and iterations | B+1 fails at the first excessive iteration | [`cases/0.1/spec-examples/10-work-budgets--budgets-idle-over`](cases/0.1/spec-examples/10-work-budgets--budgets-idle-over/) | Existing |
| Expression formula and charges | B-1 expression units succeed with N=0 | [`cases/0.1/spec-examples/10-work-budgets--budgets-effort-under`](cases/0.1/spec-examples/10-work-budgets--budgets-effort-under/) | Existing |
| Expression formula and charges | B expression units succeed with N=0 | [`cases/0.1/spec-examples/10-work-budgets--budgets-effort-equal`](cases/0.1/spec-examples/10-work-budgets--budgets-effort-equal/) | Existing |
| Expression formula and charges | B+1 expression units fail with N=0 | [`cases/0.1/spec-examples/10-work-budgets--budgets-effort-over`](cases/0.1/spec-examples/10-work-budgets--budgets-effort-over/) | Existing |
| Counter lifetime | Reading input does not reset idle work | [`cases/0.1/spec-examples/10-work-budgets--budgets-idle-no-reset`](cases/0.1/spec-examples/10-work-budgets--budgets-idle-no-reset/) | Existing |
| Counter lifetime | Post-body idle error retains the final body line and active iteration | [`cases/0.1/spec-examples/10-work-budgets--budgets-idle-body-location`](cases/0.1/spec-examples/10-work-budgets--budgets-idle-body-location/) | Existing |
| Exact budget arithmetic | Ordinary-sized exact equality; C.3 overflow remains unexercised | [`cases/0.1/spec-examples/10-work-budgets--budgets-effort-equal`](cases/0.1/spec-examples/10-work-budgets--budgets-effort-equal/) | Existing |
| Exact budget arithmetic | Ordinary-sized first excess; C.3 overflow remains unexercised | [`cases/0.1/spec-examples/10-work-budgets--budgets-effort-over`](cases/0.1/spec-examples/10-work-budgets--budgets-effort-over/) | Existing |
| Idle limit and iterations | Indefinite nonprogressing repeat reports E-NO-PROGRESS rather than an idle charge | [`cases/0.1/structure/progress-1`](cases/0.1/structure/progress-1/) | Existing |
| Idle limit and iterations | Nonprogressing instance reports E-NO-PROGRESS | [`cases/0.1/spec-examples/09-params-and-instances--instances-no-progress`](cases/0.1/spec-examples/09-params-and-instances--instances-no-progress/) | Existing |

## D.5 Integer semantics

| Appendix subitem | Individual decided rule | Case directories | Coverage |
|---|---|---|---|
| Power results and overflow | Negative base one exponent beyond (-2)^63 fails | [`cases/0.1/pinned/power-negative-overflow`](cases/0.1/pinned/power-negative-overflow/) | New |
| Power results and overflow | zero-negative: negative exponent still fails | [`cases/0.1/pinned/power-zero-negative`](cases/0.1/pinned/power-zero-negative/) | New |
| Power results and overflow | one-negative: negative exponent still fails | [`cases/0.1/pinned/power-one-negative`](cases/0.1/pinned/power-one-negative/) | New |
| 64-bit checked arithmetic | Exact successful endpoint additions, subtractions and products; product signs and zero | [`cases/0.1/pinned/integer-boundary-results`](cases/0.1/pinned/integer-boundary-results/) | New |
| 64-bit checked arithmetic | Addition above maximum fails | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-add-overflow`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-add-overflow/) | Existing |
| 64-bit checked arithmetic | Addition below minimum fails | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-addition-underflow`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-addition-underflow/) | Existing |
| 64-bit checked arithmetic | Subtraction above maximum fails | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-subtract-overflow`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-subtract-overflow/) | Existing |
| 64-bit checked arithmetic | Subtraction below minimum fails | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-subtract-underflow`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-subtract-underflow/) | Existing |
| 64-bit checked arithmetic | Multiplication above maximum fails | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-multiply-overflow`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-multiply-overflow/) | Existing |
| 64-bit checked arithmetic | Multiplication below minimum fails | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-multiply-underflow`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-multiply-underflow/) | Existing |
| 64-bit checked arithmetic | Negating minimum fails | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-negate-minimum`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-negate-minimum/) | Existing |
| 64-bit checked arithmetic | Intermediate aggregate sum overflow fails before later cancellation | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-sum-intermediate-overflow`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-sum-intermediate-overflow/) | Existing |
| Truncating division | Negative dividend truncates toward zero | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-integer-results`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-integer-results/) | Existing |
| Truncating division | Negative divisor truncates toward zero | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-integer-results`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-integer-results/) | Existing |
| Truncating division | Both negative quotient operands truncate toward zero | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-integer-results`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-integer-results/) | Existing |
| Truncating division | Minimum divided by -1 overflows | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-minimum-div-minus-one`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-minimum-div-minus-one/) | Existing |
| Truncating division | Division by zero fails | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-div-zero`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-div-zero/) | Existing |
| Negative remainder operands | Negative dividend gives negative remainder | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-integer-results`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-integer-results/) | Existing |
| Negative remainder operands | Negative divisor retains positive dividend remainder sign | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-integer-results`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-integer-results/) | Existing |
| Negative remainder operands | Both negative retains negative dividend remainder sign | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-integer-results`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-integer-results/) | Existing |
| Negative remainder operands | Minimum modulo -1 overflows | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-minimum-mod-minus-one`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-minimum-mod-minus-one/) | Existing |
| Negative remainder operands | Modulo zero fails | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-mod-zero`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-mod-zero/) | Existing |
| Power results and overflow | Zero exponent including 0^0 gives 1 | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-power-results`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-power-results/) | Existing |
| Power results and overflow | (-2)^63 reaches the signed minimum exactly | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-power-results`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-power-results/) | Existing |
| Power results and overflow | Minimum base to exponent 1 is exact | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-power-results`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-power-results/) | Existing |
| Power results and overflow | Minimum base to exponent 0 is 1 | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-power-results`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-power-results/) | Existing |
| Power results and overflow | Bases 0, 1 and -1 accept maximum exponent | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-power-results`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-power-results/) | Existing |
| Power results and overflow | 2^63 overflows | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-power-overflow`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-power-overflow/) | Existing |
| Power results and overflow | Negative exponent fails | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-negative-exponent`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-negative-exponent/) | Existing |
| Power results and overflow | Power before unary negation can overflow | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-power-precedence-overflow`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-power-precedence-overflow/) | Existing |
| Power results and overflow | Power is right associative; in-range nontrivial powers remain exact | [`cases/0.1/expressions/power-1`](cases/0.1/expressions/power-1/) | Existing |

## D.6 Error order

| Appendix subitem | Individual decided rule | Case directories | Coverage |
|---|---|---|---|
| Phase order | types-before-records | [`cases/0.1/pinned/order-types-before-records`](cases/0.1/pinned/order-types-before-records/) | New |
| Runtime statement and operand order | offset-before-range | [`cases/0.1/pinned/order-offset-before-range`](cases/0.1/pinned/order-offset-before-range/) | New |
| Runtime statement and operand order | range-before-dimension | [`cases/0.1/pinned/order-range-before-dimension`](cases/0.1/pinned/order-range-before-dimension/) | New |
| Runtime statement and operand order | dimension-before-layout | [`cases/0.1/pinned/order-dimension-before-layout`](cases/0.1/pinned/order-dimension-before-layout/) | New |
| Runtime statement and operand order | left-before-right | [`cases/0.1/pinned/order-left-before-right`](cases/0.1/pinned/order-left-before-right/) | New |
| Sections ties | sections-missing-order | [`cases/0.1/pinned/order-sections-missing-order`](cases/0.1/pinned/order-sections-missing-order/) | New |
| Sections ties | First missing required arm in declaration order determines exact catalog summary | [`cases/0.1/pinned/catalog-missing-order-a`](cases/0.1/pinned/catalog-missing-order-a/), [`cases/0.1/pinned/catalog-missing-order-b`](cases/0.1/pinned/catalog-missing-order-b/) | New |
| Phase order | Whole-source lexing precedes earlier statement syntax failure | [`cases/0.1/spec-examples/errors--errors-lex-before-syntax`](cases/0.1/spec-examples/errors--errors-lex-before-syntax/) | Existing |
| Phase order | Global binding/value conflict precedes named-type/value conflict | [`cases/0.1/spec-examples/errors--errors-binding-before-type`](cases/0.1/spec-examples/errors--errors-binding-before-type/) | Existing |
| Structural prechecks and component order | Global structural conflict precedes field resolution | [`cases/0.1/spec-examples/errors--errors-record-precheck-before-field`](cases/0.1/spec-examples/errors--errors-record-precheck-before-field/) | Existing |
| Structural prechecks and component order | Resolve type bounds before dimensions | [`cases/0.1/spec-examples/errors--errors-resolver-type-before-dimensions`](cases/0.1/spec-examples/errors--errors-resolver-type-before-dimensions/) | Existing |
| Runtime statement and operand order | Evaluate right operand before rejecting left arithmetic type | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-operand-before-type`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-operand-before-type/) | Existing |
| Runtime statement and operand order | Runtime array base validation precedes index evaluation | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-index-base-first`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-index-base-first/) | Existing |
| Runtime statement and operand order | List offset evaluates after consuming values and sentinel | [`cases/0.1/spec-examples/06-types-and-values--values-offset-list-timing`](cases/0.1/spec-examples/06-types-and-values--values-offset-list-timing/) | Existing |
| Runtime statement and operand order | Declaration offset evaluates before reads | [`cases/0.1/spec-examples/06-types-and-values--values-offset-type`](cases/0.1/spec-examples/06-types-and-values--values-offset-type/) | Existing |
| Runtime statement and operand order | Short-circuit boolean operand skips runtime errors | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-boolean-results`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-boolean-results/) | Existing |
| Sections ties | First completely matching arm wins in declaration order | [`cases/0.1/spec-examples/07-structure--structure-sections-first-arm`](cases/0.1/spec-examples/07-structure--structure-sections-first-arm/) | Existing |
| Sections ties | Missing required arm uses its saved termination position | [`cases/0.1/spec-examples/07-structure--structure-sections-missing`](cases/0.1/spec-examples/07-structure--structure-sections-missing/) | Existing |
| Deferred failures | A later immediately selected rejection takes priority over deferred rejection | [`cases/0.1/spec-examples/09-params-and-instances--instances-name-becomes-selected`](cases/0.1/spec-examples/09-params-and-instances--instances-name-becomes-selected/) | Existing |
| Deferred failures | A later hard read error takes priority over deferred rejection | [`cases/0.1/spec-examples/09-params-and-instances--instances-skipped-hard-error`](cases/0.1/spec-examples/09-params-and-instances--instances-skipped-hard-error/) | Existing |
| Deferred failures | Ordinary line mismatch remains hard in skipped instances | [`cases/0.1/spec-examples/09-params-and-instances--instances-line-error-is-hard`](cases/0.1/spec-examples/09-params-and-instances--instances-line-error-is-hard/) | Existing |
| Deferred failures | Check-evaluation error remains hard in skipped instances | [`cases/0.1/spec-examples/09-params-and-instances--instances-skipped-check-expression`](cases/0.1/spec-examples/09-params-and-instances--instances-skipped-check-expression/) | Existing |
| Deferred failures | Matching completion rethrows deferred witness at original position | [`cases/0.1/spec-examples/09-params-and-instances--instances-selected-record-failure`](cases/0.1/spec-examples/09-params-and-instances--instances-selected-record-failure/) | Existing |
| Phase order | Unknown caller key is rejected before executing required reads | [`cases/0.1/smoke/unknown-parameter`](cases/0.1/smoke/unknown-parameter/) | Existing |

## D.7 Name scopes

| Appendix subitem | Individual decided rule | Case directories | Coverage |
|---|---|---|---|
| Type-name scopes and bounds | New named type is unavailable in its own right-hand side | [`cases/0.1/pinned/type-self-reference`](cases/0.1/pinned/type-self-reference/) | New |
| Ordinary record names | Whole repeated value outside checks, without implicit quantification | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-ordinary-outside`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-ordinary-outside/) | Existing |
| Ordinary record names | Current local value and enclosing fallback, with fresh iteration frames | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-lookup-frame`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-lookup-frame/) | Existing |
| Ordinary record names | Direct ordinary name in size uses whole array | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-whole-vs-record`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-whole-vs-record/) | Existing |
| Ordinary record names | Direct ordinary comprehension domain uses whole array | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-whole-vs-record`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-whole-vs-record/) | Existing |
| Ordinary record names | Direct ordinary name in defined uses availability without lifting | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-guard-absent`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-guard-absent/) | Existing |
| Ordinary record names | Direct ordinary name in count has no whole-array exception | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-whole-vs-record`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-whole-vs-record/) | Existing |
| Ordinary record names | Whole-array exception is shallow for indexed names | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-size-index-not-whole`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-size-index-not-whole/) | Existing |
| Ordinary record names | Whole-array exception is shallow for field selections | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-size-field-lifts`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-size-field-lifts/) | Existing |
| Ordinary record names | Fields of the same repeat align instead of forming a Cartesian product | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-same-repeat`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-same-repeat/) | Existing |
| Ordinary record names | Nested repeats align inner records with their own outer fields | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-nested-repeat`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-nested-repeat/) | Existing |
| Ordinary record names | Active outer repeat removes only the common repeat-chain prefix | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-nested-completed-current`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-nested-completed-current/) | Existing |
| Ordinary record names | Empty completed records make the check vacuous | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-empty-records`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-empty-records/) | Existing |
| Ordinary record names | One name defined in different completed repeats is ambiguous | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-ambiguous-repeat`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-ambiguous-repeat/) | Existing |
| Ordinary record names | Different names in incompatible completed repeats are rejected | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-cross-repeat`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-cross-repeat/) | Existing |
| Ordinary record names | Inside and outside definitions prohibit implicit lifting | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-inside-outside`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-inside-outside/) | Existing |
| Ordinary record names | Whole-array uses remain legal despite inside/outside definitions | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-ambiguous-whole`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-ambiguous-whole/) | Existing |
| Ordinary record names | All definition suffixes empty gives ordinary local lookup | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-inside-outside-local`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-inside-outside-local/) | Existing |
| Ordinary record names | Multiple definitions in the same repeat are compatible | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-branch-definitions`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-branch-definitions/) | Existing |
| Ordinary record names | Indexing a lifted scalar fails as scalar indexing | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-record-subscript`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-record-subscript/) | Existing |
| Ordinary record names | Unavailable potential collection has no records to traverse; reached lookup fails | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-absent-not-guarded`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-absent-not-guarded/) | Existing |
| Ordinary record names | Guard can skip an unavailable potential collection | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-guard-absent`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-guard-absent/) | Existing |
| Ordinary record names | Later empty repeat does not make an earlier check vacuous | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-check-before-empty`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-check-before-empty/) | Existing |
| Ordinary record names | Unreachable operand retains static repeat-chain restrictions | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-cross-repeat-unreachable`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-cross-repeat-unreachable/) | Existing |
| Ordinary record names | Stored underscore is ordinary lookup, excluded from definition collection | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-store-underscore-lookup`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-store-underscore-lookup/) | Existing |
| Explicit `each` names | as list supports count, indexing, comprehensions and field access | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-record-fields`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-record-fields/) | Existing |
| Explicit `each` names | as list availability begins only after the repeat | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-defined-before-repeat`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-defined-before-repeat/) | Existing |
| Explicit `each` names | as list counts zero records and supports vacuous comprehension | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-zero-record-list`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-zero-record-list/) | Existing |
| Explicit `each` names | as list defined succeeds even for zero records | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-record-defined-list`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-record-defined-list/) | Existing |
| Explicit `each` names | each name denotes the current record inside its repeat | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-record-fields`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-record-fields/) | Existing |
| Explicit `each` names | each name outside its repeat is forbidden in let | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-each-outside`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-each-outside/) | Existing |
| Explicit `each` names | each name outside its repeat implicitly quantifies only in checks | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-mixed-nested-each`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-mixed-nested-each/) | Existing |
| Explicit `each` names | as name cannot be used inside its own repeat | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-self-list`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-self-list/) | Existing |
| Explicit `each` names | Ordinary value.field requires a record base | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-field-not-record`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-field-not-record/) | Existing |
| Explicit `each` names | Record-list.field requires an indexed or bound record base | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-field-list`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-field-list/) | Existing |
| Explicit `each` names | Unknown record field is rejected | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-field-unknown`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-field-unknown/) | Existing |
| Explicit `each` names | Field spelling defined outside its owning repeat is rejected | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-field-outside-definition`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-field-outside-definition/) | Existing |
| Explicit `each` names | Current record field must have executed before runtime lookup | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-current-record-before-field`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-current-record-before-field/) | Existing |
| Explicit `each` names | Single record cannot be stored as a value | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-record-not-value`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-record-not-value/) | Existing |
| Explicit `each` names | Record list cannot be stored as a value | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-list-not-value`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-list-not-value/) | Existing |
| Explicit `each` names | Record cannot be indexed again | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-record-not-indexable`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-record-not-indexable/) | Existing |
| Explicit `each` names | Record cannot be a comprehension domain | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-record-not-domain`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-record-not-domain/) | Existing |
| Explicit `each` names | Structural name repeated across repeats conflicts globally | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-record-name-repeated`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-record-name-repeated/) | Existing |
| Explicit `each` names | Structural name overlapping an ordinary definition conflicts globally | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-record-name-conflict`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-record-name-conflict/) | Existing |
| Explicit `each` names | Identical as and each names conflict | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-record-name-same`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-record-name-same/) | Existing |
| Explicit `each` names | Structural underscore and store underscore can coexist | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-store-underscore-records`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-store-underscore-records/) | Existing |
| Explicit `each` names | Structural name can also be a named type | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-as-type-name`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-as-type-name/) | Existing |
| Explicit `each` names | defined(each) is forbidden outside its repeat | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-record-defined-each`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-record-defined-each/) | Existing |
| Explicit `each` names | defined(each) is forbidden inside its repeat | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-defined-each-inside`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-defined-each-inside/) | Existing |
| Nested repeats and field/list paths | Nested lists require an enclosing record path | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-nested-fields`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-nested-fields/) | Existing |
| Nested repeats and field/list paths | Unqualified nested list outside its enclosing repeat fails | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-nested-list-unqualified`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-nested-list-unqualified/) | Existing |
| Nested repeats and field/list paths | An intervening repeat cannot be skipped in a structural list path | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-nested-unnamed-gap`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-nested-unnamed-gap/) | Existing |
| Nested repeats and field/list paths | Outer record cannot select an unfinished field of an active inner repeat | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-field-incomplete`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-field-incomplete/) | Existing |
| Nested repeats and field/list paths | Explicitly selected record index is omitted from check witness path | [`cases/0.1/spec-examples/08-expressions-and-checks--expressions-indexed-record-witness`](cases/0.1/spec-examples/08-expressions-and-checks--expressions-indexed-record-witness/) | Existing |
| Type-name scopes and bounds | Forward type use is unavailable | [`cases/0.1/spec-examples/06-types-and-values--values-type-forward`](cases/0.1/spec-examples/06-types-and-values--values-type-forward/) | Existing |
| Type-name scopes and bounds | Type use after leaving its defining block is unavailable | [`cases/0.1/spec-examples/06-types-and-values--values-type-escape`](cases/0.1/spec-examples/06-types-and-values--values-type-escape/) | Existing |
| Type-name scopes and bounds | Visible type cannot be shadowed in a nested block | [`cases/0.1/spec-examples/06-types-and-values--values-type-shadow`](cases/0.1/spec-examples/06-types-and-values--values-type-shadow/) | Existing |
| Type-name scopes and bounds | Base-type name cannot be redefined | [`cases/0.1/spec-examples/06-types-and-values--values-type-base`](cases/0.1/spec-examples/06-types-and-values--values-type-base/) | Existing |
| Type-name scopes and bounds | Value/type collision is global even across unreachable blocks | [`cases/0.1/spec-examples/06-types-and-values--values-type-value-conflict`](cases/0.1/spec-examples/06-types-and-values--values-type-value-conflict/) | Existing |
| Type-name scopes and bounds | Sibling blocks can define different types with the same name | [`cases/0.1/spec-examples/06-types-and-values--values-type-siblings`](cases/0.1/spec-examples/06-types-and-values--values-type-siblings/) | Existing |
| Type-name scopes and bounds | Following statements and nested blocks see the defined type | [`cases/0.1/spec-examples/06-types-and-values--values-type-nested`](cases/0.1/spec-examples/06-types-and-values--values-type-nested/) | Existing |
| Type-name scopes and bounds | Named and aliased bounds freeze at definition execution | [`cases/0.1/spec-examples/06-types-and-values--values-range-timing`](cases/0.1/spec-examples/06-types-and-values--values-range-timing/) | Existing |
| Type-name scopes and bounds | Frozen range rejects values accepted only by later bound replacement | [`cases/0.1/spec-examples/06-types-and-values--values-range-frozen-reject`](cases/0.1/spec-examples/06-types-and-values--values-range-frozen-reject/) | Existing |
| Type-name scopes and bounds | Type definitions execute anew in each repeat iteration | [`cases/0.1/spec-examples/06-types-and-values--values-range-reexecution`](cases/0.1/spec-examples/06-types-and-values--values-range-reexecution/) | Existing |
| Type-name scopes and bounds | Unexecuted type definitions never evaluate their bounds | [`cases/0.1/spec-examples/06-types-and-values--values-type-unreached`](cases/0.1/spec-examples/06-types-and-values--values-type-unreached/) | Existing |

## D.8 Matrix layouts

| Appendix subitem | Individual decided rule | Case directories | Coverage |
|---|---|---|---|
| `FULL_MATRIX` | n=0: empty, consumes no cells | [`cases/0.1/pinned/layouts-0`](cases/0.1/pinned/layouts-0/) | New |
| `UPPER_ROW` | n=0: empty, consumes no cells | [`cases/0.1/pinned/layouts-0`](cases/0.1/pinned/layouts-0/) | New |
| `LOWER_ROW` | n=0: empty, consumes no cells | [`cases/0.1/pinned/layouts-0`](cases/0.1/pinned/layouts-0/) | New |
| `UPPER_DIAG_ROW` | n=0: empty, consumes no cells | [`cases/0.1/pinned/layouts-0`](cases/0.1/pinned/layouts-0/) | New |
| `LOWER_DIAG_ROW` | n=0: empty, consumes no cells | [`cases/0.1/pinned/layouts-0`](cases/0.1/pinned/layouts-0/) | New |
| `UPPER_COL` | n=0: empty, consumes no cells | [`cases/0.1/pinned/layouts-0`](cases/0.1/pinned/layouts-0/) | New |
| `LOWER_COL` | n=0: empty, consumes no cells | [`cases/0.1/pinned/layouts-0`](cases/0.1/pinned/layouts-0/) | New |
| `UPPER_DIAG_COL` | n=0: empty, consumes no cells | [`cases/0.1/pinned/layouts-0`](cases/0.1/pinned/layouts-0/) | New |
| `LOWER_DIAG_COL` | n=0: empty, consumes no cells | [`cases/0.1/pinned/layouts-0`](cases/0.1/pinned/layouts-0/) | New |
| `FULL_MATRIX` | n=1: diagonal consumption, offset, and unread zero | [`cases/0.1/pinned/layouts-1`](cases/0.1/pinned/layouts-1/) | New |
| `UPPER_ROW` | n=1: diagonal consumption, offset, and unread zero | [`cases/0.1/pinned/layouts-1`](cases/0.1/pinned/layouts-1/) | New |
| `LOWER_ROW` | n=1: diagonal consumption, offset, and unread zero | [`cases/0.1/pinned/layouts-1`](cases/0.1/pinned/layouts-1/) | New |
| `UPPER_DIAG_ROW` | n=1: diagonal consumption, offset, and unread zero | [`cases/0.1/pinned/layouts-1`](cases/0.1/pinned/layouts-1/) | New |
| `LOWER_DIAG_ROW` | n=1: diagonal consumption, offset, and unread zero | [`cases/0.1/pinned/layouts-1`](cases/0.1/pinned/layouts-1/) | New |
| `UPPER_COL` | n=1: diagonal consumption, offset, and unread zero | [`cases/0.1/pinned/layouts-1`](cases/0.1/pinned/layouts-1/) | New |
| `LOWER_COL` | n=1: diagonal consumption, offset, and unread zero | [`cases/0.1/pinned/layouts-1`](cases/0.1/pinned/layouts-1/) | New |
| `UPPER_DIAG_COL` | n=1: diagonal consumption, offset, and unread zero | [`cases/0.1/pinned/layouts-1`](cases/0.1/pinned/layouts-1/) | New |
| `LOWER_DIAG_COL` | n=1: diagonal consumption, offset, and unread zero | [`cases/0.1/pinned/layouts-1`](cases/0.1/pinned/layouts-1/) | New |
| `FULL_MATRIX` | Distinct 4×4 values establish row-major asymmetric order without mirroring | [`cases/0.1/spec-examples/06-types-and-values--values-layout-full-matrix`](cases/0.1/spec-examples/06-types-and-values--values-layout-full-matrix/) | Existing |
| `UPPER_ROW` | Distinct 4×4 values establish strict triangle, omitted diagonal zero row order, offset before mirroring | [`cases/0.1/spec-examples/06-types-and-values--values-layout-upper-row`](cases/0.1/spec-examples/06-types-and-values--values-layout-upper-row/) | Existing |
| `LOWER_ROW` | Distinct 4×4 values establish strict triangle, omitted diagonal zero row order, offset before mirroring | [`cases/0.1/spec-examples/06-types-and-values--values-layout-lower-row`](cases/0.1/spec-examples/06-types-and-values--values-layout-lower-row/) | Existing |
| `UPPER_DIAG_ROW` | Distinct 4×4 values establish diagonal-inclusive row order, offset before mirroring | [`cases/0.1/spec-examples/06-types-and-values--values-layout-upper-diag-row`](cases/0.1/spec-examples/06-types-and-values--values-layout-upper-diag-row/) | Existing |
| `LOWER_DIAG_ROW` | Distinct 4×4 values establish diagonal-inclusive row order, offset before mirroring | [`cases/0.1/spec-examples/06-types-and-values--values-layout-lower-diag-row`](cases/0.1/spec-examples/06-types-and-values--values-layout-lower-diag-row/) | Existing |
| `UPPER_COL` | Distinct 4×4 values establish strict triangle, omitted diagonal zero column order, offset before mirroring | [`cases/0.1/spec-examples/06-types-and-values--values-layout-upper-col`](cases/0.1/spec-examples/06-types-and-values--values-layout-upper-col/) | Existing |
| `LOWER_COL` | Distinct 4×4 values establish strict triangle, omitted diagonal zero column order, offset before mirroring | [`cases/0.1/spec-examples/06-types-and-values--values-layout-lower-col`](cases/0.1/spec-examples/06-types-and-values--values-layout-lower-col/) | Existing |
| `UPPER_DIAG_COL` | Distinct 4×4 values establish diagonal-inclusive column order, offset before mirroring | [`cases/0.1/spec-examples/06-types-and-values--values-layout-upper-diag-col`](cases/0.1/spec-examples/06-types-and-values--values-layout-upper-diag-col/) | Existing |
| `LOWER_DIAG_COL` | Distinct 4×4 values establish diagonal-inclusive column order, offset before mirroring | [`cases/0.1/spec-examples/06-types-and-values--values-layout-lower-diag-col`](cases/0.1/spec-examples/06-types-and-values--values-layout-lower-diag-col/) | Existing |
| `FULL_MATRIX` | Rectangular full layout is accepted | [`cases/0.1/spec-examples/06-types-and-values--values-layout-rectangular`](cases/0.1/spec-examples/06-types-and-values--values-layout-rectangular/) | Existing |
| Triangular layout validation | Reject non-string layout expression | [`cases/0.1/spec-examples/06-types-and-values--values-layout-type`](cases/0.1/spec-examples/06-types-and-values--values-layout-type/) | Existing |
| Triangular layout validation | Reject nonsquare triangular matrix | [`cases/0.1/spec-examples/06-types-and-values--values-layout-square`](cases/0.1/spec-examples/06-types-and-values--values-layout-square/) | Existing |
| Triangular layout validation | Reject unknown case-sensitive layout name | [`cases/0.1/spec-examples/06-types-and-values--values-layout-unknown`](cases/0.1/spec-examples/06-types-and-values--values-layout-unknown/) | Existing |
| Triangular layout validation | Enforce n*n structural cell limit before reading | [`cases/0.1/spec-examples/06-types-and-values--values-layout-limit`](cases/0.1/spec-examples/06-types-and-values--values-layout-limit/) | Existing |

## D.9 Instances and catalog

| Appendix subitem | Individual decided rule | Case directories | Coverage |
|---|---|---|---|
| Name selection, duplicates, and empty names | float-spelling: exact name selection and local-name derivation | [`cases/0.1/pinned/select-float-spelling`](cases/0.1/pinned/select-float-spelling/) | New |
| Name selection, duplicates, and empty names | whitespace: exact name selection and local-name derivation | [`cases/0.1/pinned/select-whitespace`](cases/0.1/pinned/select-whitespace/) | New |
| Name selection, duplicates, and empty names | empty-float: exact name selection and local-name derivation | [`cases/0.1/pinned/select-empty-float`](cases/0.1/pinned/select-empty-float/) | New |
| Name selection, duplicates, and empty names | empty-array: exact name selection and local-name derivation | [`cases/0.1/pinned/select-empty-array`](cases/0.1/pinned/select-empty-array/) | New |
| Ordinal selection and name priority | plus: complete integer selects the ordinal before a numeric-looking name | [`cases/0.1/pinned/select-ordinal-plus`](cases/0.1/pinned/select-ordinal-plus/) | New |
| Ordinal selection and name priority | zeros: complete integer selects the ordinal before a numeric-looking name | [`cases/0.1/pinned/select-ordinal-zeros`](cases/0.1/pinned/select-ordinal-zeros/) | New |
| Ordinal selection and name priority | space: incomplete integer conversion falls through to exact name without trimming | [`cases/0.1/pinned/select-name-space`](cases/0.1/pinned/select-name-space/) | New |
| Ordinal selection and name priority | suffix: incomplete integer conversion falls through to exact name without trimming | [`cases/0.1/pinned/select-name-suffix`](cases/0.1/pinned/select-name-suffix/) | New |
| Ordinal selection and name priority | Complete integer selects 1-based ordinal before name | [`cases/0.1/spec-examples/09-params-and-instances--instances-ordinal-before-name`](cases/0.1/spec-examples/09-params-and-instances--instances-ordinal-before-name/) | Existing |
| Ordinal selection and name priority | Nonpositive ordinal fails before empty-input testing | [`cases/0.1/spec-examples/09-params-and-instances--instances-invalid-number`](cases/0.1/spec-examples/09-params-and-instances--instances-invalid-number/) | Existing |
| Ordinal selection and name priority | Overflowing integer selector falls through to exact name | [`cases/0.1/spec-examples/09-params-and-instances--instances-text-selection`](cases/0.1/spec-examples/09-params-and-instances--instances-text-selection/) | Existing |
| Name selection, duplicates, and empty names | Duplicate names retain the first completed matching instance | [`cases/0.1/spec-examples/09-params-and-instances--instances-duplicate-names`](cases/0.1/spec-examples/09-params-and-instances--instances-duplicate-names/) | Existing |
| Name selection, duplicates, and empty names | Empty selector matches absent local name | [`cases/0.1/spec-examples/09-params-and-instances--instances-empty-name-selection`](cases/0.1/spec-examples/09-params-and-instances--instances-empty-name-selection/) | Existing |
| Retained data, frames, and unread tail | Skipped frames stay isolated; retained frame replaces prefix entries in order | [`cases/0.1/spec-examples/09-params-and-instances--instances-frame-selection`](cases/0.1/spec-examples/09-params-and-instances--instances-frame-selection/) | Existing |
| Retained data, frames, and unread tail | Explicit selection returns before executing unread invalid tail | [`cases/0.1/spec-examples/09-params-and-instances--instances-frame-selection`](cases/0.1/spec-examples/09-params-and-instances--instances-frame-selection/) | Existing |
| Retained data, frames, and unread tail | No selector publishes retained first frame before later traversal | [`cases/0.1/spec-examples/09-params-and-instances--instances-retained-frame-visible`](cases/0.1/spec-examples/09-params-and-instances--instances-retained-frame-visible/) | Existing |
| Retained data, frames, and unread tail | Exactly one instance returns its data | [`cases/0.1/spec-examples/09-params-and-instances--instances-single`](cases/0.1/spec-examples/09-params-and-instances--instances-single/) | Existing |
| Retained data, frames, and unread tail | No instance gives E-INSTANCE-EMPTY | [`cases/0.1/spec-examples/09-params-and-instances--instances-no-instance`](cases/0.1/spec-examples/09-params-and-instances--instances-no-instance/) | Existing |
| Retained data, frames, and unread tail | Unmatched selector gives E-INSTANCE-NOT-FOUND | [`cases/0.1/spec-examples/09-params-and-instances--instances-not-found`](cases/0.1/spec-examples/09-params-and-instances--instances-not-found/) | Existing |
| Retained data, frames, and unread tail | Multiple instances require a caller selector; param default does not select | [`cases/0.1/spec-examples/09-params-and-instances--instances-selection-required`](cases/0.1/spec-examples/09-params-and-instances--instances-selection-required/) | Existing |
| Catalog entries, optionality, and order | Count then optional names then failed summaries, in input pass order | [`cases/0.1/spec-examples/09-params-and-instances--catalog-failure-summaries`](cases/0.1/spec-examples/09-params-and-instances--catalog-failure-summaries/) | Existing |
| Catalog entries, optionality, and order | Empty block input returns count 0 with no optional vectors | [`cases/0.1/spec-examples/09-params-and-instances--catalog-empty-file`](cases/0.1/spec-examples/09-params-and-instances--catalog-empty-file/) | Existing |
| Catalog entries, optionality, and order | No block executes once and omits ordinary name entry | [`cases/0.1/spec-examples/09-params-and-instances--catalog-without-block`](cases/0.1/spec-examples/09-params-and-instances--catalog-without-block/) | Existing |
| Catalog entries, optionality, and order | Scalar int names normalize to decimal; duplicate names remain | [`cases/0.1/spec-examples/09-params-and-instances--catalog-usable-names`](cases/0.1/spec-examples/09-params-and-instances--catalog-usable-names/) | Existing |
| Catalog entries, optionality, and order | Absent, empty, float or array local name suppresses entire names vector | [`cases/0.1/spec-examples/09-params-and-instances--catalog-unusable-name`](cases/0.1/spec-examples/09-params-and-instances--catalog-unusable-name/) | Existing |
| Catalog entries, optionality, and order | Catalog validates nonpositive selector even though it ignores selection | [`cases/0.1/spec-examples/09-params-and-instances--catalog-bad-selector`](cases/0.1/spec-examples/09-params-and-instances--catalog-bad-selector/) | Existing |
| Catalog rejection summaries and recovery | Nonempty check message is retained exactly | [`cases/0.1/spec-examples/09-params-and-instances--catalog-failure-summaries`](cases/0.1/spec-examples/09-params-and-instances--catalog-failure-summaries/) | Existing |
| Catalog rejection summaries and recovery | Empty or omitted check message uses exact reader-spec line summary | [`cases/0.1/spec-examples/09-params-and-instances--catalog-failure-summaries`](cases/0.1/spec-examples/09-params-and-instances--catalog-failure-summaries/) | Existing |
| Catalog rejection summaries and recovery | Repeated-arm summary omits first-line diagnostic detail | [`cases/0.1/spec-examples/09-params-and-instances--catalog-failure-summaries`](cases/0.1/spec-examples/09-params-and-instances--catalog-failure-summaries/) | Existing |
| Catalog rejection summaries and recovery | Missing-arm summary uses the decoded pattern | [`cases/0.1/spec-examples/09-params-and-instances--catalog-failure-summaries`](cases/0.1/spec-examples/09-params-and-instances--catalog-failure-summaries/) | Existing |
| Catalog rejection summaries and recovery | Only first rejection summary is retained; valid passes use empty string | [`cases/0.1/spec-examples/09-params-and-instances--catalog-failure-summaries`](cases/0.1/spec-examples/09-params-and-instances--catalog-failure-summaries/) | Existing |
| Catalog rejection summaries and recovery | Repeated captures commit during deferral and can replace name | [`cases/0.1/spec-examples/09-params-and-instances--catalog-failure-summaries`](cases/0.1/spec-examples/09-params-and-instances--catalog-failure-summaries/) | Existing |
| Catalog rejection summaries and recovery | Supplied ordinal does not restrict catalog traversal | [`cases/0.1/spec-examples/09-params-and-instances--catalog-failure-summaries`](cases/0.1/spec-examples/09-params-and-instances--catalog-failure-summaries/) | Existing |
| Catalog rejection summaries and recovery | Missing-arm EOF aborts without a partial catalog | [`cases/0.1/spec-examples/09-params-and-instances--catalog-missing-arm-at-end`](cases/0.1/spec-examples/09-params-and-instances--catalog-missing-arm-at-end/) | Existing |
| Catalog rejection summaries and recovery | Rejection outside instance body is immediate | [`cases/0.1/spec-examples/09-params-and-instances--catalog-no-block-check`](cases/0.1/spec-examples/09-params-and-instances--catalog-no-block-check/) | Existing |

## D.10 dzn output

| Appendix subitem | Individual decided rule | Case directories | Coverage |
|---|---|---|---|
| Names, integers, and entry order | Every member of the frozen quoting set; case-sensitive near misses and underscore prefix | [`cases/0.1/pinned/dzn-keyword-set`](cases/0.1/pinned/dzn-keyword-set/) | New |
| Catalog, separators, LF, and errors | Rendering at expression equality adds zero expression effort | [`cases/0.1/pinned/dzn-effort-equal`](cases/0.1/pinned/dzn-effort-equal/) | New |
| Catalog, separators, LF, and errors | Read failure remains an error document; no partial dzn assignment | [`cases/0.1/pinned/dzn-read-error`](cases/0.1/pinned/dzn-read-error/) | New |
| Rectangular, empty, ragged, and deep arrays | Different extents under different parents are ragged even when siblings have equal lengths | [`cases/0.1/pinned/dzn-cross-parent-shape`](cases/0.1/pinned/dzn-cross-parent-shape/) | New |
| Rectangular, empty, ragged, and deep arrays | One-row positive matrix uses ordinary matrix punctuation | [`cases/0.1/pinned/dzn-thin-matrices`](cases/0.1/pinned/dzn-thin-matrices/) | New |
| Rectangular, empty, ragged, and deep arrays | One-column positive matrix uses ordinary matrix punctuation and row LFs | [`cases/0.1/pinned/dzn-thin-matrices`](cases/0.1/pinned/dzn-thin-matrices/) | New |
| Names, integers, and entry order | Replacement retains entry position; metadata creates no entry | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-scalars-names-order`](cases/0.1/spec-examples/appendix-b-dzn--dzn-scalars-names-order/) | Existing |
| Names, integers, and entry order | Signed integer minimum and maximum render exact decimal text | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-scalars-names-order`](cases/0.1/spec-examples/appendix-b-dzn--dzn-scalars-names-order/) | Existing |
| Float text | Positive and negative stored zero render with decimal point and sign | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-float-spellings`](cases/0.1/spec-examples/appendix-b-dzn--dzn-float-spellings/) | Existing |
| Float text | Increasing precision finds round-trip decimal digits | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-float-spellings`](cases/0.1/spec-examples/appendix-b-dzn--dzn-float-spellings/) | Existing |
| Float text | At exponent -4 use fixed notation; at -5 scientific | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-float-spellings`](cases/0.1/spec-examples/appendix-b-dzn--dzn-float-spellings/) | Existing |
| Float text | Integral fixed notation below 1e17; scientific at 1e17 | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-float-spellings`](cases/0.1/spec-examples/appendix-b-dzn--dzn-float-spellings/) | Existing |
| Float text | Scientific exponent uses lowercase e, explicit sign and at least two digits | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-float-spellings`](cases/0.1/spec-examples/appendix-b-dzn--dzn-float-spellings/) | Existing |
| Float text | Normal binary64 minimum and maximum render finite round-trip text | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-float-spellings`](cases/0.1/spec-examples/appendix-b-dzn--dzn-float-spellings/) | Existing |
| Floats just below 1e17 | Exact starting precision renders gap leaves; complete C.4 alternate records omitted text | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-float-precision-gap`](cases/0.1/spec-examples/appendix-b-dzn--dzn-float-precision-gap/) | Existing |
| String escaping | Empty string is quoted | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-string-bytes`](cases/0.1/spec-examples/appendix-b-dzn--dzn-string-bytes/) | Existing |
| String escaping | Double quote and backslash use backslash escapes | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-string-bytes`](cases/0.1/spec-examples/appendix-b-dzn--dzn-string-bytes/) | Existing |
| String escaping | LF and tab use named escapes | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-string-bytes`](cases/0.1/spec-examples/appendix-b-dzn--dzn-string-bytes/) | Existing |
| String escaping | Other controls 00..1f use exactly two lowercase hex digits | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-string-bytes`](cases/0.1/spec-examples/appendix-b-dzn--dzn-string-bytes/) | Existing |
| String escaping | DEL, slash, UTF-8 and combining sequences stay literal | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-string-bytes`](cases/0.1/spec-examples/appendix-b-dzn--dzn-string-bytes/) | Existing |
| Rectangular, empty, ragged, and deep arrays | Depth-1 vectors separate scalar leaves by comma-space | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-rectangular-arrays`](cases/0.1/spec-examples/appendix-b-dzn--dzn-rectangular-arrays/) | Existing |
| Rectangular, empty, ragged, and deep arrays | Positive depth-2 matrix uses matrix punctuation and row LFs | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-rectangular-arrays`](cases/0.1/spec-examples/appendix-b-dzn--dzn-rectangular-arrays/) | Existing |
| Rectangular, empty, ragged, and deep arrays | Rectangular depth 3 uses array3d with row-major leaves | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-rectangular-arrays`](cases/0.1/spec-examples/appendix-b-dzn--dzn-rectangular-arrays/) | Existing |
| Rectangular, empty, ragged, and deep arrays | Rectangular depth 4 uses array4d with row-major leaves | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-rectangular-arrays`](cases/0.1/spec-examples/appendix-b-dzn--dzn-rectangular-arrays/) | Existing |
| Rectangular, empty, ragged, and deep arrays | Rectangular depth 5 uses array5d with row-major leaves | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-rectangular-arrays`](cases/0.1/spec-examples/appendix-b-dzn--dzn-rectangular-arrays/) | Existing |
| Rectangular, empty, ragged, and deep arrays | Rectangular depth 6 uses array6d with row-major leaves | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-rectangular-arrays`](cases/0.1/spec-examples/appendix-b-dzn--dzn-rectangular-arrays/) | Existing |
| Rectangular, empty, ragged, and deep arrays | Empty vectors preserve value type in data but render [] | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-empty-arrays`](cases/0.1/spec-examples/appendix-b-dzn--dzn-empty-arrays/) | Existing |
| Rectangular, empty, ragged, and deep arrays | Empty matrix uses array2d; unrepresented inner extents become 0 | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-empty-arrays`](cases/0.1/spec-examples/appendix-b-dzn--dzn-empty-arrays/) | Existing |
| Rectangular, empty, ragged, and deep arrays | Empty depth 3 uses array3d with inferred zero extents | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-empty-arrays`](cases/0.1/spec-examples/appendix-b-dzn--dzn-empty-arrays/) | Existing |
| Rectangular, empty, ragged, and deep arrays | Empty depth 4 uses array4d with inferred zero extents | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-empty-arrays`](cases/0.1/spec-examples/appendix-b-dzn--dzn-empty-arrays/) | Existing |
| Rectangular, empty, ragged, and deep arrays | Empty depth 5 uses array5d with inferred zero extents | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-empty-arrays`](cases/0.1/spec-examples/appendix-b-dzn--dzn-empty-arrays/) | Existing |
| Rectangular, empty, ragged, and deep arrays | Empty depth 6 uses array6d with inferred zero extents | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-empty-arrays`](cases/0.1/spec-examples/appendix-b-dzn--dzn-empty-arrays/) | Existing |
| Rectangular, empty, ragged, and deep arrays | Ragged int/float/string arrays use whole-entry nested one-tuples and type comments | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-ragged-and-deep`](cases/0.1/spec-examples/appendix-b-dzn--dzn-ragged-and-deep/) | Existing |
| Rectangular, empty, ragged, and deep arrays | Depth greater than 6 uses nested one-tuples even when rectangular | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-ragged-and-deep`](cases/0.1/spec-examples/appendix-b-dzn--dzn-ragged-and-deep/) | Existing |
| Rectangular, empty, ragged, and deep arrays | Completely empty deep array retains depth and type in comment only | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-empty-arrays`](cases/0.1/spec-examples/appendix-b-dzn--dzn-empty-arrays/) | Existing |
| Catalog, separators, LF, and errors | Render catalog entries in count/name/failed order with exact summary strings | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-catalog-summaries`](cases/0.1/spec-examples/appendix-b-dzn--dzn-catalog-summaries/) | Existing |
| Catalog, separators, LF, and errors | Assignments use exact spaces, semicolon and one LF; no intervening blank lines | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-scalars-names-order`](cases/0.1/spec-examples/appendix-b-dzn--dzn-scalars-names-order/) | Existing |
| Catalog, separators, LF, and errors | Zero entries render zero bytes, even at idle equality | [`cases/0.1/spec-examples/appendix-b-dzn--dzn-empty-data`](cases/0.1/spec-examples/appendix-b-dzn--dzn-empty-data/) | Existing |
