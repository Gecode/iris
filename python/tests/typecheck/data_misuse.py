"""Misuses the API; pyright and mypy report an error on each marked line.

The type checkers read this file and never run it.
"""

from __future__ import annotations

import gecode_iris as iris

spec = iris.Spec.parse("n:int row[n]:int")
data = spec.read(b"3 10 20 30")

iris.Spec.parse(3)  # error: the text is str or bytes
spec.read(b"1", params={"n": 3})  # error: parameter values are str
spec.read(b"1", origin=None, strict=True)  # error: no such keyword
total = data["n"] + 1  # error: a value may be a str or a list
row: list[str] = data.ints("row")  # error: ints returns list[int]
data.ints("row", "0")  # error: path indices are int
data.ints(name="row")  # error: the name is positional
data[0]  # error: entry names are str
entries: list[iris.Entry] = data.entries  # error: entries is a tuple
iris.Entry("n", "integer", 0)  # error: the value type is int, float or string
code: int = iris.Error("text", "E", "m", None, None, None, None, "{}").code  # error: str
