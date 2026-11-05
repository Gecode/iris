# D Pinned details (informative)

This appendix indexes the details identified in
[PLAN.md §6.2](../../PLAN.md#62-what-the-doxygen-text-leaves-open).
The normative decisions are in the linked chapters and
[appendix C](appendix-c-differences.md). The reference pin is
`599f977b7855dba2a1a5f67db1892722372067d3`, as defined in
[§1.2](01-introduction.md#12-reference-behavior).

Each example link names an exact `id` from an extractable `iris example`
block and leads to its containing section. Reference status distinguishes
examples that agree with the pin from deliberate Iris differences.

## D.1 Input lexing

| PLAN.md §6.2 subitem | Decision | Normative section | Executable examples | Reference status |
|---|---|---|---|---|
| Integer token syntax | Complete signed decimal, within the signed 64-bit domain. | [§4.4](04-input.md#44-integer-tokens) | [`input-integer-spellings`](04-input.md#44-integer-tokens), [`input-integer-token`](04-input.md#44-integer-tokens) | Pinned reference |
| Float token syntax and exponents | Complete decimal binary64 conversion with the specified rounding and underflow rules. | [§4.5](04-input.md#45-floating-point-tokens) | [`input-float-spellings`](04-input.md#45-floating-point-tokens), [`input-float-normal-boundary`](04-input.md#45-floating-point-tokens), [`input-float-underflow`](04-input.md#45-floating-point-tokens) | Pinned reference |
| Leading `+` for integers | Accepted on input. | [§4.4](04-input.md#44-integer-tokens) | [`input-integer-spellings`](04-input.md#44-integer-tokens) | Pinned reference |
| Leading `+` for floats | Accepted on input. | [§4.5](04-input.md#45-floating-point-tokens) | [`input-float-spellings`](04-input.md#45-floating-point-tokens) | Pinned reference |
| `inf` rejection | Infinity spellings are rejected. | [§4.5](04-input.md#45-floating-point-tokens) | [`input-float-inf`](04-input.md#45-floating-point-tokens) | Pinned reference |
| `nan` rejection | NaN spellings are rejected. | [§4.5](04-input.md#45-floating-point-tokens) | [`input-float-nan`](04-input.md#45-floating-point-tokens) | Pinned reference |
| Hexadecimal integer rejection | Hexadecimal input is rejected. | [§4.4](04-input.md#44-integer-tokens) | [`input-integer-hex`](04-input.md#44-integer-tokens) | Pinned reference |
| Hexadecimal float rejection | Hexadecimal input is rejected. | [§4.5](04-input.md#45-floating-point-tokens) | [`input-float-hex`](04-input.md#45-floating-point-tokens) | Pinned reference |
| Default whitespace | Six ASCII bytes separate ordinary tokens. | [§4.2](04-input.md#42-white-space-and-line-blanks) | [`input-ascii-whitespace`](04-input.md#42-white-space-and-line-blanks), [`notation-crlf-tab`](02-notation.md#25-complete-examples) | Pinned reference |
| VT/FF versus line blanks | VT and FF separate tokens but remain in line text and do not satisfy pattern blanks. | [§4.2](04-input.md#42-white-space-and-line-blanks), [§7.2](07-structure.md#72-line-patterns-and-captures) | [`input-ascii-whitespace`](04-input.md#42-white-space-and-line-blanks), [`input-line-blanks`](04-input.md#42-white-space-and-line-blanks), [`input-pattern-blanks`](04-input.md#42-white-space-and-line-blanks) | Pinned reference |
| CR LF and lone CR | Only LF ends a physical line; CR counts as one byte. | [§4.1](04-input.md#41-bytes-and-physical-lines), [§4.6](04-input.md#46-input-positions) | [`input-line-endings`](04-input.md#41-bytes-and-physical-lines), [`input-crlf-columns`](04-input.md#46-input-positions), [`input-cr-columns`](04-input.md#46-input-positions) | Pinned reference |
| Final newline | A final LF is optional, including for patterns. | [§4.1](04-input.md#41-bytes-and-physical-lines), [§7.3](07-structure.md#73-line-find-skip-and-expect) | [`input-line-endings`](04-input.md#41-bytes-and-physical-lines), [`structure-line-final-lf`](07-structure.md#73-line-find-skip-and-expect) | Pinned reference |

## D.2 Positions and UTF-8

| PLAN.md §6.2 subitem | Decision | Normative section | Executable examples | Reference status |
|---|---|---|---|---|
| Column unit | Columns count bytes, independently of Unicode code points or grapheme clusters. | [§4.6](04-input.md#46-input-positions) | [`input-byte-columns`](04-input.md#46-input-positions) | Pinned reference |
| Tabs | A tab advances the column by one. | [§4.6](04-input.md#46-input-positions) | [`input-byte-columns`](04-input.md#46-input-positions), [`input-crlf-columns`](04-input.md#46-input-positions) | Pinned reference |
| Input-wide UTF-8 | Validate all supplied bytes after parameter admission and before execution; locate the first ill-formed subsequence. | [§4.10](04-input.md#410-utf-8-validity) | [`input-invalid-utf8-comment`](04-input.md#410-utf-8-validity), [`input-invalid-utf8-tail`](04-input.md#410-utf-8-validity) | Iris difference C.1 |
| Read-value UTF-8 | Validate each actually read word/rest value, including when byte settings split valid input. | [§4.10](04-input.md#410-utf-8-validity) | [`input-invalid-utf8-split`](04-input.md#410-utf-8-validity) | Iris difference C.1 |
| Reader-spec UTF-8 | Validate the whole source before lexing, with a byte column on failure. | [§5.1](05-reader-specs.md#51-reader-specs-and-parsing) | [`reader-invalid-utf8-string`](05-reader-specs.md#51-reader-specs-and-parsing), [`reader-invalid-utf8-comment`](05-reader-specs.md#51-reader-specs-and-parsing) | Iris difference C.2 |

## D.3 Comment and stop prefixes

| PLAN.md §6.2 subitem | Decision | Normative section | Executable examples | Reference status |
|---|---|---|---|---|
| Prefix boundary class | The final and following bytes use ASCII letters, digits, and underscore. | [§4.7](04-input.md#47-comment-and-stop-prefixes) | [`input-comment-boundaries`](04-input.md#47-comment-and-stop-prefixes), [`input-prefix-underscore`](04-input.md#47-comment-and-stop-prefixes) | Pinned reference |
| Non-ASCII prefix/following bytes | Non-ASCII bytes are outside the boundary class. | [§4.7](04-input.md#47-comment-and-stop-prefixes) | [`input-prefix-nonascii`](04-input.md#47-comment-and-stop-prefixes), [`input-comment-boundaries`](04-input.md#47-comment-and-stop-prefixes) | Pinned reference |
| Stop boundary rule | Stop uses the same boundary test as comment. | [§4.7](04-input.md#47-comment-and-stop-prefixes) | [`input-stop-boundaries`](04-input.md#47-comment-and-stop-prefixes) | Pinned reference |

## D.4 Work budgets

| PLAN.md §6.2 subitem | Decision | Normative section | Executable examples | Reference status |
|---|---|---|---|---|
| Idle limit and iterations | B = 2^20; charge each counted iteration that consumes no input; equality succeeds. | [§10.1](10-work-budgets.md#101-counters-and-input-byte-length), [§10.2](10-work-budgets.md#102-idle-work) | [`budgets-idle-under`](10-work-budgets.md#107-executable-boundary-examples), [`budgets-idle-equal`](10-work-budgets.md#107-executable-boundary-examples), [`budgets-idle-over`](10-work-budgets.md#107-executable-boundary-examples) | Pinned reference |
| Empty-row charges | Charge positive dimension prefixes of empty ordinary arrays and the row count of zero-column alphabets. | [§10.2](10-work-budgets.md#102-idle-work) | [`budgets-idle-empty-prefixes`](10-work-budgets.md#107-executable-boundary-examples), [`values-alphabet-empty`](06-types-and-values.md#610-alphabets) | Pinned reference |
| Expression formula and charges | Limit B + 16*N; use the exact event and lifted-dependency tables. | [§10.1](10-work-budgets.md#101-counters-and-input-byte-length), [§10.3](10-work-budgets.md#103-expression-effort), [§10.4](10-work-budgets.md#104-lifted-check-traversal) | [`budgets-effort-under`](10-work-budgets.md#107-executable-boundary-examples), [`budgets-effort-equal`](10-work-budgets.md#107-executable-boundary-examples), [`budgets-effort-over`](10-work-budgets.md#107-executable-boundary-examples) | Pinned reference |
| Input byte length | N is the entire original byte length, including ignored tails and multibyte characters. | [§10.1](10-work-budgets.md#101-counters-and-input-byte-length) | [`budgets-effort-raw-bytes`](10-work-budgets.md#107-executable-boundary-examples) | Pinned reference |
| Counter lifetime | Both counters span the whole read or catalog operation without resets. | [§10.1](10-work-budgets.md#101-counters-and-input-byte-length), [§10.5](10-work-budgets.md#105-evaluation-instances-and-catalog) | [`budgets-idle-no-reset`](10-work-budgets.md#107-executable-boundary-examples) | Pinned reference |
| Exact budget arithmetic | Mathematical nonnegative integers; the ordinary boundary examples do not reproduce the pinned overflow bug (see below). | [§10.1](10-work-budgets.md#101-counters-and-input-byte-length) | [`budgets-effort-equal`](10-work-budgets.md#107-executable-boundary-examples), [`budgets-effort-over`](10-work-budgets.md#107-executable-boundary-examples) | Iris difference C.3 |

C.3 is a source-backed correction to overflow in the pinned budget
arithmetic. No full-size reader/input reproduction was run; the linked
ordinary boundary examples agree with the reference and cover accounting,
not overflow. [C.3](appendix-c-differences.md#c3-exact-budget-arithmetic)
identifies the affected source expressions and the verification limit.

## D.5 Integer semantics

| PLAN.md §6.2 subitem | Decision | Normative section | Executable examples | Reference status |
|---|---|---|---|---|
| 64-bit checked arithmetic | All integer arithmetic is exact or fails on overflow. | [§8.4](08-expressions-and-checks.md#84-integer-arithmetic) | [`data-integer-limits`](03-data-model.md#34-integers), [`expressions-add-overflow`](08-expressions-and-checks.md#84-integer-arithmetic), [`expressions-addition-underflow`](08-expressions-and-checks.md#84-integer-arithmetic), [`expressions-negate-minimum`](08-expressions-and-checks.md#84-integer-arithmetic) | Pinned reference |
| Truncating division | The quotient truncates toward zero. | [§8.4](08-expressions-and-checks.md#84-integer-arithmetic) | [`expressions-integer-results`](08-expressions-and-checks.md#84-integer-arithmetic), [`expressions-minimum-div-minus-one`](08-expressions-and-checks.md#84-integer-arithmetic) | Pinned reference |
| Negative remainder operands | The remainder has the dividend's sign; minimum-int modulo -1 fails. | [§8.4](08-expressions-and-checks.md#84-integer-arithmetic) | [`expressions-integer-results`](08-expressions-and-checks.md#84-integer-arithmetic), [`expressions-minimum-mod-minus-one`](08-expressions-and-checks.md#84-integer-arithmetic) | Pinned reference |
| Power results and overflow | Nonnegative exponents produce exact in-range powers; negative exponents and overflow fail. | [§8.4](08-expressions-and-checks.md#84-integer-arithmetic) | [`expressions-power-results`](08-expressions-and-checks.md#84-integer-arithmetic), [`expressions-power-overflow`](08-expressions-and-checks.md#84-integer-arithmetic), [`expressions-negative-exponent`](08-expressions-and-checks.md#84-integer-arithmetic) | Pinned reference |

## D.6 Error order

| PLAN.md §6.2 subitem | Decision | Normative section | Executable examples | Reference status |
|---|---|---|---|---|
| Phase order | Complete-source lexing precedes parsing; global conflicts precede resolution, parameter admission, and execution. | [§11.2](errors.md#112-error-order) | [`errors-lex-before-syntax`](errors.md#118-executable-examples), [`errors-binding-before-type`](errors.md#118-executable-examples) | Pinned reference |
| Structural prechecks and component order | Structural-name conflicts precede expression resolution; type bounds resolve before dimensions. | [§11.2](errors.md#112-error-order) | [`errors-record-precheck-before-field`](errors.md#118-executable-examples), [`errors-resolver-type-before-dimensions`](errors.md#118-executable-examples) | Pinned reference |
| Runtime statement and operand order | Follow executed statements, each statement's local evaluation order, and operand order. | [§11.2](errors.md#112-error-order), [§6.7](06-types-and-values.md#67-offsets-and-evaluation-order), [§8.1](08-expressions-and-checks.md#81-values-and-value-lookup), [§8.9](08-expressions-and-checks.md#89-indexing) | [`expressions-operand-before-type`](08-expressions-and-checks.md#81-values-and-value-lookup), [`expressions-index-base-first`](08-expressions-and-checks.md#89-indexing), [`values-offset-list-timing`](06-types-and-values.md#67-offsets-and-evaluation-order) | Pinned reference |
| Sections ties | The first matching arm and first missing required arm use declaration order. | [§7.8](07-structure.md#78-sections) | [`structure-sections-first-arm`](07-structure.md#78-sections), [`structure-sections-missing`](07-structure.md#710-errors-and-execution-context) | Pinned reference |
| Deferred failures | Only specified instance rejections defer; later hard or immediately selected failures can become primary. | [§9.5](09-params-and-instances.md#95-deferred-failures-in-skipped-instances), [§11.2](errors.md#112-error-order) | [`instances-name-becomes-selected`](09-params-and-instances.md#95-deferred-failures-in-skipped-instances), [`instances-skipped-hard-error`](09-params-and-instances.md#95-deferred-failures-in-skipped-instances) | Pinned reference |

## D.7 Name scopes

| PLAN.md §6.2 subitem | Decision | Normative section | Executable examples | Reference status |
|---|---|---|---|---|
| Ordinary record names | Use the remaining-repeat judgments and shallow whole-array exceptions; `size` has none. | [§8.12](08-expressions-and-checks.md#812-record-names-and-field-access) | [`expressions-whole-vs-record`](08-expressions-and-checks.md#812-record-names-and-field-access), [`expressions-ambiguous-repeat`](08-expressions-and-checks.md#812-record-names-and-field-access) | Pinned reference, except Iris difference C.5 |
| Explicit `each` names | Current record inside its repeat; implicit quantification only in checks outside it. | [§8.12](08-expressions-and-checks.md#812-record-names-and-field-access), [§7.6](07-structure.md#76-logical-records-and-names) | [`structure-record-each-only`](07-structure.md#76-logical-records-and-names), [`expressions-each-outside`](08-expressions-and-checks.md#812-record-names-and-field-access) | Pinned reference |
| Nested repeats and field/list paths | Align nested records and require explicit paths for nested structural lists. | [§8.12](08-expressions-and-checks.md#812-record-names-and-field-access), [§7.1](07-structure.md#71-statement-execution-and-frames) | [`expressions-nested-fields`](08-expressions-and-checks.md#812-record-names-and-field-access), [`expressions-nested-list-unqualified`](08-expressions-and-checks.md#812-record-names-and-field-access) | Pinned reference |
| Type-name scopes and bounds | Lexical visibility, no visible shadowing; bounds freeze when definitions execute. | [§6.5](06-types-and-values.md#65-named-types-and-visibility) | [`values-type-forward`](06-types-and-values.md#65-named-types-and-visibility), [`values-type-escape`](06-types-and-values.md#65-named-types-and-visibility), [`values-range-timing`](06-types-and-values.md#65-named-types-and-visibility) | Pinned reference |

## D.8 Matrix layouts

| PLAN.md §6.2 subitem | Decision | Normative section | Executable examples | Reference status |
|---|---|---|---|---|
| `FULL_MATRIX` | Row-major, rectangular, without mirroring. | [§6.8](06-types-and-values.md#68-matrix-layouts) | [`values-layout-full-matrix`](06-types-and-values.md#68-matrix-layouts), [`values-layout-rectangular`](06-types-and-values.md#68-matrix-layouts) | Pinned reference |
| `UPPER_ROW` | Strict upper triangle by row, then mirror. | [§6.8](06-types-and-values.md#68-matrix-layouts) | [`values-layout-upper-row`](06-types-and-values.md#68-matrix-layouts) | Pinned reference |
| `LOWER_ROW` | Strict lower triangle by row, then mirror. | [§6.8](06-types-and-values.md#68-matrix-layouts) | [`values-layout-lower-row`](06-types-and-values.md#68-matrix-layouts) | Pinned reference |
| `UPPER_DIAG_ROW` | Upper triangle including diagonal by row, then mirror. | [§6.8](06-types-and-values.md#68-matrix-layouts) | [`values-layout-upper-diag-row`](06-types-and-values.md#68-matrix-layouts) | Pinned reference |
| `LOWER_DIAG_ROW` | Lower triangle including diagonal by row, then mirror. | [§6.8](06-types-and-values.md#68-matrix-layouts) | [`values-layout-lower-diag-row`](06-types-and-values.md#68-matrix-layouts) | Pinned reference |
| `UPPER_COL` | Strict upper triangle by column, then mirror. | [§6.8](06-types-and-values.md#68-matrix-layouts) | [`values-layout-upper-col`](06-types-and-values.md#68-matrix-layouts) | Pinned reference |
| `LOWER_COL` | Strict lower triangle by column, then mirror. | [§6.8](06-types-and-values.md#68-matrix-layouts) | [`values-layout-lower-col`](06-types-and-values.md#68-matrix-layouts) | Pinned reference |
| `UPPER_DIAG_COL` | Upper triangle including diagonal by column, then mirror. | [§6.8](06-types-and-values.md#68-matrix-layouts) | [`values-layout-upper-diag-col`](06-types-and-values.md#68-matrix-layouts) | Pinned reference |
| `LOWER_DIAG_COL` | Lower triangle including diagonal by column, then mirror. | [§6.8](06-types-and-values.md#68-matrix-layouts) | [`values-layout-lower-diag-col`](06-types-and-values.md#68-matrix-layouts) | Pinned reference |

## D.9 Instances and catalog

| PLAN.md §6.2 subitem | Decision | Normative section | Executable examples | Reference status |
|---|---|---|---|---|
| Ordinal selection and name priority | Complete integer conversion selects a 1-based ordinal before name matching. | [§9.4](09-params-and-instances.md#94-instance-selection) | [`instances-frame-selection`](09-params-and-instances.md#93-instance-blocks-and-frames), [`instances-ordinal-before-name`](09-params-and-instances.md#94-instance-selection) | Pinned reference |
| Name selection, duplicates, and empty names | Exact local derived names; first match wins; unusable names derive the empty string. | [§9.4](09-params-and-instances.md#94-instance-selection) | [`instances-text-selection`](09-params-and-instances.md#94-instance-selection), [`instances-duplicate-names`](09-params-and-instances.md#94-instance-selection), [`instances-empty-name-selection`](09-params-and-instances.md#94-instance-selection) | Pinned reference |
| Retained data, frames, and unread tail | Copy the retained frame into prefix data; explicit selection returns immediately. | [§9.3](09-params-and-instances.md#93-instance-blocks-and-frames), [§9.4](09-params-and-instances.md#94-instance-selection) | [`instances-frame-selection`](09-params-and-instances.md#93-instance-blocks-and-frames), [`instances-retained-frame-visible`](09-params-and-instances.md#94-instance-selection) | Pinned reference |
| Catalog entries, optionality, and order | Return instances, then optional name and failed vectors, in pass order. | [§9.6](09-params-and-instances.md#96-catalog-results) | [`catalog-failure-summaries`](09-params-and-instances.md#96-catalog-results), [`catalog-unusable-name`](09-params-and-instances.md#96-catalog-results), [`catalog-empty-file`](09-params-and-instances.md#96-catalog-results) | Pinned reference |
| Catalog rejection summaries and recovery | Retain exact first-rejection summaries; hard failures and missing-arm EOF abort. | [§9.5](09-params-and-instances.md#95-deferred-failures-in-skipped-instances), [§9.6](09-params-and-instances.md#96-catalog-results) | [`catalog-failure-summaries`](09-params-and-instances.md#96-catalog-results), [`catalog-missing-arm-at-end`](09-params-and-instances.md#96-catalog-results) | Pinned reference |

## D.10 dzn output

| PLAN.md §6.2 subitem | Decision | Normative section | Executable examples | Reference status |
|---|---|---|---|---|
| Names, integers, and entry order | Frozen quoting set, exact signed decimal integers, definition order. | [§B.1](appendix-b-dzn.md#b1-rendering-data-and-operation-boundaries), [§B.2](appendix-b-dzn.md#b2-entry-names-and-integers) | [`dzn-scalars-names-order`](appendix-b-dzn.md#b2-entry-names-and-integers) | Pinned reference |
| Float text | Increasing-precision round-trip search with fixed notation thresholds and exponent spelling. | [§B.3](appendix-b-dzn.md#b3-floating-point-text) | [`dzn-float-spellings`](appendix-b-dzn.md#b3-floating-point-text) | Pinned reference |
| Floats just below 1e17 | Exact starting precision prevents omitted leaf text. | [§B.3](appendix-b-dzn.md#b3-floating-point-text) | [`dzn-float-precision-gap`](appendix-b-dzn.md#b3-floating-point-text) | Iris difference C.4 |
| String escaping | Quote UTF-8 bytes with the specified control escapes. | [§B.4](appendix-b-dzn.md#b4-string-quoting-and-utf-8-bytes) | [`dzn-string-bytes`](appendix-b-dzn.md#b4-string-quoting-and-utf-8-bytes) | Pinned reference |
| Rectangular, empty, ragged, and deep arrays | Choose matrix, arrayNd, or nested one-tuples from resulting shape and depth. | [§B.5](appendix-b-dzn.md#b5-rectangular-arrays-and-empty-dimensions), [§B.6](appendix-b-dzn.md#b6-ragged-and-deeper-arrays) | [`dzn-rectangular-arrays`](appendix-b-dzn.md#b5-rectangular-arrays-and-empty-dimensions), [`dzn-empty-arrays`](appendix-b-dzn.md#b5-rectangular-arrays-and-empty-dimensions), [`dzn-ragged-and-deep`](appendix-b-dzn.md#b6-ragged-and-deeper-arrays) | Pinned reference |
| Catalog, separators, LF, and errors | Render catalog data unchanged; exact assignment separators and LF; render only successful data. | [§B.7](appendix-b-dzn.md#b7-catalog-data-and-executable-examples), [§B.8](appendix-b-dzn.md#b8-separators-line-endings-and-errors) | [`dzn-catalog-summaries`](appendix-b-dzn.md#b7-catalog-data-and-executable-examples), [`dzn-empty-data`](appendix-b-dzn.md#b8-separators-line-endings-and-errors) | Pinned reference |

