# iris-sema

Parse-time validity, name resolution, and conservative value facts for Iris 0.1.
This is the semantic layer in [PLAN.md §4.2](../../PLAN.md#42-the-rust-reference-implementation-is-layered),
part of milestone M3 ([§8.1](../../PLAN.md#81-rust-rust)). It depends only on
`iris-syntax`; it performs no I/O or expression execution.

```rust
let parsed = iris_syntax::parse(source, origin, iris_syntax::LanguageVersion::V0_1)?;
let analysis = iris_sema::analyze(&parsed);
```

`Analysis` retains at most the first language error in the order defined by
[Specification §11.2](../../spec/0.1/errors.md#112-error-order), plus a semantic
model. Invalid source can leave that model partial. The checker replays
`Parse::checks()` before global conflicts and structural expression resolution.
It never rejects unavailable ordinary names or uncertain runtime types.

The model exposes individual definitions and references in both directions,
source ranges, lexical scopes and value frames, named types, repeat fields,
record denotations, comprehension domains, and ordered check dependencies.
IDs belong to one analysis; CST handles remain owned by the model. Expressions
are queried by CST node. `Possibilities::Unknown` and finite `Known` sets keep
value type and depth uncertainty separate. Reference facts describe the selected
view; a definition inside a repeat keeps its local depth. Repeat fields retain
first-definition fallback facts separately.

Value targets follow statement order and branch joins. Sections and instances
use conservative summaries; these do not enumerate input orders or solve path
conditions. Facts describe successful values, not proof that a statement can
execute without a runtime error.

From `rust/`, `cargo test --locked -p iris-sema` checks model navigation,
uncertainty and shapes, diagnostic messages, and phase/operand ordering.
The driver conformance integration test lives in `iris-cli`.

`render_error` and `render_format_error` provide the shared source excerpts for
the CLI and browser. Checking keeps Specification error order; formatting stays
syntax-only. The CLI retains its public `render_error` re-export.
