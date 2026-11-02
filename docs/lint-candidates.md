# Lint candidates

This file collects evidence for lints: mistakes found in valid reader specs
that a tool could have flagged. Milestone M6b reviews it and decides whether
Iris gets a linter (`PLAN.md` §8.13). A candidate counts only if the mistake
was made in a real spec, such as a format library spec, one of Gecode's
examples, a skill eval attempt, or a user report; if the Specification
cannot make it an error; and if the check gives no false warnings on the
format library.

Each entry names the mistake, the spec where it happened, how a tool would
detect it, and any false warnings it would give. A mistake that comes from
confusing Iris 0.1 behavior may belong in [1.0 ideas](1.0-ideas.md) as
well.

## Candidates

### `size` of a list column used as a per-row emptiness test

- **Mistake.** A draft of the site tutorial's DIMACS step
  (`site/examples/tutorial/dimacs-nonempty.iris`) meant to reject empty
  clauses and wrote `check size(clause) > 0` after `repeat clauses { list
  clause: ... until 0 }`. The spec passed `iris check`, but an input with an
  empty clause was accepted: `size` has a whole-array exception, so
  `size(clause)` counts clauses instead of running once per clause. The
  intended check is `count(clause) > 0`. Running the draft on an empty-clause
  input caught it.
- **Detection.** Warn when `size(X)` is compared with `0` or `1` by `>`,
  `>=` or `!=` (a nonemptiness test) and `X` is a list collected inside a
  `repeat`, so that `size` sees the whole column. Suggest `count(X)` for a
  per-row test.
- **False warnings.** None on the format library: `nonogram.iris` and
  `tsplib.iris` use `size` on a whole column, but compare it with a declared
  dimension by `==`, which this check does not match.
- **1.0.** Related to "Make check quantification and whole-array access
  explicit" in
  [1.0 ideas](1.0-ideas.md#make-check-quantification-and-whole-array-access-explicit).
