# iris-core

Input reading, expression evaluation, column data, and canonical JSON
for Iris 0.1. This is the runtime layer in
[PLAN.md §4.2](../../PLAN.md#42-the-rust-reference-implementation-is-layered),
part of milestone M3 ([§8.1](../../PLAN.md#81-rust-rust)). It depends on
`iris-syntax` and `iris-sema`; parsing and resolution use their existing APIs.

```rust
use iris_core::{Params, Spec};

let spec = Spec::parse(b"x:int let square=x^2", "scalar.iris")?;
let data = spec.read(b"7", "input.txt", &Params::new())?;
assert_eq!(data.int("square")?, 49);
let json = data.to_json()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

`Spec::parse` selects language 0.1; `parse_version` takes an explicit
`LanguageVersion`. A valid `Spec` can read several inputs independently.
`read` borrows bytes only for the duration of execution; `read_path` acquires
a file and calls it. Returned data and errors own their retained content.
The byte-slice interface also permits a future caller to use a memory map.

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
array comprehensions with scoped bindings. Integers use checked 64-bit
arithmetic. Numeric reads promote the complete value before applying offsets.
Whole-input UTF-8 validation follows unknown-parameter admission; consumed
word/rest values are validated again after byte-wise tokenization.

`Data::entries` preserves first-definition order through replacement.
Values store typed contiguous leaves and cumulative offsets for each array
level, preserving ragged rows, empty arrays, value types, and depths.
`Data::value(name, path)` selects a borrowed value; `int(name)` reads an
integer scalar, and `ints(name, path)` returns a borrowed integer row.
Expression indexing copies the selected subtree and rebases its offsets;
let owns its result before replacing an entry. Accessor misuse returns
`AccessError`, separate from language failures.

`Failure` distinguishes language `Error`, host acquisition/resource failures,
and reached features awaiting later tasks. `Error` preserves registered
codes, input/spec locations and raw spans, context, and nullable record paths.
Its display renders runtime excerpts and byte-position carets; `to_json`
renders the canonical error envelope. Data JSON uses shortest round-tripping
float digits with the Specification's notation choice and signed-zero rule.

Repeat and sections execution use an explicit work stack for blocks,
conditionals and nested bodies. Pattern metadata is decoded once per `Spec`;
matching borrows one prepared physical line for every candidate arm. Failed
candidates publish nothing, and selected captures validate strings before
publishing names and checking ranges. Positions use incremental cursor and
line-relative offsets. Private record metadata retains start positions and
parent/row identities independently of field values; `as` and `each` create no entries.
Column construction and JSON writing iterate over flat array levels, including
arrays whose depth exceeds the parser's block-nesting limit.

Later tasks add checks and record expressions (rust-007), parameter execution
and instances/catalog (008), budget enforcement (009), and dzn output (010). Supplied parameter
strings are transported and unknown names rejected; declarations/defaults do
not execute yet. Expression and idle-work hooks are fallible and impose no
limit. They mark the work events defined by Specification §10.

From `rust/`:

```sh
cargo test --locked -p iris-core
cargo test --locked -p iris-cli --test read_conformance -- --nocapture
```

Conformance is the main semantic coverage. A focused column-access test checks
borrowed ragged rows, selected-subtree copying, and empty depth/type
preservation. The CLI's ignored `scalar_differential` test compares 256 scalar
and 384 array/read probes, plus 384 repeat probes at seed 73005 and 384
line/sections probes at seed 73006, with the pinned Gecode adapter; see the [CLI README](../iris-cli/README.md#conformance-driver).
