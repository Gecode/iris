# Iris 0.1 Language Specification

This is the chapter index for Iris 0.1, the reader language of Gecode 6.5.0.
The Specification is being written for milestone M1 in
[PLAN.md §6](../../PLAN.md#6-the-iris-01-specification). Files marked planned
do not yet exist; their names and chapter numbers are reserved.

| Chapter | File | Subject |
|---|---|---|
| 1 | [01-introduction.md](01-introduction.md) | Introduction |
| 2 | [02-notation.md](02-notation.md) | Notation and executable examples |
| 3 | [03-data-model.md](03-data-model.md) | Data model and canonical JSON |
| 4 | [04-input.md](04-input.md) | Input text |
| 5 | [05-reader-specs.md](05-reader-specs.md) | Reader specs |
| 6 | [06-types-and-values.md](06-types-and-values.md) | Types and value-reading statements |
| 7 | [07-structure.md](07-structure.md) | Input structure |
| 8 | [08-expressions-and-checks.md](08-expressions-and-checks.md) | Expressions, checks, and record names |
| 9 | [09-params-and-instances.md](09-params-and-instances.md) | Parameters, instances, and catalog |
| 10 | [10-work-budgets.md](10-work-budgets.md) | Work budgets |
| 11 | [errors.md](errors.md) | Error reporting and code registry |
| A | [appendix-a-grammar.md](appendix-a-grammar.md), [grammar.ebnf](grammar.ebnf) | Formal grammar |
| B | `appendix-b-dzn.md` (planned) | dzn output |
| C | [appendix-c-differences.md](appendix-c-differences.md) | Differences from Gecode 6.5.0 |
| D | `appendix-d-pinned-details.md` (planned) | Informative index of decisions for PLAN.md §6.2 |

The canonical encodings have schemas for [data](../schema/data.schema.json)
and [errors](../schema/error.schema.json). The project vocabulary is in
[CONTEXT.md](../../CONTEXT.md).
