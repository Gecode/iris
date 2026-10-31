# 0008. Monorepo layout

## Status

Accepted, 2026-10-31.

## Context

A language change can require changes to the Specification, conformance
cases, and every implementation. These need to be reviewed together
([PLAN.md §5](../../PLAN.md#5-repository-layout)).

## Decision

Keep Iris in one monorepo, using the layout in PLAN.md §5. The normative
Specification lives in `spec/`, versioned cases and their runner in
`conformance/`, and reader specs for standard formats in `formats/`.

Implementations live in `rust/`, `python/`, `cpp/`, and `java/`. Editor
support, the tree-sitter grammar, agent skill, documentation site, and
brand assets have their own component directories. `CONTEXT.md` holds the
glossary and `docs/adr/` holds architecture decisions.

## Consequences

One contribution can update the Specification, add a conformance case,
and fix every implementation. Component workflows use path filters, while
the conformance workflow checks all implementations and Gecode 6.5.0
([PLAN.md §9](../../PLAN.md#9-engineering-practices)).
