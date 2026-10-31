# 2 Notation and executable examples

## 2.1 Requirement words

The words MUST, MUST NOT, REQUIRED, SHALL, SHALL NOT, SHOULD, SHOULD NOT,
RECOMMENDED, MAY, and OPTIONAL have their RFC 2119 meanings only when
capitalized. Normative rules describe required behavior; text explicitly
marked informative explains it without adding requirements.

## 2.2 Grammar notation

The planned `grammar.ebnf` uses the following EBNF dialect. A production has
the form `name = expression ;`. Juxtaposition means concatenation, `|`
separates alternatives, parentheses group expressions, brackets enclose an
optional expression, and braces enclose zero or more repetitions. Single
or double quotes delimit literal terminals; the other quote may occur
literally inside them. `(* ... *)` delimits a comment. `? ... ?` marks a
special sequence whose meaning MUST be explained beside its production.
Comments and special sequences do not nest. No escape convention is implicit
in quoted terminals; a terminal needing both quote characters uses an
explained special sequence.

Whitespace between grammar symbols separates notation and does not imply
input whitespace. Chapter 5, `05-reader-specs.md` (planned), defines
reader-spec whitespace and lexical rules.

## 2.3 Example headers and parts

Each executable example MUST occupy one unindented Markdown fenced block
with the info string exactly `iris example`. The fence uses at least three
backticks. Its body uses LF line endings and contains header lines followed
by parts. An extractor MUST reject unknown or repeated headers, unknown or
repeated parts, invalid markers, and combinations disallowed below.

The first header is `id: ID`, where ID matches
`[a-z]+(-[a-z0-9]+)+` and is unique throughout the 0.1 Specification.
The first word SHOULD identify the chapter, such as `notation` or `data`.
The only other header is `operation: OP`, where OP is `read`, `parse`,
`catalog`, or `dzn`. It MAY be omitted; omission means `read`. Headers have
exactly one ASCII space after the colon and no leading or trailing spaces.
There are no blank header lines.

Each marker is a whole line consisting of `--- ` followed by a part name
and, when allowed, one space and one modifier. Part names are `spec`,
`input`, `params`, `expected data`, `expected dzn`, and `expected error`.
The byte parts (`spec`, `input`, `expected dzn`) allow either no modifier,
`no-final-newline`, or `hex`. Other parts allow no modifier. Markers have
no leading or trailing spaces. Parts MUST occur in this order:

1. `spec` (required).
2. `input` (required except for `parse`).
3. `params` (optional).
4. Exactly one expected part (required).

The operations mean:

| Operation | Action | Successful expected part |
|---|---|---|
| `read` | Parse the reader spec and read one selected instance. | `expected data` |
| `parse` | Parse and resolve the reader spec without reading input. | Reserved for the conformance protocol; see below. |
| `catalog` | Parse the reader spec and catalog the input's instances. | `expected data` |
| `dzn` | Read as for `read`, then render dzn. | `expected dzn` |

Any operation MAY have `expected error` instead. A `parse` example MAY omit
input; if present, the operation MUST NOT read it. Its params MUST be absent
or empty. Parse-success examples are reserved until conformance-001 defines
their result and extends this notation. Empty read data MUST NOT stand in
for parse success. Chapter 9, `09-params-and-instances.md` (planned), defines
catalog entries.

A params part is a JSON object mapping strings to strings. Omission means
an empty map. An extractor MUST preserve each value exactly, including leading
zeros and commas; it MUST NOT parse values as numbers or split them. Expected
data and error parts are complete [canonical JSON documents](03-data-model.md).
All JSON parts MUST reject duplicate members, non-finite number extensions,
and unpaired Unicode surrogate escapes. Insignificant JSON whitespace and
object-member order do not affect their meaning.

## 2.4 Byte parts and splitting algorithm

A literal byte part consists of its content lines, each encoded as UTF-8
without a BOM and followed by one LF byte. It MUST contain only Unicode
scalar values and MUST NOT contain a CR byte. Tabs MAY appear literally.
The modifier `no-final-newline` removes the last LF from the resulting
bytes, if any. Neither the Markdown fence nor a part marker contributes
bytes. Thus the ordinary final content line supplies its LF even immediately
before the closing fence; there is no additional LF from the fence.

An empty part has its next marker or the closing fence immediately after
its marker, and represents zero bytes. One empty content line represents
one LF. With `no-final-newline`, that one empty line represents zero bytes
as well. Blank lines within a part are content, never visual separators.

For `hex`, an extractor MUST discard only ASCII space, tab, and LF from the
content, then decode pairs of lowercase hexadecimal digits into bytes.
It MUST reject any other character or an odd digit count. Zero digits means
zero bytes. Hex can represent CR LF, NUL, invalid UTF-8, and any other bytes.
For example, `ff0a` represents byte 255 followed by LF; its transport does
not imply that Iris accepts it as a string.

A literal content line MUST NOT begin with `--- `; use `hex` to represent
such a line. When content contains a run of three or more backticks, the
Markdown fence MUST be longer than every such run in the content.

An extractor MUST use this splitting algorithm:

1. Remove the opening and closing fence lines. Split the body at LF into
   physical lines, excluding the line terminators; a terminal LF does not
   create an extra empty line. Reject CR anywhere in the body.
2. Read the headers up to the first line beginning with `--- `. Parse that
   line and every later line beginning with `--- ` as a marker using §2.3.
   Collect intervening lines as the preceding marker's content. The closing
   fence ends the last part.
3. Check the headers, part order, modifiers, and operation requirements.
   Decode byte parts using the literal or hex rules above. Join JSON-part
   content lines with LF and parse each as one JSON document. Validate params
   as a string map and expected documents under chapter 3.

Backslashes in literal byte parts are bytes for the reader spec or input;
the extractor MUST NOT interpret them as escapes. JSON escapes apply only
within JSON parts. This format describes example transport;
[chapter 4](04-input.md) defines input lexing and coordinate counting.

## 2.5 Complete examples

This input contains the single byte `37` in hexadecimal, with no final LF.

```iris example
id: notation-no-final-newline
operation: read
--- spec
x: int
--- input no-final-newline
7
--- expected data
{"iris": "0.1", "entries": [
  {"name": "x", "type": "int", "depth": 0, "value": 7}
]}
```

This input is exactly `1`, CR LF, a tab, and `2`, with no final LF.

```iris example
id: notation-crlf-tab
operation: read
--- spec
a: int
b: int
--- input hex
31 0d 0a 09 32
--- expected data
{"iris": "0.1", "entries": [
  {"name": "a", "type": "int", "depth": 0, "value": 1},
  {"name": "b", "type": "int", "depth": 0, "value": 2}
]}
```

NUL is a Unicode character. Here the input is NUL followed by LF, and JSON
escapes the NUL in the resulting string.

```iris example
id: notation-nul-word
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
