# Lint candidates

This file collects evidence for lints: mistakes found in valid reader specs
that a tool could have flagged. Milestone M6b reviewed it and decided on no
linter for now, because it held no candidates (`PLAN.md` §8.13 and §12). Keep
adding entries: candidates that meet the rule reopen the decision. A candidate
counts only if the mistake was made in a real spec, such as a format library
spec, one of Gecode's examples, a skill eval attempt, or a user report; if the
Specification cannot make it an error; and if the check gives no false warnings
on the format library.

Each entry names the mistake, the spec where it happened, how a tool would
detect it, and any false warnings it would give. A mistake that comes from
confusing Iris 0.1 behavior may belong in [1.0 ideas](1.0-ideas.md) as
well.

## Candidates

None yet. The first candidate, `size` of a list column used as a per-row
emptiness test, was removed when
[ADR 0016](adr/0016-size-lifts-like-count.md) made `size` lift like `count`.
