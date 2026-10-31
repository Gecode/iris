# C Differences from Gecode 6.5.0

## C.1 Unicode strings

Iris 0.1 strings MUST be Unicode scalar sequences encoded as UTF-8. Reading
a `word` or `rest` value containing invalid UTF-8 MUST fail. This deliberate
difference follows [ADR 0011](../../docs/adr/0011-utf8-strings.md).

The pinned Gecode 6.5.0 reader stores raw bytes: for example, it accepts
`ff0a` (hexadecimal) as a one-byte word followed by LF. Iris rejects that
word. The [NUL example](02-notation.md#25-complete-examples) is accepted by
both: NUL is valid Unicode and canonical JSON escapes it as `\u0000`.

Chapter 4, `04-input.md` (planned; spec-003), will decide whether UTF-8
validation also covers bytes outside string values and how failures are
located. Chapter 11, `errors.md` (planned; spec-002), will assign the error
code. This appendix does not extend validation to all input bytes.
