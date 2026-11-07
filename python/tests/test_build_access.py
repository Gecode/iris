"""Typed access through handles: empty arrays, absence, and the errors a
handle from another spec can still meet."""

from collections.abc import Callable
from typing import Any, cast

import pytest

import gecode_iris.build as ib
from gecode_iris import Data


def read(build: Callable[[ib.SpecBuilder], object], data: bytes) -> Data:
    b = ib.SpecBuilder()
    build(b)
    return b.build().read(data)


def test_an_empty_array_has_the_typed_empty_view() -> None:
    handles: list[ib.Field[object]] = []

    def build(b: ib.SpecBuilder) -> None:
        handles.append(b.read("ints", ib.int_.dim(0)))
        handles.append(b.read("floats", ib.float_.dim(0)))
        handles.append(b.read("strs", ib.word.dim(0)))
        handles.append(b.read("nums", ib.num.dim(0)))
        handles.append(b.read("rows", ib.float_.dims(0, 3)))

    data = read(build, b"")
    for handle in handles:
        assert data[handle] == []


def test_a_repeat_of_nothing_collects_an_empty_array() -> None:
    b = ib.SpecBuilder()
    with b.repeat(0) as r:
        x = r.read("x", ib.word)
    assert b.build().read(b"")[x] == []


def test_rows_walk_one_level_at_a_time() -> None:
    b = ib.SpecBuilder()
    x = b.read("x", ib.float_.dims(2, 2))
    data = b.build().read(b"1 2 3 4")
    assert data[x] == [[1.0, 2.0], [3.0, 4.0]]
    assert list(data.array(x, 1)) == [3.0, 4.0]


def test_an_absent_optional_entry_is_none() -> None:
    b = ib.SpecBuilder()
    n = b.read("n", ib.int_)
    with b.if_(n) as t:
        x = t.read("x", ib.int_)
    data = b.build().read(b"0")
    assert data[x] is None
    assert x not in data
    with pytest.raises(KeyError):
        data["x"]


def test_numbers_keep_the_type_the_data_gave() -> None:
    b = ib.SpecBuilder()
    row = b.read("row", ib.num.dim(2))
    value = b.build().read(b"1 2")[row]
    assert value == [1, 2]
    assert [type(v) for v in value] == [int, int]


def test_a_handle_of_another_spec_reports_what_went_wrong() -> None:
    data = read(lambda b: b.read("x", ib.int_), b"1")
    other = ib.SpecBuilder()
    missing = other.read("missing", ib.int_)
    wrong_type = other.read("x", ib.word)
    with other.repeat(1) as r:
        wrong_depth = r.read("x2", ib.int_)
    with other.if_(1) as t:
        absent = t.read("absent", ib.int_)
    loose = other.let(wrong_type, 1)

    with pytest.raises(KeyError, match="missing"):
        data[missing]
    with pytest.raises(TypeError, match="entry 'x' does not match its handle"):
        data[wrong_type]
    assert data[absent] is None
    assert data[loose] == 1, "a dynamic handle reads any value"

    depth = ib.SpecBuilder()
    deep = depth.read("x", ib.int_.dim(1))
    with pytest.raises(TypeError, match="does not match its handle"):
        data[deep]
    assert wrong_depth.name == "x2"


def test_string_keys_are_unchanged() -> None:
    b = ib.SpecBuilder()
    x = b.read("x", ib.int_)
    data = b.build().read(b"5")
    assert data["x"] == data[x] == 5
    assert "x" in data and x in data
    assert 3 not in data
    with pytest.raises(TypeError, match="entry names or builder handles"):
        data[cast(Any, 3)]


def test_ints_and_array_take_handles() -> None:
    b = ib.SpecBuilder()
    a = b.read("a", ib.int_.dims(2, 2))
    data = b.build().read(b"1 2 3 4")
    assert data.ints(a, 1) == [3, 4]
    assert data.ints("a", 0) == [1, 2]


def test_ints_and_array_check_their_handles() -> None:
    b = ib.SpecBuilder()
    x = b.read("x", ib.int_)
    b.read(x, ib.int_.dim(2))
    data = b.build().read(b"1 2 3")
    words = ib.SpecBuilder().read("x", ib.word.dim(2))
    for access in (data.__getitem__, data.ints, data.array):
        with pytest.raises(TypeError, match="has depth 0, but the entry has depth 1"):
            access(x)
    with pytest.raises(TypeError, match="has value type string, but the entry has int"):
        data.array(words)
    assert data.ints("x") == data.ints(ib.SpecBuilder().read("x", ib.int_.dim(2))) == [2, 3]


def test_handles_print_their_names() -> None:
    b = ib.SpecBuilder()
    x = b.read("x", ib.int_)
    assert x.name == "x"
    assert repr(x) == "<IntField x>"
    assert repr(b) == "<SpecBuilder>"
