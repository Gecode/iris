# 0005. Conformance suite and canonical JSON

## Status

Accepted, 2026-10-31.

## Context

Independent implementations need a common, observable contract. The
Specification defines the data model and canonical JSON encoding
([PLAN.md §4.3](../../PLAN.md#43-the-data-model-is-part-of-the-specification));
the versioned conformance suite checks agreement
([§7](../../PLAN.md#7-conformance-suite)).

## Decision

The conformance suite and canonical JSON are the contract between
implementations. An implementation conforms to a language version when it
passes that version's suite. Cases cover reading, parsing, and catalog
results, with expected data or errors.

Each implementation provides a conformance driver for `read`, `catalog`,
and `parse`. One runner, `iris conformance`, runs cases against any driver,
including the Gecode adapter. The CLI also emits canonical JSON.

## Consequences

The Specification remains normative; implementations expose the same
ordered entries, value types, depths, ragged arrays, 64-bit signed integers,
and paths of indices counted from 0. Canonical JSON makes their data and
errors comparable. Gecode 6.5.0 is the oracle while the 0.1 Specification is
written. Deliberate differences belong in the Specification's differences
appendix ([PLAN.md §6.1](../../PLAN.md#61-sources)).
