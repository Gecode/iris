"""Line patterns as tuples of text and parts, on every Python version, and
the determinism of the printed text."""

import threading
from typing import Any, cast

import pytest

import gecode_iris.build as ib


def test_a_tuple_joins_text_and_parts() -> None:
    b = ib.SpecBuilder()
    x = b.capture("x", ib.int_)
    y = b.capture("y")
    b.line(("x = ", x, ", ", "y = ", y, ""))
    assert b.source() == 'line "x = {x:int}, y = {y}"\n'
    data = b.build().read(b"x = 1, y = two\n")
    assert (data[x], data[y]) == (1, "two")

    empty = ib.SpecBuilder()
    empty.line(())
    assert empty.source() == 'line ""\n'


def test_braces_and_dots_in_text_are_rejected() -> None:
    for text in ("a { b", "a ... b", "{", "..."):
        b = ib.SpecBuilder()
        with pytest.raises(ib.BuildError) as info:
            b.line(("", text))
            b.source()
        assert info.value.kind == "unprintable"


def test_a_pattern_part_has_a_pattern_type() -> None:
    b = ib.SpecBuilder()
    with pytest.raises(TypeError, match="pattern part"):
        b.line(cast(Any, ("x", 1)))
    with pytest.raises(TypeError, match="a pattern is"):
        b.line(cast(Any, ["x"]))
    with pytest.raises(TypeError, match="bind takes a handle"):
        ib.bind(cast(Any, "x"))
    with pytest.raises(TypeError, match="no dimensions"):
        ib.validate(ib.int_.dim(2))


def program() -> str:
    b = ib.SpecBuilder("determinism")
    n = b.capture("n", ib.at_least(1))
    b.line((n,))
    with b.repeat(n) as r:
        record, x = r.record, r.read("x", ib.int_)
    b.check(ib.sum(record, lambda a: ib.sum(record, lambda c: a[x].dynamic() * c[x])) >= 0)
    return b.source()


def test_the_same_program_prints_the_same_text() -> None:
    assert program() == program()


def test_concurrent_builders_print_the_same_text() -> None:
    expected = program()
    texts: list[str] = []

    def run() -> None:
        texts.extend(program() for _ in range(200))

    threads = [threading.Thread(target=run) for _ in range(2)]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join()
    assert texts == [expected] * 400
