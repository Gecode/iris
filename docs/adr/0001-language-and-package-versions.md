# 0001. Language and package versions

## Status

Accepted, 2026-10-31.

## Context

Iris starts with Gecode 6.5.0's reader language and develops it further for
Gecode 7. Language versions and package versions describe different things
([PLAN.md §1](../../PLAN.md#1-versions),
[§9](../../PLAN.md#9-engineering-practices)).

## Decision

Iris 0.1 records the Gecode 6.5.0 language without new features. The
Specification documents deliberate differences, including
[UTF-8 strings](0011-utf8-strings.md), as required by
[PLAN.md §6.1](../../PLAN.md#61-sources).
Iris 1.0 develops the language further and is the language of Gecode 7.

Package versions are separate and follow semantic versioning. Each package
states its supported language versions; `iris --version` prints both.
The first package releases are 0.1.0 and implement Iris 0.1. Packages reach
1.0.0 with Iris 1.0, released together with Gecode 7.

## Consequences

The 0.1 Specification is frozen after release; corrections are errata
([PLAN.md §6.4](../../PLAN.md#64-form-and-tooling)). New language features
belong to Iris 1.0. From 1.0 onward, implementations support both language
versions ([PLAN.md §10.2](../../PLAN.md#102-compatibility-between-01-and-10));
the strength of the compatibility promise remains open in
[§12](../../PLAN.md#12-decisions).
