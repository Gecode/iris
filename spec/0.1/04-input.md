# 4 Input text

This chapter defines input scanning and coordinate counting. Settings are
specified here only by their effect on input; [chapter 5](05-reader-specs.md)
defines their syntax. [Chapter 6](06-types-and-values.md) defines
value-reading statements, and [chapter 7](07-structure.md)
defines line patterns and input structure.

## 4.1 Bytes and physical lines

Implementations MUST scan input as bytes, subject to the
[UTF-8 requirements](#410-utf-8-validity). LF (`0a` in hexadecimal) alone
ends a physical line. CR LF consists of a CR byte followed by the LF that
ends the line. A lone CR does not end a physical line. Implementations
MUST NOT normalize line endings before scanning or counting positions.

A final LF is OPTIONAL, including for a line-pattern match. This input has
CR LF, a lone CR, and no final LF.

```iris example
id: input-line-endings
operation: read
--- spec
a: int b: int c: int
--- input hex
31 0d 0a 32 0d 33
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 0, "value": 1},
  {"name": "b", "type": "int", "depth": 0, "value": 2},
  {"name": "c", "type": "int", "depth": 0, "value": 3}
]}
```

## 4.2 White space and line blanks

The following byte sets MUST be used independently of the host locale.

| Set | Bytes in hexadecimal | Uses |
|---|---|---|
| Default separators | space `20`, tab `09`, LF `0a`, CR `0d`, vertical tab `0b`, form feed `0c` | Ordinary token boundaries and initial skipping |
| Line blanks | space `20`, tab `09`, CR `0d` | Trailing trimming of line text, pattern whitespace, and indentation before comment/stop prefixes |

Vertical tab and form feed separate ordinary words.

```iris example
id: input-ascii-whitespace
operation: read
--- spec
a: word b: word c: word
--- input hex
61 0b 62 0c 63
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "string", "depth": 0, "value": "a"},
  {"name": "b", "type": "string", "depth": 0, "value": "b"},
  {"name": "c", "type": "string", "depth": 0, "value": "c"}
]}
```

A `rest` read MUST first skip separators and applicable comment lines as
in [§4.7](#47-comment-and-stop-prefixes). It MUST then retain the bytes
through the current physical line, excluding LF and trailing line blanks.
Interior CR, vertical tab, and form feed remain in the value. Added
separators affect initial skipping but do not extend trailing trimming.

```iris example
id: input-line-blanks
operation: read
--- spec
x: rest
--- input hex
20 61 6c 70 68 61 0d 62 65 74 61 0b 0c 20 09 0d 0a
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "string", "depth": 0, "value": "alpha\rbeta\u000b\f"}
]}
```

Initial skipping before a line pattern uses separators. Within the line,
pattern whitespace MUST use only line blanks. Vertical tab and form feed
can occur within a captured word but do not satisfy pattern whitespace.
Here vertical tab cannot match the space between `a` and `b`.

```iris example
id: input-pattern-blanks
operation: read
--- spec
line "a b"
--- input hex
61 0b 62 0a
--- expected error
{"code": "E-LINE-MATCH", "message": "expected a line matching \"a b\"", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

Unicode whitespace outside the default byte set MUST be ordinary input
unless a setting changes its bytes. The word below contains U+00A0,
NO-BREAK SPACE, between `a` and `b`.

```iris example
id: input-unicode-space
operation: read
--- spec
x: word
--- input no-final-newline
a b
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "string", "depth": 0, "value": "a b"}
]}
```

## 4.3 Tokens and separators

Without settings, an ordinary token MUST be a maximal nonempty run of bytes
outside the default separator set. A `word` read returns that token.

The `tokens` setting MUST make each listed byte a separate one-byte token,
unless that byte is already a separator. It does not define token strings:
`tokens "::"` makes each colon a token.

```iris example
id: input-token-bytes
operation: read
--- spec
tokens "::" a: word b: word c: word d: word
--- input no-final-newline
a::b
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "string", "depth": 0, "value": "a"},
  {"name": "b", "type": "string", "depth": 0, "value": ":"},
  {"name": "c", "type": "string", "depth": 0, "value": ":"},
  {"name": "d", "type": "string", "depth": 0, "value": "b"}
]}
```

The `separators` setting MUST add each listed byte to the separator set.
Separator status MUST take precedence over token status in either setting
order. A later `tokens` setting MUST NOT remove separator status. Settings
accumulate and affect subsequent execution; they MUST NOT change values
already read.

```iris example
id: input-separators-win
operation: read
--- spec
tokens "," separators "," tokens "," a: int b: int
--- input no-final-newline
1,2
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 0, "value": 1},
  {"name": "b", "type": "int", "depth": 0, "value": 2}
]}
```

```iris example
id: input-settings-order
operation: read
--- spec
a: word tokens ";" b: word p: word c: word
--- input no-final-newline
a;b x;y
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "string", "depth": 0, "value": "a;b"},
  {"name": "b", "type": "string", "depth": 0, "value": "x"},
  {"name": "p", "type": "string", "depth": 0, "value": ";"},
  {"name": "c", "type": "string", "depth": 0, "value": "y"}
]}
```

Settings MUST operate byte by byte, including the bytes of multibyte UTF-8
characters. Resulting word/rest values MUST still satisfy
[§4.10](#410-utf-8-validity).

These settings govern ordinary token boundaries and initial skipping.
They MUST NOT split the interior of a `rest` value or replace the separate
capture rules of line patterns.

```iris example
id: input-line-settings
operation: read
--- spec
tokens ";" separators "," x: rest
--- input no-final-newline
a;b,c
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "string", "depth": 0, "value": "a;b,c"}
]}
```

## 4.4 Integer tokens

An integer token MUST match ASCII `[+-]?[0-9]+` and denote a signed 64-bit
integer in the [range of §3.4](03-data-model.md#34-integers). Conversion
MUST consume the complete token. A leading plus and leading zeros are
accepted; leading zeros do not select octal. Signed zero denotes zero.
These rules apply to scalar, array, and list integer reads.

```iris example
id: input-integer-spellings
operation: read
--- spec
a[6]: int
--- input no-final-newline
+007 -07 -0 0 -9223372036854775808 9223372036854775807
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 1, "value": [7, -7, 0, 0, -9223372036854775808, 9223372036854775807]}
]}
```

Hexadecimal, fractions, exponents, digit separators, bare or doubled signs,
and tokens containing NUL MUST be rejected. A lexical failure or an
out-of-range input integer MUST report
[`E-EXPECTED-VALUE`](errors.md#115-read-time-codes) at the token start.
`E-INTEGER-OVERFLOW` is reserved for arithmetic and value adjustment.

```iris example
id: input-integer-overflow
operation: read
--- spec
x: int
--- input no-final-newline
9223372036854775808
--- expected error
{"code": "E-EXPECTED-VALUE", "message": "expected int, found '9223372036854775808'", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": "x", "record_path": null}
```

```iris example
id: input-integer-token
operation: read
--- spec
x: int
--- input no-final-newline
1e2
--- expected error
{"code": "E-EXPECTED-VALUE", "message": "expected int, found '1e2'", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": "x", "record_path": null}
```

Token settings change the tokens presented to conversion. A digit made a
one-byte token is independently readable as an integer; a sign made a
one-byte token is invalid by itself.

## 4.5 Floating point tokens

A float token MUST match this ASCII decimal form (regular-expression
notation):

```text
[+-]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][+-]?[0-9]+)?
```

Conversion MUST consume the complete token and MUST use a decimal point
of `.` independently of the host locale. Integer-looking tokens, leading
zeros, leading plus, exponents, `1.`, `.5`, `+.5`, and `1.e2` are accepted.
The resulting value type is float even when the value is integral.

```iris example
id: input-float-spellings
operation: read
--- spec
a[7]: float
--- input no-final-newline
+7 -07 1. .5 +.5 1.e2 +2.5E-1
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "float", "depth": 1, "value": [7, -7, 1, 0.5, 0.5, 100, 0.25]}
]}
```

Hexadecimal, infinity, NaN, missing significand or exponent digits, misplaced
signs, and tokens containing NUL MUST be rejected with
[`E-EXPECTED-VALUE`](errors.md#115-read-time-codes) at the token start.

```iris example
id: input-float-hex
operation: read
--- spec
x: float
--- input no-final-newline
0x1p2
--- expected error
{"code": "E-EXPECTED-VALUE", "message": "expected float, found '0x1p2'", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": "x", "record_path": null}
```

```iris example
id: input-float-inf
operation: read
--- spec
x: float
--- input no-final-newline
inf
--- expected error
{"code": "E-EXPECTED-VALUE", "message": "expected float, found 'inf'", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": "x", "record_path": null}
```

```iris example
id: input-float-nan
operation: read
--- spec
x: float
--- input no-final-newline
nan
--- expected error
{"code": "E-EXPECTED-VALUE", "message": "expected float, found 'nan'", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": "x", "record_path": null}
```

The exact decimal value MUST be rounded to binary64 using round to nearest,
ties to even. Overflow to infinity MUST report `E-EXPECTED-VALUE` at the
token start.

```iris example
id: input-float-overflow
operation: read
--- spec
x: float
--- input no-final-newline
1e309
--- expected error
{"code": "E-EXPECTED-VALUE", "message": "expected float, found '1e309'", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": "x", "record_path": null}
```

If the exact decimal value is nonzero but rounds to a subnormal value or
to zero, conversion MUST report `E-EXPECTED-VALUE` at the token start.

```iris example
id: input-float-underflow
operation: read
--- spec
x: float
--- input no-final-newline
5e-324
--- expected error
{"code": "E-EXPECTED-VALUE", "message": "expected float, found '5e-324'", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": "x", "record_path": null}
```

Exact zero in any spelling, including `0e-9999`, MUST be accepted. Values
that round to the smallest normal binary64 value MUST be accepted, even
when their exact decimal magnitude is slightly smaller than that value.

```iris example
id: input-float-normal-boundary
operation: read
--- spec
a[2]: float
--- input no-final-newline
0e-9999 2.2250738585072013e-308
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "float", "depth": 1, "value": [0, 2.2250738585072014e-308]}
]}
```

The `num` reading type MUST try integer conversion before float conversion.
For example, `9223372036854775808` fails integer conversion but succeeds
as a float through `num`. Value construction, promotion, and signed zero
follow [§3.5](03-data-model.md#35-floating-point-values).

## 4.6 Input positions

Input lines and columns MUST start at 1. A byte's line is one plus the
number of preceding LF bytes. Its column is one plus the number of bytes
since the preceding LF, or since the start of input if none precedes it.
Tabs, CR, vertical tab, and form feed each occupy one column. Each byte of
a multibyte UTF-8 character also occupies one column; columns count neither
Unicode scalar values nor grapheme clusters.

The first word below occupies two bytes, and the tab occupies one, so the
invalid integer starts at column 4.

```iris example
id: input-byte-columns
operation: read
--- spec
a: word b: int
--- input no-final-newline
é	bad
--- expected error
{"code": "E-EXPECTED-VALUE", "message": "expected int, found 'bad'", "input_location": {"origin": "input.bin", "line": 1, "column": 4}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": "b", "record_path": null}
```

A lone CR increments the column. CR LF increments the line only at LF.

```iris example
id: input-cr-columns
operation: read
--- spec
a: int b: int
--- input hex
31 0d 62 61 64
--- expected error
{"code": "E-EXPECTED-VALUE", "message": "expected int, found 'bad'", "input_location": {"origin": "input.bin", "line": 1, "column": 3}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": "b", "record_path": null}
```

```iris example
id: input-crlf-columns
operation: read
--- spec
a: int b: int
--- input hex
31 0d 0a 09 62 61 64
--- expected error
{"code": "E-EXPECTED-VALUE", "message": "expected int, found 'bad'", "input_location": {"origin": "input.bin", "line": 2, "column": 2}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": "b", "record_path": null}
```

End of input has the position immediately after the final byte: after LF,
column 1 of the next line; otherwise, one beyond the final byte's column.

```iris example
id: input-eof-columns
operation: read
--- spec
a: int b: int
--- input hex
31 0d 0a
--- expected error
{"code": "E-EXPECTED-VALUE", "message": "expected int, found end of input", "input_location": {"origin": "input.bin", "line": 2, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": "b", "record_path": null}
```

A stop prefix advances the reading position to the physical end of all
supplied bytes. A later required read can therefore report end of input
beyond the stop line. [Chapter 11](errors.md#111-reporting-and-comparison)
defines which current or saved position belongs to each error; an
implementation MUST NOT relocate saved check positions to the final cursor.

## 4.7 Comment and stop prefixes

Whenever normal skipping is required, implementations MUST first skip
separator bytes. Prefixes MUST then be considered only if every byte before
the current position on its physical line is a line blank. A matching
comment prefix discards the remainder of that physical line and resumes
skipping. A matching stop prefix discards all remaining input. Inline
occurrences are ordinary data. If comment and stop prefixes both match,
comment handling MUST take precedence.

Comparison MUST be case-sensitive and byte-exact. Empty prefixes MUST NOT
match. The boundary class is ASCII `[A-Za-z0-9_]`. If the final prefix byte
belongs to that class, the following byte MUST be absent or outside that
class for the prefix to match. Otherwise there is no boundary restriction.
Non-ASCII letters do not belong to the class.

Thus `END` does not match `ENDx`, `END7`, or `END_`, but does match `ENDé`.

```iris example
id: input-comment-boundaries
operation: read
--- spec
comment "END" repeat until eof { x: word }
--- input no-final-newline
ENDx
END7
END_
ENDé
keep
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "string", "depth": 1, "value": ["ENDx", "END7", "END_", "keep"]}
]}
```

An underscore at the end of a prefix still requires a boundary. A
non-ASCII final byte does not: prefix `é` matches `éx`.

```iris example
id: input-prefix-underscore
operation: read
--- spec
comment "END_" repeat until eof { x: word }
--- input no-final-newline
END_x
keep
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "string", "depth": 1, "value": ["END_x", "keep"]}
]}
```

```iris example
id: input-prefix-nonascii
operation: read
--- spec
comment "é" repeat until eof { x: word }
--- input no-final-newline
éx
keep
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "string", "depth": 1, "value": ["keep"]}
]}
```

Space, tab, and CR indentation permit recognition. A preceding vertical
tab, form feed, or added comma separator does not: even after skipping,
that byte is not a line blank. A lone CR after data does not start a new
physical line. Here the first line is a comment, but the second is data.

```iris example
id: input-prefix-indentation
operation: read
--- spec
comment "#" repeat until eof { x: word }
--- input hex
20 09 0d 23 73 6b 69 70 0a 0b 23 6b 65 65 70 0a
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "string", "depth": 1, "value": ["#keep"]}
]}
```

Stops use the same boundary rule. Here `ENDé` stops reading before `later`.

```iris example
id: input-stop-boundaries
operation: read
--- spec
stop "END" repeat until eof { x: word }
--- input no-final-newline
ENDx
END_
ENDé
later
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "string", "depth": 1, "value": ["ENDx", "END_"]}
]}
```

When both settings match `END`, the comment consumes only that line.

```iris example
id: input-comment-before-stop
operation: read
--- spec
comment "END" stop "END" repeat until eof { x: word }
--- input no-final-newline
END
keep
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "string", "depth": 1, "value": ["keep"]}
]}
```

## 4.8 End of input and trailing input

After successful execution, implementations MUST perform normal skipping
and report [`E-TRAILING-INPUT`](errors.md#115-read-time-codes) if data
remains. Its position is the first remaining data byte. Separators,
comments, and stops therefore apply during this check.

```iris example
id: input-trailing
operation: read
--- spec
x: int
--- input no-final-newline
7
 junk
