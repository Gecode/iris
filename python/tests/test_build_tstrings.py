"""Line patterns as t-strings, on Python 3.14 and later. ``conftest.py``
skips this file on older versions, which cannot parse it."""

import pytest

import gecode_iris.build as ib


def test_a_t_string_interpolates_captures_and_parts() -> None:
    b = ib.SpecBuilder()
    x = b.capture("x", ib.int_)
    w = b.forward("w", ib.StrField[str])
    b.line(t"x = {x}, {ib.blank()} {ib.discard()}")
    b.line(t"w = {ib.bind(w, ib.word)} and {ib.ref(x)} {ib.rest_of_line}")
    assert b.source() == 'line "x = {x:int}, {} {_}"\nline "w = {w:word} and {$x} ..."\n'
    data = b.build().read(b"x = 1, a b\nw = hi and 1 tail\n")
    assert (data[x], data[w]) == (1, "hi")


def test_a_t_string_prints_as_its_tuple_does() -> None:
    b = ib.SpecBuilder()
    x = b.capture("x", ib.int_)
    b.line(t"a {x} b")
    tuple_builder = ib.SpecBuilder()
    y = tuple_builder.capture("x", ib.int_)
    tuple_builder.line(("a ", y, " b"))
    assert b.source() == tuple_builder.source()


def test_a_t_string_rejects_conversions_and_format_specs() -> None:
    b = ib.SpecBuilder()
    x = b.capture("x", ib.int_)
    with pytest.raises(TypeError, match="conversion or format spec"):
        b.line(t"{x!r}")
    with pytest.raises(TypeError, match="conversion or format spec"):
        b.line(t"{x:>4}")
