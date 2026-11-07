"""Writes patterns as t-strings, which need Python 3.14; pyright and mypy accept it.

The type checkers read this file and never run it.
"""

from __future__ import annotations

from typing import assert_type

import gecode_iris as iris
import gecode_iris.build as ib


def patterns(data: iris.Data) -> None:
    b = ib.SpecBuilder()
    n = b.capture("n", ib.int_)
    name = b.capture("name")
    total = b.forward("total", ib.IntField[int])
    b.line(t"jobs = {n} name = {name}")
    b.find(t"total {ib.bind(total, ib.int_)}{ib.rest_of_line}")
    with b.sections() as arms:
        arms.arm(t"[{ib.discard()}]", optional=True)
    assert_type(data[n], int)
    assert_type(data[name], str)
    assert_type(data[total], int)
