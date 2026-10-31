# B dzn output

This appendix defines the exact bytes rendered from the data of
[chapter 3](03-data-model.md). The rules apply independently of the
implementation language, locale, and host stream settings. In byte
spellings below, LF is `0a`, space is `20`, and punctuation is ASCII.

## B.1 Rendering data and operation boundaries

Rendering MUST preserve [entry order](03-data-model.md#31-entries-and-definition-order)
and render each entry once, using its final value at its existing position.
It MUST NOT sort entries, expose record names as extra entries, include
format metadata, or add a filename or header. It MUST render stored values
without rereading input, reevaluating expressions, checks, or defaults, or
reinterpreting original token spellings.

The `dzn` [example operation](02-notation.md#23-example-headers-and-parts)
MUST perform the same parsing, parameter admission, UTF-8 validation,
instance selection, execution, and trailing-input checks as `read`, then
render its successful data. An earlier failure MUST retain its existing
code and locations. Failed reading produces no successful dzn document.
Rendering successful [catalog data](09-params-and-instances.md#96-catalog-results)
MUST use these same rendering rules; §B.7 defines its example notation.

Rendering MUST add zero idle-work and expression-effort units under
[chapter 10](10-work-budgets.md), regardless of depth, leaf count, output
size, or the work used to choose numeric text. Successful execution exactly
at either limit MUST remain renderable. Rendering introduces no language
error code; §B.8 covers output failures.

## B.2 Entry names and integers

An entry name MUST be emitted unchanged if its first byte is an ASCII
letter and it is not exactly one of these case-sensitive names:

```text
ann annotation any array bool case constraint default diff div else elseif
endif enum false float function if in include int intersect let list
maximize minimize mod not of op opt output par predicate record satisfy
set solve string subset superset symdiff test then true tuple type union
var where xor
```

Every other entry name MUST be enclosed in single ASCII quotes. Thus `_`
and every underscore-leading name are quoted, `var` is quoted, and `Var`
and `annotation_2` are plain. Implementations MUST use this frozen set,
independently of any host MiniZinc package. Entry names remain subject to
[§5.4](05-reader-specs.md#54-names); these output rules introduce neither
arbitrary identifiers nor a name-escape syntax.

An int leaf MUST use its exact signed base-10 spelling: a minus sign only
for negative values, no plus sign or leading zeros, and no decimal point
or exponent. Zero MUST be `0`. Implementations MUST preserve the entire
[signed 64-bit range](03-data-model.md#34-integers) without narrowing to a
host int or converting through floating point.

```iris example
id: dzn-scalars-names-order
operation: dzn
--- spec
format ignored
let z=1 let a="text" let z=9
lo:int hi:int var:int _name:int Var:int annotation_2:int
repeat until "END" store _ {}
--- input no-final-newline
-9223372036854775808 9223372036854775807 3 4 5 6 END
--- expected dzn
z = 9;
a = "text";
lo = -9223372036854775808;
hi = 9223372036854775807;
'var' = 3;
'_name' = 4;
Var = 5;
annotation_2 = 6;
'_' = "END";
```

## B.3 Floating point text

For every finite binary64 float leaf f, implementations MUST use the
algorithm below. All rounding in this algorithm MUST be mathematical
round to nearest, ties to even, independently of the caller's locale or
floating point rounding mode. This spelling differs from
[canonical JSON](03-data-model.md#35-floating-point-values). A host default
conversion, fixed 17-digit format, or unconstrained shortest-output
algorithm MUST NOT replace these rules.

1. If f is zero, emit `0.0` for positive zero or `-0.0` for negative zero
   and stop. Rendering MUST preserve the stored sign. Reading declarations
   already apply [default offset addition](06-types-and-values.md#67-offsets-and-evaluation-order);
   `let`, captures, and supplied float parameters can retain negative zero.
2. Let m be the exact magnitude of f. If 1 ≤ m < 100000000000000000,
   let the initial precision p0 be the number of decimal digits in the
   integer part of m. Otherwise let p0 = 1. This digit count MUST use exact
   comparisons, without a `log10` approximation.
3. For p = p0 through 17 in increasing order, round the exact m to p
   significant decimal digits. On an exact tie, choose the decimal with
   an even last retained digit. Convert that decimal to binary64 using
   round to nearest, ties to even. Select the first p whose conversion
   equals m; at p = 17 select its rounded decimal in any event. Subsequent
   steps use that p and that rounded decimal.
4. Let e be the rounded decimal's normalized base-10 exponent, after any
   rounding carry: its magnitude is at least 10^e and less than 10^(e+1).
   Use scientific notation exactly when e < -4 or e ≥ p. It MUST have one
   leading digit, a decimal point only when fractional digits remain,
   lowercase `e`, an explicit `+` or `-` exponent sign, and at least two
   exponent digits, padding with leading zeros only to reach two digits.
   Examples are `1e-05`, `1e+17`, and `1e+100`.
5. Otherwise use fixed decimal notation with all necessary integer digits,
   a leading `0` before a fractional decimal point, and no exponent.
   In either notation, remove insignificant fractional trailing zeros and
   an unnecessary decimal point. Significant integer zeros MUST remain.
6. Prepend `-` exactly when f is negative. If the text contains neither
   `.` nor `e`, append `.0`. Thus `20.0`, `1000.0`, and
   `10000000000000000.0` retain fixed integral-float spelling; `1e+17`
   receives no `.0`.

This is an increasing-precision round-trip search beginning at p0, rather
than a global search for the shortest text. Even when an integral float
has a shorter scientific spelling, its integer-digit starting precision
can require fixed notation.

```iris example
id: dzn-float-spellings
operation: dzn
--- spec
f[18]:float
let negative=-0.0
param supplied=0.0
line "{captured:float}"
--- input
0 -0.0 20 1000 0.1 1.2345678901234567 0.0001 0.00001 1e16 1e17 1e20 1e-7 1e100 1e-100 1.7976931348623157e308 2.2250738585072014e-308 9007199254740992 9999999999999998
-0.0
--- params
{"supplied":"-0.0"}
--- expected dzn
f = [0.0, 0.0, 20.0, 1000.0, 0.1, 1.2345678901234567, 0.0001, 1e-05, 10000000000000000.0, 1e+17, 1e+20, 1e-07, 1e+100, 1e-100, 1.7976931348623157e+308, 2.2250738585072014e-308, 9007199254740992.0, 9999999999999998.0];
negative = -0.0;
supplied = -0.0;
captured = -0.0;
```

Iris-only correction; the pinned Gecode reference emits empty text for
float leaves with 99999999999999600 ≤ |f| < 100000000000000000, as recorded
in [appendix C.4](appendix-c-differences.md#c4-dzn-floats-just-below-1e17).
The algorithm above renders every one of those 25 positive values and
its negative with p = 17 in fixed notation plus `.0`. The following
expected bytes follow that algorithm, independently checked with exact
decimal arithmetic; they are not the pinned reference's output.

```iris example
id: dzn-float-precision-gap
operation: dzn
--- spec
let below=99999999999999584.0
let edge=99999999999999600.0
let negative=-99999999999999984.0
let above=1e17
values[2]:float
--- input
99999999999999984.0 1
--- expected dzn
below = 99999999999999584.0;
edge = 99999999999999600.0;
negative = -99999999999999984.0;
above = 1e+17;
values = [99999999999999984.0, 1.0];
```

## B.4 String quoting and UTF-8 bytes

Every string leaf, including an empty string, MUST be enclosed in ASCII
double quotes. Implementations MUST process its UTF-8 encoding bytes in
order, without normalization, using this table. Byte values are hexadecimal.

| Byte | Output bytes |
|---|---|
| `22` (double quote) | Backslash followed by double quote: `\"` |
| `5c` (backslash) | Two backslashes: `\\` |
| `0a` (LF) | Backslash followed by `n`: `\n` |
| `09` (tab) | Backslash followed by `t`: `\t` |
| Every other byte `00` through `1f` | `\xhh`, with exactly two lowercase hexadecimal digits for the byte |
| All remaining bytes | The byte itself |

Consequently CR is `\x0d`, backspace `\x08`, form feed `\x0c`, NUL
`\x00`, and ESC `\x1b`. Implementations MUST NOT substitute JSON `\u`
escapes or named `\r`, `\b`, or `\f` escapes. DEL (`7f`), slash,
apostrophe, and every non-ASCII UTF-8 byte remain literal. This includes
U+0085 and combining sequences. String validity continues to follow
[§3.6](03-data-model.md#36-strings) and
[appendix C](appendix-c-differences.md#c1-unicode-strings).

```iris example
id: dzn-string-bytes
operation: dzn
--- spec
let empty="" let escaped="a\nb\tc\\d\"e" payload:rest
--- input hex
58 00 01 02 03 04 05 06 07 08 09 0b 0c 0d 0e 0f 10 11 12 13 14 15 16 17 18 19 1a 1b 1c 1d 1e 1f 22 5c 2f 7f c3 a9 20 65 cc 81 20 c2 85 20 f0 9f 98 80 59 0a
--- expected dzn hex
65 6d 70 74 79 20 3d 20 22 22 3b 0a 65 73 63 61 70 65 64 20 3d 20 22 61 5c 6e 62 5c 74 63 5c 5c 64 5c 22 65 22 3b 0a 70 61 79 6c 6f 61 64 20 3d 20 22 58 5c 78 30 30 5c 78 30 31 5c 78 30 32 5c 78 30 33 5c 78 30 34 5c 78 30 35 5c 78 30 36 5c 78 30 37 5c 78 30 38 5c 74 5c 78 30 62 5c 78 30 63 5c 78 30 64 5c 78 30 65 5c 78 30 66 5c 78 31 30 5c 78 31 31 5c 78 31 32 5c 78 31 33 5c 78 31 34 5c 78 31 35 5c 78 31 36 5c 78 31 37 5c 78 31 38 5c 78 31 39 5c 78 31 61 5c 78 31 62 5c 78 31 63 5c 78 31 64 5c 78 31 65 5c 78 31 66 5c 22 5c 5c 2f 7f c3 a9 20 65 cc 81 20 c2 85 20 f0 9f 98 80 59 22 3b 0a
```

## B.5 Rectangular arrays and empty dimensions

An array MUST be treated as rectangular exactly when, separately at each
array level, all existing arrays at that level have the same length.
The comparison MUST include arrays under different parents, not just
siblings. A level with no existing arrays satisfies this condition.
Rectangular entries of depth 1 through 6 MUST use this section's
representation, regardless of which statements created them.

For each level, its extent MUST be the length of its first existing array,
or 0 when no array exists there. Extents MUST come from the resulting
data, not saved reader-spec dimensions. Thus `[0][3]` has output extents
0,0; `[2][0][9]` has 2,0,0. Implementations MUST NOT invent dimensions
below an empty array.

A depth-1 array MUST use `[` followed by its scalar leaves separated by
exactly comma-space, then `]`, without a trailing comma. Empty int, float,
and string vectors MUST all render `[]`.

A depth-2 array with positive row and column extents MUST use matrix
syntax: start with `[| `, separate leaves within a row with `, `, start
each subsequent row with LF followed by two spaces and `| `, then end with
` |]`. There MUST NOT be a leading newline or an extra row separator.
One-row and one-column matrices MUST use the same rule.

Empty depth-2 arrays and all rectangular arrays at depths 3 through 6
MUST use `arrayNd(1..s1, 1..s2, ..., [LEAVES])`, replacing N with the
entry's decimal depth and each si with its level's extent, using §B.2's
integer spelling. There MUST be comma-space after each index set. LEAVES MUST be the row-major
flattened leaves, with the last level varying fastest, separated by
comma-space without a trailing comma. Empty index sets MUST be `1..0`;
an empty leaf list MUST be `[]`. No B.5 array receives a type comment.

```iris example
id: dzn-rectangular-arrays
operation: dzn
--- spec
words[2]:word matrix[2][2]:float
three[2][1][2]:int four[1][1][1][2]:int
five[1][1][1][1][2]:int six[1][1][1][1][1][2]:int
--- input no-final-newline
hi there 1 2.5 3 4 1 2 3 4 5 6 7 8 9 10
--- expected dzn
words = ["hi", "there"];
matrix = [| 1.0, 2.5
  | 3.0, 4.0 |];
three = array3d(1..2, 1..1, 1..2, [1, 2, 3, 4]);
four = array4d(1..1, 1..1, 1..1, 1..2, [5, 6]);
five = array5d(1..1, 1..1, 1..1, 1..1, 1..2, [7, 8]);
six = array6d(1..1, 1..1, 1..1, 1..1, 1..1, 1..2, [9, 10]);
```

The last entry below uses the depth-greater-than-6 rule in §B.6.

```iris example
id: dzn-empty-arrays
operation: dzn
--- spec
i[0]:int f[0]:float s[0]:word
outer[0][3]:int inner[2][0]:float
three[2][0][9]:word four[1][2][0][7]:int
five[0][2][3][4][5]:float six[1][0][3][4][5][6]:int
seven[0][2][3][4][5][6][7]:int
--- input
--- expected dzn
i = [];
f = [];
s = [];
outer = array2d(1..0, 1..0, []);
inner = array2d(1..2, 1..0, []);
three = array3d(1..2, 1..0, 1..0, []);
four = array4d(1..1, 1..2, 1..0, 1..0, []);
five = array5d(1..0, 1..0, 1..0, 1..0, 1..0, []);
six = array6d(1..1, 1..0, 1..0, 1..0, 1..0, 1..0, []);
% seven: array[int] of tuple(array[int] of tuple(array[int] of tuple(array[int] of tuple(array[int] of tuple(array[int] of tuple(array[int] of int))))))
seven = [];
```

## B.6 Ragged and deeper arrays

Every nonrectangular array entry and every array entry of depth greater than 6
MUST use the nested one-tuple representation below, even when a deep array
is rectangular or empty. This choice MUST apply to the entire entry;
a rectangular subarray within a ragged entry MUST NOT independently use
matrix syntax.

Define the type string T1 as `array[int] of TYPE`, where TYPE is exactly
`int`, `float`, or `string` according to the entry's value type. Define
T(k+1) as `array[int] of tuple(` followed by Tk and `)`. A depth-d entry
MUST emit `% NAME: Td` followed by LF immediately before its assignment,
substituting its original unquoted name for NAME and its type string for
Td. A quoted assignment name MUST still be unquoted in the comment.

Each actual array MUST render as `[` followed by comma-space-separated
items and `]`, without a trailing comma. At the last array level, items
MUST be scalar leaves rendered under §§B.2–B.4. At every higher level,
each child array MUST render recursively, wrapped as `(` followed by
CHILD and `,)`. Thus a child `[5]` becomes `([5],)` and an empty child
becomes `([],)`. Higher levels repeat the wrapping. Implementations MUST
NOT insert line breaks or indentation inside this value. Every existing
empty array remains `[]`; nonexistent descendants MUST NOT be created.
For a completely empty deep entry, only the comment retains its type and
depth.

```iris example
id: dzn-ragged-and-deep
operation: dzn
--- spec
repeat 2 {list var:int until 0 list real:float until 0 list text:word until "END"}
repeat 2 {k:int repeat k {list nested:int until 0}}
deep[1][1][1][1][1][1][1]:int
--- input no-final-newline
1 0 1.5 0 a END 2 3 0 2.5 3.5 0 b c END 1 7 0 2 8 9 0 0 5
--- expected dzn
% var: array[int] of tuple(array[int] of int)
'var' = [([1],), ([2, 3],)];
% real: array[int] of tuple(array[int] of float)
real = [([1.5],), ([2.5, 3.5],)];
% text: array[int] of tuple(array[int] of string)
text = [(["a"],), (["b", "c"],)];
k = [1, 2];
% nested: array[int] of tuple(array[int] of tuple(array[int] of int))
nested = [([([7],)],), ([([8, 9],), ([],)],)];
% deep: array[int] of tuple(array[int] of tuple(array[int] of tuple(array[int] of tuple(array[int] of tuple(array[int] of tuple(array[int] of int))))))
deep = [([([([([([([5],)],)],)],)],)],)];
```

## B.7 Catalog data and executable examples

Successful catalog data MUST render using the same entry, scalar, and
array rules as read data. [§9.6](09-params-and-instances.md#96-catalog-results)
continues to define its entries and their order: scalar `instances`,
optional string vector `name`, then optional string vector `failed`.
Each `failed` element MUST retain the entire exact summary required there,
including the four summary templates and reader-spec line number.
Implementations MUST apply §B.4 to these data strings, including embedded
quotes, backslashes, LF, tabs, and controls; they MUST NOT localize or
reword them or convert them into error documents. A valid instance's
empty summary MUST remain `""`.

Under [§2.3](02-notation.md#23-example-headers-and-parts), a `catalog`
example with `expected dzn` MUST catalog the input, then render the
returned data. A `catalog` example with `expected data` retains its
canonical JSON result. The expected part chooses the representation;
it does not change which operation constructs the data. The `dzn`
operation continues to mean read-then-render under §B.1. This notation
defines no command-line option combination.

```iris example
id: dzn-catalog-summaries
operation: catalog
--- spec
instances {
 sections {
  "NAME {name}" optional
  "X {x:int}"
  "END" last
 }
 check !defined(x)||x>0 "positive"
 check !defined(x)||(x!=0 && x!=2)
}
--- input
NAME a
X 0
END
NAME b
NAME c
X 1
END
NAME d
END
NAME e
X 2
END
NAME f
X 3
END
--- expected dzn
instances = 5;
name = ["a", "c", "d", "e", "f"];
failed = ["positive", "a second line matching \"NAME {name}\"", "no line matching \"X {x:int}\"", "check on line 8 of the spec", ""];
```

## B.8 Separators, line endings, and errors

Each assignment MUST consist of its rendered name, exactly ` = `, its
rendered value, exactly `;`, then one LF. A B.6 type comment MUST immediately
precede its assignment; a B.5 matrix MUST contain the internal LFs defined
there. Output MUST NOT contain blank lines between entries, a BOM, CR LF
conversion, tab indentation, or an additional final newline. Nonempty data
ends in exactly the LF terminating its last assignment. Zero entries MUST
produce zero bytes, with no final LF.

In [executable examples](02-notation.md#24-byte-parts-and-splitting-algorithm),
a literal `expected dzn` part includes its final LF. An empty expected
part represents zero bytes. `hex` makes controls, CR, DEL, and other
visually ambiguous bytes explicit; `no-final-newline` on an input part
removes that input's final LF. It MUST NOT remove the required LF from a
nonempty expected dzn result.

The following read succeeds exactly at the idle-work limit and renders
zero bytes.

```iris example
id: dzn-empty-data
operation: dzn
--- spec
repeat 1048576 {}
--- input
--- expected dzn
```

Rendering introduces no new [language error code](errors.md). Failures
of the preceding read or catalog MUST follow [§11.2](errors.md#112-error-order)
and prevent a successful dzn document. Catalog rejection summaries remain
data under §B.7. Host output-stream or I/O failures and allocation failures
remain outside language errors under [§11.7](errors.md#117-exclusions-and-source-coverage).
