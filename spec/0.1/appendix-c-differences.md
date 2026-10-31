# C Differences from Gecode 6.5.0

## C.1 Unicode strings

Iris 0.1 strings MUST be Unicode scalar sequences encoded as UTF-8. The
whole supplied input MUST be valid UTF-8, including comments, skipped lines,
literal pattern text, bytes after a stop or selected instance, and ignored
trailing input. Implementations MUST validate it after parsing/resolution
and unknown-parameter checks, before executing statements. Each `word` or
`rest` value actually read MUST also be valid UTF-8, including discarded
reads and committed captures: byte-wise token settings can split a character
in valid input. Either failure MUST report
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
