from __future__ import annotations

from typing import NamedTuple

import gecode_iris as iris
import gecode_iris.build as ib


class DimacsCnf(NamedTuple):
    """A DIMACS CNF reader: the number of variables and one row of literals per
    clause."""

    spec: iris.Spec
    vars: ib.IntField[int]
    clause: ib.IntField[list[list[int]]]


def dimacs_cnf() -> DimacsCnf:
    b = ib.SpecBuilder("dimacs-cnf")
    b.comment("c")
    b.stop("%")  # SATLIB files end with "%" and "0"
    variables = b.capture("vars", ib.int_range(0, ib.gecode.INT_MAX))
    clauses = b.capture("clauses", ib.at_least(0))
    b.line(("p cnf ", variables, " ", clauses))
    with b.repeat(clauses) as c:
        # The terminating 0 is not stored.
        clause = c.list("clause", ib.int_range(-variables, variables).until(0))
    return DimacsCnf(b.build(), variables, clause)


def main() -> None:
    cnf = dimacs_cnf()
    data = cnf.spec.read("formats/dimacs-cnf/cases/three-variables/input.txt")
    print(f"{data[cnf.vars]} variables")
    for clause in data[cnf.clause]:
        print(clause)


if __name__ == "__main__":
    main()
