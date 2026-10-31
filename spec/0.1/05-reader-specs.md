# 5 Reader specs

This chapter defines reader-spec source, lexical structure, syntax, placement,
format metadata, and parser limits. The formal productions are in
[grammar.ebnf](grammar.ebnf); [appendix A](appendix-a-grammar.md) explains
its layers and named predicates. [Chapter 6](06-types-and-values.md)
defines types and value reads.
Further execution rules belong to [chapter 7](07-structure.md)
(input structure and patterns), chapter 8 (`08-expressions-and-checks.md`,
planned: evaluation, checks, and record names), and chapter 9
(`09-params-and-instances.md`, planned: parameters, instances, and catalog).
Chapter 10 (`10-work-budgets.md`, planned) defines work budgets.

## 5.1 Reader specs and parsing

A reader spec MAY be empty. Parsing MUST proceed in this order:

1. Validate the entire source byte sequence as UTF-8.
2. Lex the complete source into tokens.
3. Parse statements, including embedded line patterns and their types.
4. Check global name conflicts, then resolve names.

Source validation MUST use strict UTF-8: reject malformed continuation
sequences, truncated sequences, overlong encodings, surrogate encodings,
and encodings above U+10FFFF. Implementations MUST NOT normalize Unicode
or strip a BOM. NUL, noncharacters, and U+FEFF are valid scalar values;
the lexical rules still restrict where they can occur.

