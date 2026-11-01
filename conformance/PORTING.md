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
