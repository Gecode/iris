# iris-core

Input reading, expression evaluation, column data, canonical JSON, and dzn
output for Iris 0.1. This is the runtime layer in [PLAN.md
§4.2](../../PLAN.md#42-the-rust-reference-implementation-is-layered), part of
milestone M3 ([§8.1](../../PLAN.md#81-rust-rust)). It depends on `iris-syntax`
and `iris-sema`; parsing and resolution use their existing APIs.

```rust
use iris_core::{Params, Spec};

let spec = Spec::parse(b"x:int let square=x^2", "scalar.iris")?;
let data = spec.read(b"7", "input.txt", &Params::new())?;
assert_eq!(data.int("square")?, 49);
let json = data.to_json()?;
let dzn = data.to_dzn()?;
assert_eq!(dzn, b"x = 7;\nsquare = 49;\n");
# Ok::<(), Box<dyn std::error::Error>>(())
```

`Spec::format_name` returns the decoded format name when declared.
`Spec::parse` selects language 0.1; `parse_version` takes an explicit
`LanguageVersion`. A valid `Spec` can read several inputs independently.
`read` borrows bytes only for the duration of execution; `read_path` acquires
a file and calls it. `catalog` and `catalog_path` execute every instance and
return the count, usable names, and first rejection summaries as ordinary data.
Returned data and errors own their retained content.
The byte-slice interface also permits a future caller to use a memory map.

Hosts can pass `ReadOptions { cancellation: Some(token) }` to
`Spec::read_with_options`. `CancellationToken` is a monotonic, cloneable atomic
flag; `cancel()` requests cancellation and `is_cancelled()` observes it.
`Spec::read` supplies default options with no token. Catalog and path acquisition
keep their existing interfaces. Cancellation returns the distinct
`Failure::Host(HostError::Cancelled)`, including after a deferred instance
rejection; it is neither a language error nor `E-BUDGET`, and has no canonical
language-error JSON. Budgets and charge order are unchanged.

The interpreter checks short dispatches and continuations, and chunks long
scans, value copying, promotion, offsets, array metadata, aggregates and record
walks. A supplied token is checked at least every 1,024 explicit row/leaf/metadata
iterations or 64 KiB of scanning/copying. Without a token, bulk helpers retain
their ordinary path. This is a bound on cooperative work, not elapsed time:
parser analysis, OS acquisition/scheduling, allocation/deallocation and a single
standard-library conversion are indivisible. Cancellation cannot interrupt a
blocked syscall or instantly release a large allocation. Parsing remains outside
the cancellable reading API. Spec names are interned when the spec loads; reading
keys frame, record and evaluator maps by small identifiers. Host parameter keys are admitted
once with checked comparisons, then statements use those same identifiers.

`summary::write_summary` writes the CLI's unlimited summary, preserving its
bytes. `summary::preview` uses the same shape renderer with editor limits:
64 entries, 1,024 second-level rows in total, 16 dimensions, 160 Unicode scalars
per name/label/string before escaping, and 16 KiB UTF-8 output. Limits bound work
and intermediate output as well as the final text; omissions are explicit, and
incomplete row scans never claim a full shape. Rendering consumes no language
work budget. `Error::excerpt()` borrows the runtime's existing bounded excerpt.

Execution supports int, float, num, word, rest, and integer ranges;
named types with frozen bounds, alphabet options, and aliases; fixed arrays,
including empty rows; numeric offsets; every matrix layout; sentinel and
end-of-line lists; and byte alphabets. Counted, token-terminated, and EOF
repeats collect final local values into columns, including ragged and missing
array rows. Index statements validate the innermost iteration number.
Line patterns support typed captures, current-value references and selected
range checks. `line`, `find`, `skip` and `expect` operate on the remaining input;
sections select arms in declaration order with optional/last arms, terminators,
stores and skip-other handling. Settings execute in statement order.
The evaluator supports if/else and let, scalar operators and comparisons,
array truth, flattened integer aggregates, size/count, indexing, and ordinary
array and record-list comprehensions with scoped bindings. Checks run where
written and align lifted columns or logical record markers in record order.
Structural `as`/`each` names support current records, list indexing, nested
lists, and borrowed ordinary fields, including completed empty lists. Integers use checked 64-bit
arithmetic. Numeric reads promote the complete value before applying offsets.
Whole-input UTF-8 validation follows unknown-parameter admission; consumed
word/rest values are validated again after byte-wise tokenization.

`Data::entries` preserves first-definition order through replacement.
Values store typed contiguous leaves and cumulative offsets for each array
level, preserving ragged rows, empty arrays, value types, and depths.
`Data::value(name, path)` selects a borrowed value; `int(name)` reads an
integer scalar, and `ints(name, path)` returns a borrowed integer row.
`Value::leaves` borrows the typed leaves and `Value::levels` the offsets of each
level, outermost first. `Value::from_ints`, `from_floats` and `from_strings`
build a value from such columns, `Entry::new` names it, and
`Data::from_entries` collects entries in order. They return `ColumnError`
instead of panicking when a level does not start at 0, offsets decrease, a
level's length does not match the rows above it, the leaf count does not match,
a float is not finite, or two entries share a name. The TypeScript binding uses
them to move data across the WebAssembly boundary.
`ValueRef::size` returns an array row's element count in constant time from its
stored offsets, and returns `AccessError::ExpectedArray` for scalars.
Expression evaluation retains borrowed row views internally; only a result
stored by a statement copies its selected subtree and rebases its offsets.
Comprehension bindings use semantic definition identities, and free names
are cached only while the value frames remain unchanged. Accessor misuse returns
`AccessError`, separate from language failures.

`Failure` distinguishes language `Error`, host acquisition/resource failures,
and reached features awaiting later tasks. `Error` preserves registered
codes, input/spec locations and raw spans, context, and nullable record paths.
Its display renders runtime excerpts and byte-position carets; `to_json`
renders the canonical error envelope. Data JSON uses shortest round-tripping
float digits with the Specification's notation choice and signed-zero rule.

Each `Spec` owns a flat execution plan for blocks, lists, repeats and
conditionals, with statement ranges and source lines prepared at load time.
Other statements retain their existing helpers; their bodies enter the same
planned blocks. Repeat and sections continuations use an explicit work stack.
A repeat whose body contains one list calls its prepared descriptor directly,
while sharing ordinary iteration admission, record metadata and column
collection. Its bound and output-entry handles live only for that active repeat
and resolve on first use. Bounds still read current frame values at each
statement; unsupported expressions use the ordinary evaluator. The first actual
publication establishes an entry's position, and clearing a local frame retains
only its entries and reusable buffers. Other bodies keep ordinary frame lookup.

Expressions use a continuation stack; check traversal, record selection, marker
summaries, and witness links are iterative.
Model ranges are indexed once per `Spec`. Row selections retain parent cursors
and lazy leaf boundaries instead of reconstructing values or paths. Spec
strings and alphabet options are prepared once per `Spec`; read-time copies
and diagnostic quotations use the same cancellation checkpoints. Pattern
metadata is decoded once per `Spec`;
matching borrows one prepared physical line for every candidate arm. Failed
candidates publish nothing, and selected captures validate strings before
publishing names and checking ranges. Positions use incremental cursor and
line-relative offsets. Private record metadata retains start positions and
parent/row identities independently of field values; `as` and `each` create no entries.
Column construction and JSON writing iterate over flat array levels, including
arrays whose depth exceeds the parser's block-nesting limit.

False checks preserve the check statement range and saved record starts. Their
paths contain lifted indices followed by retained failed `all` indices; active
execution indices and explicit record indices are omitted. False checks
without an indexed witness have `[]`; hard evaluation failures have a null
path. Successful checks add no data entries.

Parameters retain caller order and exact strings. Defaults execute even when
overridden; conversion uses the default's leaf type. Declared names and supplied
keys are indexed, so admission and repeated assignments avoid linear searches.
Each instance runs in a local frame; ordinal and exact-name selection return
when matched. Catalog traverses all instances. Only false checks and missing or
repeated sections arms can be deferred; the first diagnostic and summary survive
until completion. Missing arms at EOF and hard errors remain immediate.
Instance record bindings are restored after skipped passes, with occurrence
identities separating cached marker summaries. Execution retains record metadata
in flat vectors, with work proportional to executed reads and statements.

Each read or catalog operation enforces the two work budgets in [Specification
§10](../../spec/0.1/10-work-budgets.md): 2^20 idle units and 2^20 + 16*N
expression units, where N counts all original input bytes. Counters span the
prefix and every traversed instance, including deferred rejections; each
separate operation starts fresh. Equality succeeds, and an excess charge
immediately reports `E-BUDGET` at the current cursor with the active statement
and execution context. Budget failures are hard errors. The reader owns both
remaining allowances; evaluators charge the shared expression allowance while
borrowing values. Each charge uses checked `u128` subtraction, with constant
accounting work and no allocation or rescan. `Data::write_dzn(&mut impl
io::Write)` streams the exact bytes in [Specification appendix
B](../../spec/0.1/appendix-b-dzn.md). `Data::to_dzn()` returns an owned byte
vector, matching `to_json`'s buffered interface. Both return `io::Result`;
stream errors can leave partial bytes in the supplied destination. Callers that
need a complete document before publication should buffer it, as the
conformance driver does.

The writer preserves entry order, quotes names and UTF-8 bytes, and chooses
matrix, arrayNd, or nested one-tuple syntax from stored offsets. Shape checks
inspect every existing row at each level, including rows under different
parents. Empty extents come from data. An explicit stack handles deep arrays;
no traversal recurses with depth. Each entry and leaf is emitted once, without
rescanning earlier output. Work is linear in stored data and output size,
with O(depth) temporary storage. Rendering consumes no language work budget.

Float formatting searches increasing significant-digit precisions with Rust's
round-to-nearest, ties-to-even formatter and parser. Exact power-of-ten
comparisons choose the starting precision; notation follows appendix B.3,
including signed zero, subnormals, and the appendix C.4 correction just below
1e17. Each binary64 value needs at most 17 conversions. No dependency is added.

From `rust/`:

```sh
cargo test --locked -p iris-core
cargo test --locked -p iris-core budget -- --nocapture
cargo test --locked -p iris-cli --test read_conformance -- --nocapture
```

Conformance is the main semantic coverage. A focused column-access test checks
borrowed ragged rows, selected-subtree copying, and empty depth/type
preservation. The CLI's ignored `scalar_differential` test compares 256 scalar
and 384 array/read probes, plus 384 repeat probes at seed 73005 and 384
line/sections probes at seed 73006 and 512 checks probes at seed 73007 and 512
params/instances/catalog probes at seed 73008 plus 640 dzn probes at seed 73010
(3,072 total), with the pinned Gecode adapter; see the [CLI
README](../iris-cli/README.md#conformance-driver).

The full conformance test runs all 932 cases (155 parse, 730 read, 28 catalog,
19 dzn) without exemptions. Budget tests cover exact limits, atomic charges,
arithmetic overflow, fresh operations, and hard errors after deferred
rejections. `budgets_scale_linearly` bounds work for 4× problem sizes by 6×
work, and `budget_exhaustion_stops_work_promptly` checks that enormous repeats
stop at the same charge and work count as minimally excessive ones.
`checks_scale_linearly` counts actual dispatches, metadata advances, lookups,
copies, leaves, and witness links. The ignored release measurement
`check_doubling_measurements` prints read/collection and evaluator timings
separately across record count, row width, depth, expression size, and check
count. It parses each program before measurement.

`instances_scale_linearly` compares 4× inputs against a 6× work bound for
ordinal/name selection, catalog, deferred rejections, instance size, and parameter
count. `instance_doubling_measurements` prints release timings for instance count,
instance size, and parameter count. Separate-execution tests reuse a `Spec` after
a failed read and after catalog.

`dzn_work_scales_linearly` checks 4× inputs against a 6× work bound for
entries, leaves, float conversions, string bytes, rows, and empty/deep arrays.
Counters include output bytes, inspected offsets, stack steps, leaves, and
precision attempts. The deep writer test renders 65,536 dimensions on a 128 KiB
thread stack. Output-failure tests exercise every byte boundary of a nested
document.

`Data::write_json` streams the same canonical bytes as `to_json`, including the
final newline, into a caller-supplied writer. Writer errors propagate; hosts
that need atomic output must buffer until rendering succeeds.