Invalid source MUST report [`E-SPEC-UTF8`](errors.md#116-iris-only-errors)
at the first byte of the first ill-formed subsequence. Its reader-spec
line is one plus the number of preceding LF bytes; its column is one plus
the number of bytes since the preceding LF, or since the start of source.
Tabs and each byte of a multibyte character each occupy one column. The
error MUST have null input location, context, and record path. This
Iris-defined position also applies to truncated sequences, whose initial
byte is the error position.

This check precedes every other parse-time error, even an earlier lexical
error. The requirement is a deliberate
[difference from Gecode 6.5.0](appendix-c-differences.md#c2-reader-spec-source).
Input validation is separate; see [§4.10](04-input.md#410-utf-8-validity).

Iris-only UTF-8 requirement; Gecode accepts these bytes. The invalid byte
is inside a reader-spec string.

```iris example
id: reader-invalid-utf8-string
operation: parse
--- spec hex
6c 65 74 20 73 3d 22 ff 22
--- expected error
{"code": "E-SPEC-UTF8", "message": "reader spec is not valid UTF-8", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": 8}, "context": null, "record_path": null}
```

Iris-only UTF-8 requirement; Gecode accepts these bytes. The invalid byte
is in a comment on line 2, at byte column 4.

```iris example
id: reader-invalid-utf8-comment
operation: parse
--- spec hex
78 3a 69 6e 74 0a 23 20 20 ff
--- expected error
{"code": "E-SPEC-UTF8", "message": "reader spec is not valid UTF-8", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 2, "column": 4}, "context": null, "record_path": null}
```

After source validation, complete-source lexing MUST precede statement
parsing. A bad string at the end can therefore precede an unknown statement
at the start. Pattern strings are compiled during statement parsing, not
during source lexing. Syntactic recognition alone does not guarantee a
successful parse: name conflicts, unknown functions, type requirements,
and record-scope errors are also parse-time failures.
[§11.2](errors.md#112-error-order) defines error order. Except for
`E-SPEC-UTF8`, reader-spec parse errors report a line and a null column.

## 5.2 White space and comments

Reader-spec whitespace MUST consist of these ASCII bytes: space (`20`),
tab (`09`), LF (`0a`), CR (`0d`), vertical tab (`0b`), and form feed (`0c`),
in hexadecimal. Only LF increments the reader-spec line number. A final LF
is OPTIONAL; a lone CR does not end a physical line.

Outside a quoted string, `#` MUST begin a comment through the next LF or
end of source. It needs no preceding whitespace. CR does not end a comment.
Whitespace and comments MUST separate tokens without producing tokens.
They do not separate statements or stop expressions.

Every non-ASCII character outside a string or comment MUST report
[`E-SPEC-CHARACTER`](errors.md#114-parse-time-codes). This includes a valid
UTF-8 BOM at the start of source. Ill-formed UTF-8 anywhere instead fails
the earlier source-validation phase. An unrecognized ASCII character also
reports `E-SPEC-CHARACTER`, except that a literal NUL is tokenized as
punctuation with no syntactic role and consequently gives `E-SPEC-SYNTAX`.
Semicolons are not statement separators; `//`
is two division tokens, not a comment.

The lexer MUST choose the longest punctuation token starting at its
position. The complete punctuation inventory is in `grammar.ebnf`.
An identifier and a number need not be separated by whitespace:
`let x=1e2foo:int` consists of a float expression followed by a declaration
named `foo`.

## 5.3 Strings

A string MUST start and end with double quotes. Its contents MAY include
any Unicode scalar value except an unescaped double quote, backslash, or
physical LF. A physical LF, even immediately after backslash, or end of
source before the closing quote MUST report
[`E-SPEC-STRING`](errors.md#114-parse-time-codes).

Exactly four escapes MUST be recognized. They yield the named characters:

| Source escape | Character |
|---|---|
| `\n` | LF, U+000A |
| `\t` | Tab, U+0009 |
| `\\` | Backslash, U+005C |
| `\"` | Double quote, U+0022 |

Every other escape MUST report `E-SPEC-STRING`. In particular, there are
no `\r`, `\0`, hexadecimal, Unicode, or escaped-slash forms. Literal CR
and NUL are permitted; their example transport requires a `hex` spec part
under [§2.4](02-notation.md#24-byte-parts-and-splitting-algorithm).
Escaped LF is string content and does not increment the source line count.

```iris example
id: reader-comments-and-strings
operation: read
--- spec no-final-newline
let s = "a\nb\tc\\d\"e" # comment
n: int
--- input no-final-newline
7
--- expected data
{"iris": "0.1", "entries": [
  {"name": "s", "type": "string", "depth": 0, "value": "a\nb\tc\\d\"e"},
  {"name": "n", "type": "int", "depth": 0, "value": 7}
]}
```

```iris example
id: reader-invalid-escape
operation: parse
--- spec no-final-newline
let s="\r"
--- expected error
{"code": "E-SPEC-STRING", "message": "unknown escape '\\r' in string (use \\n, \\t, \\\\, or \\\")", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

This complete source uses all six whitespace bytes, literal CR and NUL in
its string, and a comment containing CR. Only LF ends the comment.

```iris example
id: reader-source-bytes
operation: read
--- spec hex
09 0d 0b 0c 6c 65 74 20 73 3d 22 61 0d 00 62 22 23 20 63 6f 6d 6d 65 6e 74 0d 73 74 69 6c 6c 20 63 6f 6d 6d 65 6e 74 0a 6e 3a 69 6e 74
--- input no-final-newline
7
--- expected data
{"iris": "0.1", "entries": [
  {"name": "s", "type": "string", "depth": 0, "value": "a\r\u0000b"},
  {"name": "n", "type": "int", "depth": 0, "value": 7}
]}
```

## 5.4 Names

An ordinary name MUST match ASCII `[A-Za-z_][A-Za-z0-9_]*`. Names are
case-sensitive. Keywords are contextual: statement and option words MAY
be declaration names, subject to the top-level `format` exception in
[§5.9](#59-format-metadata).

```iris example
id: reader-keyword-declarations
operation: read
--- spec no-final-newline
line: int type: 0.. list: word each: int layout: int
--- input no-final-newline
1 2 a 3 4
--- expected data
{"iris": "0.1", "entries": [
  {"name": "line", "type": "int", "depth": 0, "value": 1},
  {"name": "type", "type": "int", "depth": 0, "value": 2},
  {"name": "list", "type": "string", "depth": 0, "value": "a"},
  {"name": "each", "type": "int", "depth": 0, "value": 3},
  {"name": "layout", "type": "int", "depth": 0, "value": 4}
]}
```

Dots are postfix field access, not part of an ordinary definition name.
Whitespace and comments MAY surround dots and brackets in expressions.
For example, `xs [0] . a` selects the same field as `xs[0].a`.

A glued `gecode.` followed by one or more ASCII letters, digits, or
underscores MUST instead form one identifier token. Its suffix MAY begin
with a digit. Thus `gecode.int_max` is one token, while `gecode . int_max`
is three tokens and uses field syntax. `gecode` alone is an ordinary name.
Definitions using the `gecode.` prefix MUST report
[`E-SPEC-NAME`](errors.md#114-parse-time-codes). This applies to declarations,
`let`, `param`, `type`, lists, `as`, `each`, `store`, and comprehension
bindings. The two known host constants and errors for unknown constants
are defined in chapter 8, `08-expressions-and-checks.md` (planned).
Pattern names use the ordinary-name form, not this special token form.

`_` MUST be accepted as a declaration, list, or capture name, and in
`as`, `each`, and `store` positions. It MUST NOT be accepted as the defined
name of `let`, `param`, or `type`, or as a comprehension binding; these
fail with `E-SPEC-NAME`. It is not universally discarded: `store _`
stores an entry named `_`. [§6.3](06-types-and-values.md#63-names-discarded-reads-and-replacement)
defines declaration and list discards.
[§7.2](07-structure.md#72-line-patterns-and-captures) defines capture discards;
[§7.6](07-structure.md#76-logical-records-and-names) defines record structure.
Chapter 8, `08-expressions-and-checks.md` (planned), defines record-name
resolution and use in expressions.

Reserved/discard definition checks occur after consuming the name. Their
reported line MAY therefore be the following token's line; implementations
MUST preserve that position rather than substitute the name's start.

```iris example
id: reader-record-fields
operation: read
--- spec no-final-newline
repeat 1 as xs {each x a:int} check x.a==xs[0].a let first=xs[0].a
--- input no-final-newline
4
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 1, "value": [4]},
  {"name": "first", "type": "int", "depth": 0, "value": 4}
]}
```

## 5.5 Numbers

A numeric literal token MUST start with an ASCII digit. Unary minus is a
separate operator; unary plus does not exist. Consequently `.5` and `+1`
are not expression literals, although input conversion accepts those
spellings under [§§4.4–4.5](04-input.md#44-integer-tokens).

Starting at a digit, the lexer MUST scan digits, dots, `e`, `E`, and a sign
only immediately after `e` or `E`. It MUST truncate this candidate before
its first `..`, if any, then validate and convert the entire remaining
candidate. Thus `0..9` contains a range operator; `1.2.3` is one invalid
number, not several tokens. Failure MUST report
[`E-SPEC-NUMBER`](errors.md#114-parse-time-codes).

An all-digit candidate MUST be an integer token from 0 through
9223372036854775807 inclusive. Leading zeros are decimal. The source
`-9223372036854775808` fails because its positive literal token overflows
before unary negation. The minimum integer can be constructed by
`-9223372036854775807-1`.

Every other candidate MUST be a decimal float: digits followed by a decimal
point and optional fractional digits, an exponent, or both. An exponent
has `e` or `E`, an optional sign, and one or more digits. Accepted spellings
include `1.`, `1.e2`, and `01e+2`. Conversion MUST obey the binary64
rounding, overflow, and underflow rules in
[§4.5](04-input.md#45-floating-point-tokens), with `E-SPEC-NUMBER` on failure.
There are no hexadecimal, infinity, or NaN literals.

```iris example
id: reader-number-spellings
operation: read
--- spec no-final-newline
let i=007 let f=1.e2 let lo=-9223372036854775807-1
--- input no-final-newline
--- expected data
{"iris": "0.1", "entries": [
  {"name": "i", "type": "int", "depth": 0, "value": 7},
  {"name": "f", "type": "float", "depth": 0, "value": 100},
  {"name": "lo", "type": "int", "depth": 0, "value": -9223372036854775808}
]}
```

```iris example
id: reader-number-overflow
operation: parse
--- spec no-final-newline
let x=-9223372036854775808
--- expected error
{"code": "E-SPEC-NUMBER", "message": "bad number 9223372036854775808", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

## 5.6 Statements, declarations, and blocks

Statements MUST be sequenced without separators. LF is whitespace, so
an expression MUST continue across it when an operator follows.

```iris example
id: reader-statement-sequencing
operation: read
--- spec no-final-newline
let x=1
+2 y:int
--- input no-final-newline
4
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "int", "depth": 0, "value": 3},
  {"name": "y", "type": "int", "depth": 0, "value": 4}
]}
```

At a statement position, the parser MUST first test for a declaration:
an identifier, zero or more balanced square-bracket token groups, then a
colon. Only punctuation tokens count as brackets; brackets inside strings
do not. If this lookahead succeeds, the statement MUST be parsed as a
declaration, even when its name is a keyword. Otherwise its first identifier
MUST name a statement. Unknown statements and missing syntax report
[`E-SPEC-SYNTAX`](errors.md#114-parse-time-codes).

The statement forms and their semantic chapters are:

| Forms | Defining chapter |
|---|---|
| Declarations, `list`, `type` | [6, types and value reads](06-types-and-values.md) |
| `expect`, `line`, `find`, `skip`, `index`, `repeat`, `each`, `sections`, `if` | [7, input structure](07-structure.md) |
| `let` | [§7.9, execution](07-structure.md#79-conditional-execution-and-let); chapter 8, `08-expressions-and-checks.md` (planned): expressions |
| `check` | 8, `08-expressions-and-checks.md` (planned): expressions and checks |
| `param`, `instances` | 9, `09-params-and-instances.md` (planned): parameters and instances |

A block MUST be a brace-enclosed sequence of statements. It MAY be empty.
Blocks occur as repeat and instances bodies, if/else bodies, and optional
sections-arm bodies. Standalone blocks are not statements. The outer
braces of `sections` enclose arms, each beginning with a pattern string;
that outer enclosure is not an ordinary block.

After `else`, either an ordinary block or a token spelled `if` MUST follow.
In the latter case, declaration-first dispatch still applies. Accordingly,
`else if:int` and `else if[1]:int` are permitted declarations, as well as
ordinary else-if chains.

```iris example
id: reader-else-declaration
operation: read
--- spec no-final-newline
if 0 {} else if:int
--- input no-final-newline
7
--- expected data
{"iris": "0.1", "entries": [
  {"name": "if", "type": "int", "depth": 0, "value": 7}
]}
```

```iris example
id: reader-missing-brace
operation: parse
--- spec no-final-newline
repeat 2 {x:int
--- expected error
{"code": "E-SPEC-SYNTAX", "message": "missing '}'", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

## 5.7 Types, ranges, and options

The grammar MUST permit the base types `int`, `float`, `num`, `word`, and
`rest`; `alphabet` followed by a string and options; visible named types;
and ranges. At a type position, a base-type name MUST take priority, then
`alphabet`, then a visible named type, then a range expression. Parentheses
can disambiguate a range bound that has the name of a type.
Type visibility and type compatibility follow
[§6.1](06-types-and-values.md#61-reading-types-and-resulting-entries) and
[§6.5](06-types-and-values.md#65-named-types-and-visibility), rather
than unrestricted alternatives in this dispatch.

An inclusive range MAY omit either bound, but MUST contain at least one.
An upper-exclusive range `..<` MAY omit its lower bound and MUST contain
its upper bound. Missing required bounds report `E-SPEC-SYNTAX`.

After the range operator, an upper bound MUST begin only when the next
token satisfies this test:

- An integer, float, or string token begins an upper bound.
- Punctuation begins one only for `(`, `-`, or `!`.
- An identifier begins one unless declaration lookahead succeeds or it is
  in the stop set below.
- End of source does not begin one.

The exact identifier stop set is:

```text
comment stop expect tokens separators ignore param list line find skip
index repeat each instances sections if let check type layout offset until
```

Parentheses permit keyword-named upper bounds, as in `0..(check)`.
`format` MUST NOT stop a range: `0..format` uses it as the bound. A following
format declaration may therefore be consumed as part of range parsing.

```iris example
id: reader-open-ranges
operation: read
--- spec no-final-newline
let check=7 a:0.. b:..3 c[1][1]:0.. layout "FULL_MATRIX" offset -1 list d:0.. until 0 e:0..(check) f:0.. check f>0
--- input no-final-newline
1 2 3 4 0 5 6
--- expected data
{"iris": "0.1", "entries": [
  {"name": "check", "type": "int", "depth": 0, "value": 7},
  {"name": "a", "type": "int", "depth": 0, "value": 1},
  {"name": "b", "type": "int", "depth": 0, "value": 2},
  {"name": "c", "type": "int", "depth": 2, "value": [[2]]},
  {"name": "d", "type": "int", "depth": 1, "value": [4]},
  {"name": "e", "type": "int", "depth": 0, "value": 5},
  {"name": "f", "type": "int", "depth": 0, "value": 6}
]}
```

```iris example
id: reader-range-missing-bound
operation: parse
--- spec no-final-newline
x:0..<
--- expected error
{"code": "E-SPEC-SYNTAX", "message": "'..<' needs an upper bound", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

A declaration MAY have `layout` followed by an expression, then `offset`
followed by an expression, each at most once and in that order. A list MAY
have `offset` after its terminator. An alphabet MAY have any number of
`blank` string, `strip` string, and `base` integer options, in any order,
including repetitions. An alphabet base permits an optional minus followed
by one integer literal token, rather than an arbitrary expression.
Repeated alphabet options MUST replace the preceding value of that option.

Before consuming `layout`, `offset`, `blank`, `strip`, or `base` as an
option, the parser MUST check that it is not a declaration start. A
same-named declaration ends the preceding option list.

```iris example
id: reader-option-names
operation: read
--- spec no-final-newline
g[1][1]:alphabet "ab" base:int x:int offset:int
--- input no-final-newline
a 7 8 9
--- expected data
{"iris": "0.1", "entries": [
  {"name": "g", "type": "int", "depth": 2, "value": [[1]]},
  {"name": "base", "type": "int", "depth": 0, "value": 7},
  {"name": "x", "type": "int", "depth": 0, "value": 8},
  {"name": "offset", "type": "int", "depth": 0, "value": 9}
]}
```

Sections options MAY repeat and interleave: `until` followed by one or
more strings and optional `store` name, and `skip other`. Terminator strings
MUST accumulate, a later store name MUST replace the previous store name,
and `skip other` remains enabled once present. Arm modifiers MUST occur
as optional `optional`, then optional `last`, then an optional body.
[§7.8](07-structure.md#78-sections) defines their matching and execution
effects.
[Chapter 6](06-types-and-values.md) defines range evaluation, layouts,
offsets, alphabets, and lists.

## 5.8 Settings

`comment` and `stop` MUST be followed by one or more strings. `tokens` and
`separators` MUST each be followed by exactly one string. `ignore` MUST be
followed by `trailing`. These settings MAY repeat and MAY appear among
top-level statements before `instances`; they need not form a header.
They MUST NOT appear inside blocks, even on an unreachable execution path.
The error is [`E-SPEC-PLACEMENT`](errors.md#114-parse-time-codes).
Declaration lookahead still allows a declaration named `comment`, for
example, inside a block.

Settings MUST activate in statement execution order. Comment and stop
prefix lists, token bytes, and separator bytes MUST accumulate across
repeated settings. `ignore trailing` remains enabled once executed.
Their input effects are defined in
[§4.3](04-input.md#43-tokens-and-separators),
[§4.7](04-input.md#47-comment-and-stop-prefixes), and
[§4.8](04-input.md#48-end-of-input-and-trailing-input).

```iris example
id: reader-settings-in-order
operation: read
--- spec no-final-newline
a:word comment "#" b:word comment "A" comment "B" stop "END" tokens "!" tokens "?" separators "," separators ";" c:int d:int expect "!" "?" ignore trailing
--- input no-final-newline
before
# skip
after
A skip
B skip
1,2;!? extra
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "string", "depth": 0, "value": "before"},
  {"name": "b", "type": "string", "depth": 0, "value": "after"},
  {"name": "c", "type": "int", "depth": 0, "value": 1},
  {"name": "d", "type": "int", "depth": 0, "value": 2}
]}
```

```iris example
id: reader-setting-in-block
operation: parse
--- spec no-final-newline
if 0 {comment "#"}
--- expected error
{"code": "E-SPEC-PLACEMENT", "message": "the setting 'comment' must be at the top level of the spec", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

## 5.9 Format metadata

At the top level, `format` MUST be handled before statement declaration
lookahead. It MUST be followed by an identifier or string, whose text is
the format name. Subsequent `-` or `.` tokens MUST be joined to that name
only when both the joiner and its following identifier or integer token
are glued to their predecessors, with no whitespace or comment between.
All such joins MUST be consumed. A float token is not a join component;
`format iris-1.0` fails, while `format "iris-1.0"` succeeds.

An empty quoted name is permitted. Each format declaration MUST replace
the preceding name; the final name is metadata, not a data entry or an
extra member of [canonical JSON](03-data-model.md#33-canonical-data-document).
Format declarations MAY appear after `instances`, including repeatedly,
because they are not executable statements.

Top-level `format:int` MUST fail with `E-SPEC-SYNTAX`: the colon is not a
format name. Inside a block, `format:int` is an ordinary declaration, while
`format "name"` fails as an unknown statement with `E-SPEC-SYNTAX`.

The final format name in this example is `last name`; the data contains
only `x`.

```iris example
id: reader-format-metadata
operation: read
--- spec no-final-newline
format dimacs-cnf.v2 instances {x:int} format "last name"
--- input no-final-newline
7
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "int", "depth": 0, "value": 7}
]}
```

```iris example
id: reader-format-keyword
operation: parse
--- spec no-final-newline
format:int
--- expected error
{"code": "E-SPEC-SYNTAX", "message": "expected a format name, found ':'", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

## 5.10 Expression syntax and precedence

Expressions MUST permit integer, float, and string literals; names and
host constants; parenthesized expressions; ordinary function calls;
comprehensions; and repeated postfix indexing or field selection.

An ordinary call has an identifier, `(`, zero or more comma-separated
expressions, and `)`. A host-constant token cannot be called. For callees
`all`, `any`, `sum`, or `count`, an identifier followed by the identifier
`in` immediately after `(` MUST instead select comprehension syntax:
a binding name, `in`, an expression, `:`, another expression, and `)`.
This is token lookahead; intervening whitespace and comments are permitted.
Function names, arities, bindings, and evaluation are defined in chapter 8,
`08-expressions-and-checks.md` (planned).

Implementations MUST use this precedence, from loosest to tightest:

| Level | Operators/forms | Association |
|---|---|---|
| 1 | `\|\|` | Left |
| 2 | `&&` | Left |
| 3 | `==`, `!=` | Left |
| 4 | `<`, `>`, `<=`, `>=` | Left |
| 5 | `+`, `-` | Left |
| 6 | `*`, `/`, `%` | Left |
| 7 | Unary `-`, `!` | Prefix |
| 8 | `^` | Right; right operand is a unary expression |
| 9 | `[expression]`, `.name` | Left, repeated suffixes |
| 10 | Literals, names, calls, comprehensions, parenthesized expressions | Primary |

Thus `-2^2` groups as `-(2^2)`, `2^3^2` as `2^(3^2)`, and `2^-1`
as `2^(-1)`. Parsing the last form succeeds; chapter 8 defines its
runtime exponent error. Chained comparisons group left, rather than
introducing a special comparison-chain form.

```iris example
id: reader-precedence
operation: read
--- spec no-final-newline
let a=2*3^2 let b=2^3^2 let c=-2^2 let d=8/2/2 let e=1<2<3 let f=1||0&&0
--- input no-final-newline
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 0, "value": 18},
  {"name": "b", "type": "int", "depth": 0, "value": 512},
  {"name": "c", "type": "int", "depth": 0, "value": -4},
  {"name": "d", "type": "int", "depth": 0, "value": 2},
  {"name": "e", "type": "int", "depth": 0, "value": 1},
  {"name": "f", "type": "int", "depth": 0, "value": 1}
]}
```

## 5.11 Placement restrictions

These restrictions MUST be checked during parsing, including in bodies
that execution would not reach. They apply to actual statements/settings,
not declarations with the same contextual keyword as their name.

| Form | Permitted position; failure |
|---|---|
| `comment`, `stop`, `tokens`, `separators`, `ignore trailing` | Top level before `instances`; nested use gives `E-SPEC-PLACEMENT`. |
| `format` metadata | Top level anywhere, including after `instances`; nested metadata syntax gives `E-SPEC-SYNTAX`. |
| `instances` | Top level, at most once, with no subsequent executable statement; otherwise `E-SPEC-PLACEMENT`. Subsequent format metadata is permitted. |
| `each` | First direct statement in a repeat body, at most once; otherwise `E-SPEC-PLACEMENT`. A first declaration named `each` uses up the first statement position. |
| Declarations, `param`, `expect`, other ordinary statements | Top level before `instances`, or in ordinary blocks, subject to their own syntax and semantic checks. |
| `index` | Syntactically permitted wherever an ordinary statement is permitted; execution outside a repeat gives `E-INDEX-CONTEXT`, not a parse-time placement error. |

The ordinary-block restriction also applies to a sections arm's body;
the outer sections braces admit only arms. Whitespace and comments do
not count as statements for `each` placement.

## 5.12 Parser limits

Implementations MUST enforce two independent counters, initially zero.
Attempting to increase either to 257 MUST report
[`E-SPEC-NESTING`](errors.md#114-parse-time-codes) at the current token's
reader-spec line, with null column. These are operational parser limits,
not a general maximum syntax-tree depth.

The block counter MUST increase after an ordinary block's opening brace
is consumed and remain held through its closing brace. An `else` followed
by `if` MUST hold an additional entry while its declaration-first statement
is parsed, including when it is a declaration. The outer braces of
`sections` MUST NOT increase this counter; each arm body does.

The expression counter MUST account as follows:

1. Each invocation parsing unary syntax holds one entry until it returns,
   including an invocation with no unary operator.
2. Binary parsing first parses its left unary expression. For each binary
   operator in levels 1–6, it holds an additional entry before consuming
   that operator and parses the right operand at the next tighter binary
   precedence. It retains all its operator entries until that binary
   invocation returns. They accumulate along a left-associative chain.
3. Postfix parsing first parses a primary. For each suffix, it consumes
   `[` or `.`, then holds an additional entry until that postfix invocation
   returns. Index expressions are parsed while these entries remain held.
4. Parenthesized expressions, call arguments, and both comprehension
   expressions are parsed while the surrounding unary entry remains held.
   Power parses its right operand through another unary invocation.

Returning from these operations releases exactly their held entries.
Separate statements, array dimensions, and call arguments do not accumulate
entries from their completed predecessors. Re-lexing a capture type does
not reset counters.

At otherwise empty nesting, these are the exact boundaries. The field-chain
accepted column means it reaches name resolution, which can still fail.

| Construction | Permitted count | First excessive count |
|---|---:|---:|
| Nested ordinary blocks | 256 | 257 |
| Parenthesis pairs around `1` | 255 | 256 |
| Unary minus signs before `1` | 255 | 256 |
| Additions `+1` after initial `1` | 255 | 256 |
| Powers `^1` after initial `1` | 255 | 256 |
| Index suffixes `[0]` after a name | 254 | 255 |
| Field suffixes `.a` after a name | 255 | 256 |
| Additional else-if arms after an initial if | 255 | 256 |

These limits MUST NOT be substituted for runtime array/count bounds or
the work budgets in chapter 10, `10-work-budgets.md` (planned). There is
no separate language limit on source bytes, tokens, statements, string
length, or the number of separate array dimensions.

The following are complete literal sources at four first-failing boundaries.
Their single long lines are intentional.

```iris example
id: reader-block-limit
operation: parse
--- spec no-final-newline
repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {repeat 0 {}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}}
--- expected error
{"code": "E-SPEC-NESTING", "message": "blocks are nested too deeply", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: reader-expression-limit
operation: parse
--- spec no-final-newline
let x=1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1
--- expected error
{"code": "E-SPEC-NESTING", "message": "expressions are nested too deeply", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: reader-else-if-limit
operation: parse
--- spec no-final-newline
if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {} else if 0 {}
--- expected error
{"code": "E-SPEC-NESTING", "message": "blocks are nested too deeply", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```

```iris example
id: reader-index-limit
operation: parse
--- spec no-final-newline
a[1]:int let x=a[0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0][0]
--- expected error
{"code": "E-SPEC-NESTING", "message": "expressions are nested too deeply", "input_location": null, "spec_location": {"origin": "spec.iris", "line": 1, "column": null}, "context": null, "record_path": null}
```
