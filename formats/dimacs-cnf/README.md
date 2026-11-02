# DIMACS CNF

[dimacs-cnf.iris](dimacs-cnf.iris) reads a `p cnf` header and the declared
number of zero-terminated clauses. It skips `c` comments and stops at `%`
for SATLIB-style tails. Data contains `vars`, `clauses`, and the ragged
`clause` array; terminating zeros are consumed without being stored.

The format source is DIMACS, [Satisfiability: Suggested Format, §2.1](https://www.cs.ubc.ca/~babic/doc/dimacs_cnf.pdf).
The reader spec is copied from
[Gecode's sat.cpp](https://github.com/Gecode/gecode/blob/5e7bbd5c26b0702c6fd652356b2289da0710ac3c/examples/sat.cpp#L56).

- [three-variables](cases/three-variables/) has three variables and three
  clauses of different lengths.
- [literal-out-of-range](cases/literal-out-of-range/) declares three
  variables but uses literal `4`. The reference reports
  `E-EXPECTED-VALUE` at input line 2, column 3.

These are hand-made samples under the repository's MIT license. See the
[library README](../README.md) for case layout, validation, and Gecode notices.
