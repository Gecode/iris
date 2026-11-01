# Porting Gecode tests

This convention belongs to the conformance suite in [PLAN.md §7](../PLAN.md#7-conformance-suite),
milestone M2. Use the [case and driver contract](README.md) for every ported case.
The reference is Gecode commit `5e7bbd5c26b0702c6fd652356b2289da0710ac3c`.

1. Read `test/instance.cpp` at that pin with
   `git -C /Users/zayenz/gecode/gecode show 5e7bbd5c26b0702c6fd652356b2289da0710ac3c:test/instance.cpp`.
   Never change or build that checkout. Record the pin and assigned source lines.
2. Enumerate every source test group and distinct execution, including loop
   iterations, helper calls, and trailing fails/rejects returns outside `CHECK`.
   Several accessor assertions on one successful execution share one case with
   the complete output.
3. Preserve original reader-spec/input bytes and string parameters. Group cases
   by language chapter and name them `<test-name>-<n>`. Record
   `source = "gecode:test/instance.cpp:<TestName>"` in case.toml. Set the operation
   and catalog output where required.
4. Obtain complete success output from the adapter. Review every expressible
   original assertion against it; retain all entries, types, depths, order and
   values. Review errors against the registry, original expected rejection,
   and primary location/path. `Data::Output` assertions become dzn cases with
   exact bytes. Split different operations and outcomes into separate cases.
5. List each host/API-only assertion individually by source line or assertion
   identity, with its concrete exclusion reason. Do not exclude a group because
   one accessor or narrowing assertion is unportable. Full int64 reads remain
   portable even where `IntArgs` narrowing is not.
6. Map every group's case directories and individual exclusions below. Run the
   batch and report counts and how all retained assertions were checked. Do not
   rewrite expectations to silence disagreements. Stop and report an unapproved
   Specification/reference disagreement. An accepted appendix C difference may
   use only the precise [known-failure contract](README.md#known-failures).

A group may occupy several rows. Every exclusion needs its own row; there is no
implicit unported remainder. Porting batches add their mappings independently.
Specification examples are extracted separately under the
[example transport convention](README.md#specification-examples-and-later-extraction).

| Source group / scenario or assertion | Pinned source lines | Case directories | Host/API exclusion and reason |
|---|---|---|---|
| `Read::Scalars` | 131–143 | `types/scalars-1` | — |
| `Read::Arrays` | 145–157 | `types/arrays-1` | — |
| `Read::Layouts`: nine loop inputs, offset read, trailing failure | 159–179 | `types/layouts-{1..11}` | — |
| `Read::Lists` | 181–190 | `types/lists-1` | — |
| `Read::Patterns` | 192–207 | `structure/patterns-{1..2}` | — |
| `Read::Params`: three reads and trailing missing-parameter failure | 209–228 | `params-instances/params-{1..4}` | Assignment-list parsing is accounted for separately below; all read outcomes are retained. |
| `Read::Params`: `p.set("which=b")` assignment-list syntax | 214 | — | Host convenience parser; the protocol supplies the equivalent pair map for `params-1`, not an assignment-list string. |
| `Read::Params`: `r.set("which=a,,k=1,")` ignores empty assignments | 223 | — | Host convenience parser; the equivalent pair map retains the read in `params-3`, but cannot exercise empty comma-separated assignments. |
| `Read::Params`: malformed assignment `which=a,k` | 225 | — | Host `Params::set` assignment-list failure, before parsing or reading; the protocol accepts only a string-to-string map. |
| `Read::Params`: malformed assignment `=a` | 226 | — | Host `Params::set` assignment-list failure, before parsing or reading; an empty assignment name is not an assignment-list operation in the protocol. |
| `Read::Repeat` | 230–256 | `structure/repeat-{1..3}` | — |
| `Read::Sections` | 258–289 | `structure/sections-{1..14}` | — |
| `Read::Alphabets` | 291–306 | `types/alphabets-{1..9}`, `structure/alphabets-10` | — |
| `Read::Ranges` | 308–329 | `types/ranges-{1..12}` | — |
| `Spec::Types` | 331–365 | `types/types-{1..4}`, `reader-specs/types-{5..7}`, `types/types-{8..18}`, `structure/types-{19..20}` | — |
| `Read::Control` | 367–385 | `expressions/control-1`, `structure/control-2`, `expressions/control-{3..5}` | — |
| `Read::Lexical` | 387–399 | `input/lexical-{1..3}` | — |
| `Read::Robustness`: separators and list terminators | 403–405 | `input/robustness-1` | — |
| `Read::Robustness`: format name syntax and subtraction | 407–409 | `reader-specs/robustness-2` | The read is retained; the metadata accessor is accounted for separately below. |
| `Read::Robustness`: `f.name() == "dimacs-cnf.v2"` | 408 | — | Format metadata is not returned by any protocol operation. Parsing the format declaration and reading with it remain covered by `robustness-2`. |
| `Read::Robustness`: multiplication and offset overflow | 411–412 | `expressions/robustness-3`, `types/robustness-4` | — |
| `Read::Robustness`: invalid float and signed captures | 413–421 | `input/robustness-5`, `structure/robustness-6` | — |
| `Read::Robustness`: malformed numeric tokens and excessive dimension | 422–424 | `input/robustness-{7..8}`, `types/robustness-9` | — |
| `Read::Robustness`: integer list/array tokens and conversion failures | 426–432 | `input/robustness-{10..15}`, `types/robustness-16` | — |
| `Read::Robustness`: malformed pattern and missing repeat field | 433–434 | `structure/robustness-{17..18}` | — |
| `Read::Robustness`: large dimensions with insufficient input | 436–437 | `types/robustness-{19..20}` | — |
| `Read::Robustness`: excessive and small empty arrays | 438–440 | `work-budgets/robustness-{21..22}`, `types/robustness-23` | — |
| `Read::Instances`: selection, single-instance reads, three catalogs, and read errors | 444–475, 482 | `params-instances/instances-{1..11}`, `params-instances/instances-14` | — |
| `Read::Instances`: misplaced instance blocks | 476–481 | `reader-specs/instances-{12..13}` | — |
| `Read::Arithmetic` | 486–504 | `expressions/arithmetic-{1..5}`, `types/arithmetic-6` | — |
| `Read::Progress`: indefinite repeat progress | 508–510 | `structure/progress-{1..3}` | — |
| `Read::Progress`: counted repeats and run-wide idle work | 511–515 | `work-budgets/progress-{4..7}` | — |
| `Read::Shapes`: empty columns, missing scalar, deep paths, and word list | 519–536, 539 | `structure/shapes-{1..4}`, `types/shapes-{5..6}` | — |
| `Read::Shapes`: `e.text("s", {0,2,0})` fails with `index out of range` | 537 | — | Host data-accessor bounds check after a successful read. The protocol returns the complete data, with no accessor request; `shapes-5` retains that read and all valid-path assertions. |
| `Read::Parameters` | 543–559 | `params-instances/parameters-{1..4}` | — |
| `Spec::Checks`: function calls and capture name | 563–567 | `expressions/checks-{1..2}`, `structure/checks-3` | — |
| `Spec::Checks`: parentheses, operator chain, else-if chain, and discard name | 568–579 | `reader-specs/checks-{4..7}` | — |
| `Spec::Checks`: offset on a word | 580 | `types/checks-8` | — |
| `Spec::Strings` | 584–596 | `reader-specs/strings-{1..4}` | — |
| `Spec::Settings`: five loop bodies and multiline placement error | 598–610 | `reader-specs/settings-{1..6}` | — |
| `Spec::Settings`: literal tokens inside a repeat | 611–612 | `structure/settings-7` | — |

## Batch 1 review: lines 131–386

The table paths are relative to [cases/0.1/](cases/0.1/); `{a..b}` means every
integer suffix from a through b, inclusive. Suffixes follow source execution
order within each group, including all nine layout-loop iterations. The batch
contains 84 cases: 38 successful reads and 46 errors (17 parse, 29 read).
There are no catalog or dzn operations in this source range and no appendix C
exclusions or known-failure annotations.

Reader-spec and input bytes were decoded directly from the pinned C++ string
literals, concatenating adjacent literals without adding a final LF. Parameter
maps preserve the values passed to the three reads; cases 2 and 3 deliberately
retain the distinct original executions despite their identical maps and data.

Every successful adapter document was compared against independently written,
complete entry lists derived from the source and the Specification. This checked
entry names, order, value types, depths, full nested shapes, and every scalar
leaf, including values the original tests did not assert. The table below
accounts for every retained successful assertion; its source lines identify
the assertions, not just the read call. The expected files retain the complete
adapter output.

| Group and successful suffixes | Assertion lines | Review against complete expected data |
|---|---|---|
| Scalars 1 | 133–141 | All six scalar values, `e`'s float type, entry count 6, and first name `a`; `b` is also float, `c` is int, and both strings retain their exact text. |
| Arrays 1 | 147–155 | `a` has depth 2, two rows of length 3, flattened leaves 1–6, second row `[4,5,6]`, and leaf `[1,2] = 6`; `_` is absent; float `f` is `[2.5,3]`. The input dimensions `n = 2`, `m = 3` remain entries. |
| Layouts 1–10 | 171, 177 | Each of nine loop inputs produces `[[0,1,2],[1,0,3],[2,3,0]]`; its string `f` remains first. Offset case 10 has only `w = [[0,2,3],[2,0,4],[3,4,0]]`, including unchanged diagonal zeros. |
| Lists 1 | 184–188 | Integer arrays `a = [1,2,3]`, `b = [4,5]`, `c = [6,7]`; string array `w = ["x","y"]` has length 2 and second leaf `y`. Sentinels are absent. |
| Patterns 1 | 198–203 | Ordered captures `n = 6`, `w = 3`, `h = 2`, `name = "a b c"`, `v = 10`, float `c = 2.5`; the discarded capture creates no entry. |
| Params 1–3 | 216–217, 221, 224 | Case 1 has `which = "b"`, default `k = 2`, `x = [3,4]`; cases 2 and 3 have `which = "a"`, integer `k = 1`, `x = [1]`, in that order. |
| Repeat 1, 3 | 239–248, 252–254 | Case 1 has `k = [2,0,1]`, ragged `s = [[3,4],[],[0]]` (three rows, empty middle row, flattened `[3,4,0]`), `a = [7,8]`, `op = "="`, `c = [2,3]`, `rhs = 4`, and two `w` leaves. Also checked `n = 3`, `v = ["x","y"]`, `w = ["foo","bar"]`, and `op` before `c`/`v`. Case 3 has depth-2 `x = [[1,3],[5,7]]`, `y = [[2,4],[6,8]]`. |
| Sections 1, 2, 4, 9, 13, 14 | 265–267, 269, 271, 280, 286–287 | Case 1 defines `name = "a"` before `n = 2`, `x = [5,6]`; case 2 has `n = 1`, one-element `x = [4]`, and no optional name; case 4 has only `v = 3`; case 9 has `a = 1`, `y = 5`; cases 13 and 14 each have only `a = 1`. |
| Alphabets 1–4 | 293, 295, 297–298 | Full depth-2 matrices are `[[1,0,4],[6,2,0]]`, `[[0,1],[1,0]]`, `[[1,2]]`, and `[[2]]`; case 3 has one outer row, and case 4 appends scalar `base = 7`. |
| Ranges 1, 6, 9 | 310–311, 318, 324 | Case 1 has `n = 2`, `a = [0,1]`, `b = -5`, `c = -2`; case 6 has offset `a = [0,1]` and `l = [3]` without its out-of-range sentinel; case 9 captures `n = 3`, `m = 2`. |
| Types 1, 5–7 | 338–339, 346–347, 349–350 | Case 1 has `n = 2`, `ready = [1,3]`, `due = [2,4]`, `g = [[1,0]]`, with no type-definition entries. Case 5 retains all keyword-named entries `line = 1`, `type = 2`, `list = "a"`, `each = 3`, `layout = 4`; case 6 has `a = 0`, `b = [1,2]`; case 7 has `a = 0`, `index = 1`. |
| Control 1–2 | 373–375, 377–380 | Case 1 executes all three checks and produces `n = 2`, `w = "x"`, `m = 7`, `a = [1,3]`, `s = 4` in order. Case 2 executes the else body and produces `n = 1`, `w = "none"`. |

For all 46 errors, the original `fails`/`rejects` message assertion was checked
against the adapter diagnostic, including every explicitly asserted coordinate.
Each code was chosen by failure meaning from [errors.md](../spec/0.1/errors.md)
before comparing the adapter output. In particular, missing sections arms use
`E-LINE-MATCH`, range conversion/rejection uses `E-EXPECTED-VALUE`, the
exclusive-bound underflow uses `E-INTEGER-OVERFLOW`, and malformed captures use
`E-SPEC-PATTERN` except missing range bounds (`E-SPEC-SYNTAX`). All errors retain
the adapter's full location document. Parse columns are null; the plain false
check in `control-3` has path `[]`; every other error has a null path.

Validation against `gecode-6.5.0-5e7bbd5c26b0`, including the 15 unchanged smoke
cases: `Summary: total=99 pass=99 xfail=0 fail=0 xpass=0` (exit 0).

## Batch 2 review: lines 387–615

This batch contains 82 cases: 25 successful reads, three successful catalogs,
and 54 errors (20 parse, 34 read). There are no dzn operations, appendix C
exclusions, or known-failure annotations in this source range. The two
host/API exclusions are listed individually in the table above. New areas
`input` and `work-budgets` follow chapters 4 and 10; the other cases reuse the
existing chapter areas. Suffixes follow source execution order within each
group, including the repeated number-selected reads and all five settings-loop
iterations. The excluded accessor in Shapes does not consume a suffix.

Reader-spec and input bytes were decoded from the pinned C++ string literals,
without adding final newlines or normalizing CR LF. The three parser-depth
cases retain the source's 300 nested parentheses, 300 additions, and 300
else-if additions. Parameter maps preserve each supplied string, including
the distinct name and ordinal selectors.

A temporary Python script reads the pinned source, extracts all 96 `CHECK`
expressions in this range, and checks them against the expected files. It
evaluates the 44 successful assertions through typed data accessors, including
paths, flattened arrays, presence, depth, and size. It checks all 54 error
executions against the original message fragments and every explicitly
asserted coordinate, expanding the five settings-loop iterations separately.
The remaining two `CHECK` expressions are the individual API exclusions.
Every retained assertion agrees.

Complete success entry lists were also written independently from the source
and the Specification and compared with the adapter output. That comparison
checks every entry's name, order, value type, depth, shape, and scalar leaves,
including values not asserted by the original tests. The expected files retain
the complete adapter output. The source-derived reader-spec/input bytes and
parameter maps were checked separately against every case file.

| Group and successful suffixes | Assertion lines | Review against complete expected data |
|---|---|---|
| Lexical 1, 3 | 394–395, 397 | Case 1 has string `w = ["a","b"]` after comment, stop, token, and CR LF handling; case 3 has only integer `a = 1`, ignoring the trailing token. |
| Robustness 1, 2, 6, 10, 11, 23 | 404–405, 409, 418–421, 426–427, 440 | Case 1 has integer `a = [1,2,3]`, `b = [4,5]`; case 2 has `n = 5`, `x = 2`, `k = 3`. Case 6 has float `a = -150`, integer `b = 3`, float `c = 2.5`, float `f = 0.25`. Cases 10 and 11 have integer `a = [5,-7,5]` and `[1,-2,3]`; case 23 has `r = 3`, `c = 0`, depth-2 `x = [[],[],[]]`. |
| Instances 1–3, 6 | 458–460, 464 | Case 1 has `name = "b"`, `n = 1`, `x = [7]`; cases 2 and 3 each have `name = "a"`, `n = 2`, `x = [1,2]`, preserving both original executions. Case 6 has no name, only `n = 3`, `x = [4,5,6]`. |
| Instances 7–9 (catalog) | 468–469, 471–472 | Case 7 has integer `instances = 2` and string `name = ["a","b"]`. Cases 8 and 9 each have only `instances = 1`, with no name or read-data entries. |
| Arithmetic 1, 4 | 491–493, 498–500 | Case 1 has integer `a = [-5,2,1]`, floats `g = -1.5`, `h = 4`, then integer `s = -2`, `lo = -5`, `hi = 7`. Case 4 retains full int64 `a`, `p`, and `r` at -9223372036854775808, with `q = 0`. |
| Progress 2, 5 | 509, 512 | Immediate termination produces empty string `w` at depth 1. The small counted repeat has `n = 3` and integer `a = [[],[],[]]` at depth 2. |
| Shapes 1–3, 5–6 | 522–523, 525–527, 529, 533–536, 539 | Case 1 has float `x = [[],[]]` at depth 3. Case 2 has empty float `x` at depth 3, empty string `w` at depth 1, and empty integer `l` at depth 2. Case 3 has `k = [1,2]` and integer `x = [[],[]]` at depth 3. Case 5 has string `s = [[["a","b"],["c","d"]]]` and integer `v = [[[1,2]],[[3,4]]]`, both at depth 3; all four path assertions agree. Case 6 has string `w = ["a","b"]`, without its sentinel. |
| Parameters 1, 3 | 546, 554 | Case 1 has inferred float `k = 1.5`; case 3 has string `s = "3"` and integer `i = 4`, in definition order. |
| Strings 1 | 587 | The sole string entry `s` contains the exact LF, tab, backslash, and double quote asserted by the source. |
| Settings 7 | 612 | Literal tokens inside the repeat produce only integer `a = [1,2]`. |

Each error code was selected by failure meaning from
[errors.md](../spec/0.1/errors.md) before comparing adapter results. Numeric
input overflow uses `E-EXPECTED-VALUE`, while multiplication, offset, and
alphabet adjustment overflow use `E-INTEGER-OVERFLOW`. Excessive dimensions
use `E-COUNT`; insufficient large arrays still fail at the first missing
value or symbol. Nonprogressing indefinite repeats and instances use
`E-NO-PROGRESS`; excessive counted repeats and empty rows use `E-BUDGET`.
The latter remain ordinary-sized budget arithmetic, outside appendix C.3.
All errors retain the adapter's full location document with case-relative
origins and null record paths. The unknown parameter has null reader-spec
line/column; all parse columns are null.

Validation against `gecode-6.5.0-5e7bbd5c26b0`, including batch 1 and the
15 unchanged smoke cases:
`Summary: total=181 pass=181 xfail=0 fail=0 xpass=0` (exit 0).
