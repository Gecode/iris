"""Misuses the builder; pyright and mypy report an error on each marked line.

The type checkers read this file and never run it.
"""

from __future__ import annotations

import gecode_iris as iris
import gecode_iris.build as ib

data: iris.Data = iris.Spec.parse("n:int").read(b"1")
b = ib.SpecBuilder()
n = b.read("n", ib.int_)
w = b.read("w", ib.word)
row = b.read("row", ib.int_.dim(n))

text: str = data[n]  # error: an int handle reads an int
count: int = data[row]  # error: an array handle reads a list
b.read("x", int)  # error: a value type is an Iris type, not a Python class
b.read(3, ib.int_)  # error: the target is a name or a handle
b.list("xs", ib.int_)  # error: a list reads until a terminator
b.let("x", n + w)  # error: arithmetic needs numbers
b.check(n > "a")  # error: an int compares with numbers
b.check(n / 2 > 0)  # error: Iris division is div() or rem()
b.check(n % 2 == 0)  # error: Iris remainder is rem()
b.repeat("n")  # error: the count is an int expression
b.line([n])  # error: a pattern is a tuple, not a list
b.forward("f", int)  # error: the kind is a field type
b.set(n, 1, 2)  # error: set takes one value
b.sections(until="END", stop=True)  # error: no such keyword
ib.div(1.5, 2)  # error: div() takes ints
with b.if_(n > 0) as then:
    maybe = then.read("maybe", ib.int_)
certain: int = data[maybe]  # error: a handle defined in an if may be absent
with b.repeat(n) as rows:
    cell = rows.read("cell", ib.int_)
scalar: int = data[cell]  # error: a handle defined in a repeat reads a list
rows.record_as("jobs")  # error: record_as takes a forward record
missing = b.record  # error: the root scope has no record
fields: ib.FloatField[float] = b.read("i", ib.int_)  # error: an int handle
spec: iris.Spec = b.source()  # error: source() returns the text
