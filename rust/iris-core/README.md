# iris-core

Input reading, scalar expression evaluation, column data, and canonical JSON
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

Execution supports scalar int, float, num, word, rest, and integer ranges;
named scalar types with frozen bounds and aliases; settings in statement
order; scalar if/else and let; and scalar operators, comparisons, truth,
functions, and host constants. Integers use checked 64-bit arithmetic.
Whole-input UTF-8 validation follows unknown-parameter admission; consumed
word/rest values are validated again after byte-wise tokenization.

`Data::entries` preserves first-definition order through replacement.
Values store typed contiguous leaves and cumulative offsets for each array
level. `Data::value(name, path)` selects a borrowed value; `int(name)` reads
an integer scalar, and `ints(name, path)` returns a borrowed integer row.
Array execution arrives in rust-004; the representation already preserves
ragged rows, empty arrays, value types, and depths. Accessor misuse returns
`AccessError`, separate from language failures.

`Failure` distinguishes language `Error`, host acquisition/resource failures,
and reached features awaiting later tasks. `Error` preserves registered
codes, input/spec locations and raw spans, context, and nullable record paths.
Its display renders runtime excerpts and byte-position carets; `to_json`
renders the canonical error envelope. Data JSON uses shortest round-tripping
float digits with the Specification's notation choice and signed-zero rule.

Later tasks add arrays, offsets, layouts, alphabets, indexing and comprehensions
(rust-004), repeat/records (005), line patterns and sections (006), checks and
record expressions (007), parameter execution and instances/catalog (008),
budget enforcement (009), and dzn output (010). Supplied parameter strings
are transported and unknown names rejected now; declarations/defaults do not
execute yet. The scalar aggregate work hook is fallible but imposes no limit.
No idle-work hook is needed until a statement can create idle work.

From `rust/`:

```sh
cargo test --locked -p iris-core
cargo test --locked -p iris-cli --test read_conformance -- --nocapture
```

Conformance is the main semantic coverage. A focused column-access test checks
borrowed ragged rows and empty depth/type preservation. The CLI's ignored
`scalar_differential` test compares bounded generated probes with the pinned
Gecode adapter; see the [CLI README](../iris-cli/README.md#conformance-driver).
