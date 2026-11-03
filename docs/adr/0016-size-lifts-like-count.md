# 0016. `size` lifts in checks like `count`

## Status

Accepted, 2026-11-03.

## Context

In a check, a field of a completed repeat denotes one record's value, so
the check runs once per record
([§8.12](../../spec/0.1/08-expressions-and-checks.md#812-record-names-and-field-access)).
Gecode 6.5.0 exempts a direct name in `size` from this rule, but not a
direct name in `count`. After `repeat n {list clause: int until 0}`,
`check count(clause) > 0` tests each clause, while `check size(clause) > 0`
tests that there are clauses. Outside checks the two functions are
identical.

The site tutorial's DIMACS step was first written with `size` and accepted
an empty clause it meant to reject. The mistake was recorded as a lint
candidate, and the 1.0 ideas list asked for clearer whole-array access in
checks.

## Decision

A direct name in `size` resolves like any other use of the name. In a
check it lifts, exactly as in `count`, so `size` and `count` are
interchangeable everywhere. `defined` and comprehension domains keep their
whole-array rule.

The change is made in the current Specification and listed in its
differences appendix as C.5. Which language version label it ships under
is left for later.

## Consequences

A check that used `size` of a repeated field to count records now tests
each record instead. The format library's two such checks, in
`nonogram.iris` and `tsplib.iris`, now name the repeat with `as` and take
`size` of the record list. A comprehension domain also counts records, as
in `count(r in a:1)`.

The pinned Gecode reference disagrees on examples that use the old rule;
the conformance suite records each as a known failure for the Gecode
driver.

## Alternative considered

Giving `count` the whole-array rule as well would also make the two
functions agree, but it would turn the per-record tests already written
with `count` into whole-column tests, and leave the per-record reading,
which is the general rule in checks, without a short form.
