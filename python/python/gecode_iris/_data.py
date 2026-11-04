"""The Python types of what ``Data`` holds."""

from __future__ import annotations

from typing import Literal, NamedTuple, TypeAlias

Value: TypeAlias = int | float | str | list["Value"]
"""An entry value: ``int``, ``float`` or ``str`` at depth 0, and a list of
values one level shallower at each greater depth."""


class Entry(NamedTuple):
    """The name, value type and depth of one entry in ``Data``."""

    name: str
    value_type: Literal["int", "float", "string"]
    depth: int


# The package exports this class, so pickles name it there.
Entry.__module__ = "gecode_iris"