--- expected error
{"code": "E-TRAILING-INPUT", "message": "unexpected input after the end of the spec: 'junk'", "input_location": {"origin": "input.bin", "line": 2, "column": 2}, "spec_location": null, "context": null, "record_path": null}
```

The `ignore trailing` setting MUST suppress this final check. It MUST NOT
suppress earlier reading errors or make an absent required value readable.
Explicit instance selection also bypasses the final trailing check, as
specified in [§11.2.1](errors.md#1121-deferred-instance-failures-and-catalog).
Neither mechanism bypasses input-wide [UTF-8 validation](#410-utf-8-validity).

```iris example
id: input-ignore-trailing
operation: read
--- spec
x: int ignore trailing
--- input no-final-newline
7
 junk
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "int", "depth": 0, "value": 7}
]}
```

## 4.9 NUL bytes

NUL (`00`) MUST be treated as a byte, not end of input or a string
terminator. It is neither a default separator nor a line ending. By default,
ordinary word/rest reads MUST retain it; numeric conversion MUST reject a token
containing it. It can be configured as a separator. NUL is valid Unicode;
canonical JSON escapes it as `\u0000`.

```iris example
id: input-nul
operation: read
--- spec
x: word
--- input hex
00 0a
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "string", "depth": 0, "value": "\u0000"}
]}
```

Line-pattern captures have a separate boundary rule. A word capture without
an immediately following literal MUST stop at NUL: `line "{x:word}"` fails
to match `a`, NUL, `b`. A word capture immediately followed by a literal
uses that literal's first byte as its stopping byte instead. A rest capture
retains NUL. [§7.2](07-structure.md#72-line-patterns-and-captures) defines
the full pattern rules.

## 4.10 UTF-8 validity

The entire supplied input byte sequence MUST be valid UTF-8. This includes
comments, skipped lines, literal portions of pattern matches, bytes after
a stop prefix, bytes after a selected instance, and ignored trailing input.
This deliberate [difference from Gecode 6.5.0](appendix-c-differences.md#c1-unicode-strings)
also applies to catalog operations.

Implementations MUST validate the input as one phase after parsing and
resolving the reader spec and rejecting unknown parameters, and before any
statement executes. An input-wide failure MUST report
[`E-UTF8`](errors.md#116-iris-only-errors) at the first byte of the first
ill-formed subsequence, using [§4.6](#46-input-positions). It MUST have null
`spec_location`, `context`, and `record_path`
([profile T](errors.md#1132-location-profiles)). Thus parse/resolve
errors precede unknown parameters, which precede input-wide `E-UTF8`, which
precedes execution errors. A parse-only operation does not read or validate
input.

Iris-only UTF-8 requirement; Gecode accepts these bytes. The invalid byte
is in a comment that execution would otherwise skip.

```iris example
id: input-invalid-utf8-comment
operation: read
--- spec
comment "#" x: int
--- input hex
23 ff 0a 37
--- expected error
{"code": "E-UTF8", "message": "input is not valid UTF-8", "input_location": {"origin": "input.bin", "line": 1, "column": 2}, "spec_location": null, "context": null, "record_path": null}
```

Iris-only UTF-8 requirement; Gecode accepts these bytes. The invalid byte
follows a valid integer in an ignored tail. Validation fails before `x`
is read.

```iris example
id: input-invalid-utf8-tail
operation: read
--- spec
x: int ignore trailing
--- input hex
37 0a ff
--- expected error
{"code": "E-UTF8", "message": "input is not valid UTF-8", "input_location": {"origin": "input.bin", "line": 2, "column": 1}, "spec_location": null, "context": null, "record_path": null}
```

Each word/rest value actually read MUST also be valid UTF-8, because
byte-wise `tokens` and `separators` settings can split a character in valid
input. This check includes scalar, array, and list reads, discarded reads,
and reads in instances traversed without retaining their data. Captures
MUST be checked after a complete pattern match is selected, before committing
its values, including unnamed or discarded captures. Speculative captures
of unsuccessful candidate matches MUST NOT introduce encoding errors.

A value-level failure is a hard reading error, ordered with execution
errors by [§11.2](errors.md#112-error-order). It MUST report `E-UTF8` at
the input byte starting the first ill-formed subsequence within the value,
with profile R availability: active reader-spec origin and line, null
reader-spec column, current execution context when present, and null
`record_path`. These locations are defined by Iris, not supplied by Gecode.

Iris-only UTF-8 requirement; Gecode accepts these bytes. The complete input
is valid UTF-8, but `tokens "é"` makes each of its two bytes a separate
word. The first word is the truncated byte `c3`.

```iris example
id: input-invalid-utf8-split
operation: read
--- spec
tokens "é" a: word b: word
--- input no-final-newline
é
--- expected error
{"code": "E-UTF8", "message": "string value is not valid UTF-8", "input_location": {"origin": "input.bin", "line": 1, "column": 1}, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": "a", "record_path": null}
```

Both checks MUST reject malformed continuation sequences, truncated
sequences, overlong encodings, surrogate encodings, and encodings above
U+10FFFF. For a truncated sequence, the error position is its initial byte.
NUL, noncharacters, and U+FEFF MUST be accepted as ordinary characters.
Implementations MUST NOT strip a BOM or normalize Unicode. These rules
concern input and read values; [chapter 5](05-reader-specs.md) defines
reader-spec source validation and lexing.

```iris example
id: input-valid-utf8
operation: read
--- spec
a: word b: rest
--- input
é café
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "string", "depth": 0, "value": "é"},
  {"name": "b", "type": "string", "depth": 0, "value": "café"}
]}
```
