"""Data access: entries, item access, ints, array and to_dict."""

from __future__ import annotations

import pickle
import sys
from typing import Any, Literal

import pytest

import gecode_iris as iris

# Every value type at depths 0 to 3; `r` is ragged.
SPEC = """
i0:int  f0:float  s0:word
i1[2]:int  f1[2]:float  s1[2]:word
i2[2][3]:int  f2[2][2]:float  s2[2][2]:word
repeat 2 { i3[2][2]:int  f3[1][2]:float  s3[2][1]:word }
repeat 2 { k:int  r[k]:int }
"""
INPUT = b"""
7 2.5 hello
1 2  0.5 1.5  a b
1 2 3 4 5 6  1 2 3 4  w x y z
1 2 3 4  0.25 0.75  p q
5 6 7 8  1.25 1.75  s t
1 9  2 10 11
"""
VALUES: dict[str, Any] = {
    "i0": 7,
    "f0": 2.5,
    "s0": "hello",
    "i1": [1, 2],
    "f1": [0.5, 1.5],
    "s1": ["a", "b"],
    "i2": [[1, 2, 3], [4, 5, 6]],
    "f2": [[1.0, 2.0], [3.0, 4.0]],
    "s2": [["w", "x"], ["y", "z"]],
    "i3": [[[1, 2], [3, 4]], [[5, 6], [7, 8]]],
    "f3": [[[0.25, 0.75]], [[1.25, 1.75]]],
    "s3": [[["p"], ["q"]], [["s"], ["t"]]],
    "k": [1, 2],
    "r": [[9], [10, 11]],
}
VALUE_TYPES: list[tuple[str, Literal["int", "float", "string"]]] = [
    ("i", "int"),
    ("f", "float"),
    ("s", "string"),
]
RECTANGULAR = ["i1", "f1", "s1", "i2", "f2", "s2", "i3", "f3", "s3", "k"]


@pytest.fixture(scope="module")
def data() -> iris.Data:
    return iris.Spec.parse(SPEC).read(INPUT)


@pytest.fixture
def without_numpy(monkeypatch: pytest.MonkeyPatch) -> None:
    """Makes `import numpy` fail, whether or not numpy is installed."""
    monkeypatch.setitem(sys.modules, "numpy", None)


def test_entries_name_value_type_and_depth_in_order(data: iris.Data) -> None:
    expected = [
        iris.Entry(f"{prefix}{depth}", value_type, depth)
        for depth in range(4)
        for prefix, value_type in VALUE_TYPES
    ]
    expected += [iris.Entry("k", "int", 1), iris.Entry("r", "int", 2)]
    assert list(data.entries) == expected


def test_entry_is_a_named_tuple() -> None:
    entry = iris.Entry("row", "int", 1)
    name, value_type, depth = entry
    assert (entry.name, entry.value_type, entry.depth) == (name, value_type, depth)
    assert repr(entry) == "Entry(name='row', value_type='int', depth=1)"
    assert pickle.loads(pickle.dumps(entry)) == entry


@pytest.mark.parametrize("name", list(VALUES))
def test_item_access_returns_scalars_and_nested_lists(data: iris.Data, name: str) -> None:
    value = data[name]
    assert value == VALUES[name]
    assert type(value) is type(VALUES[name])


def test_float_entries_hold_floats_for_integer_input(data: iris.Data) -> None:
    rows: Any = data["f2"]
    assert [type(x) for row in rows for x in row] == [float] * 4


def test_item_access_raises_key_error_for_unknown_name(data: iris.Data) -> None:
    with pytest.raises(KeyError, match="missing"):
        data["missing"]


def test_contains_tests_entry_names(data: iris.Data) -> None:
    assert "i0" in data
    assert "missing" not in data
    assert 0 not in data


def test_to_dict_has_every_entry_in_order(data: iris.Data) -> None:
    result = data.to_dict()
    assert result == VALUES
    assert list(result) == list(VALUES)


def test_ints_returns_a_whole_row(data: iris.Data) -> None:
    assert data.ints("i1") == [1, 2]
    assert data.ints("k") == [1, 2]


def test_ints_follows_a_path(data: iris.Data) -> None:
    assert data.ints("i2", 1) == [4, 5, 6]
    assert data.ints("i3", 1, 0) == [5, 6]
    assert data.ints("r", 1) == [10, 11]


def test_ints_counts_negative_indices_from_the_end(data: iris.Data) -> None:
    assert data.ints("i2", -1) == [4, 5, 6]
    assert data.ints("i3", -2, -1) == [3, 4]


@pytest.mark.parametrize(
    ("name", "path", "error", "message"),
    [
        ("missing", (), KeyError, "missing"),
        ("i2", (2,), IndexError, r"entry 'i2' at path \[2\]: path index out of bounds"),
        ("i2", (-3,), IndexError, r"entry 'i2' at path \[-3\]: path index out of bounds"),
        ("i0", (0,), TypeError, r"entry 'i0' at path \[0\]: expected an array"),
        ("i0", (), TypeError, "entry 'i0': expected an array"),
        (
            "i2",
            (),
            TypeError,
            "entry 'i2': expected a row of integers, but the selection has depth 2",
        ),
        ("f1", (), TypeError, "entry 'f1': expected integer leaves"),
        ("s2", (0,), TypeError, r"entry 's2' at path \[0\]: expected integer leaves"),
    ],
)
def test_ints_rejects_bad_names_paths_and_types(
    data: iris.Data, name: str, path: tuple[int, ...], error: type[Exception], message: str
) -> None:
    with pytest.raises(error, match=message):
        data.ints(name, *path)


@pytest.mark.parametrize("name", RECTANGULAR + ["r"])
def test_array_without_numpy_returns_nested_lists(
    data: iris.Data, without_numpy: None, name: str
) -> None:
    assert data.array(name) == VALUES[name]


def test_array_without_numpy_follows_a_path(data: iris.Data, without_numpy: None) -> None:
    assert data.array("i3", 1) == [[5, 6], [7, 8]]
    assert data.array("s3", -1, 0) == ["s"]


@pytest.mark.parametrize(
    ("name", "path", "error", "message"),
    [
        ("missing", (), KeyError, "missing"),
        ("i0", (), TypeError, "entry 'i0': expected an array"),
        ("i1", (0,), TypeError, r"entry 'i1' at path \[0\]: expected an array"),
        ("i2", (0, 5), IndexError, r"entry 'i2' at path \[0, 5\]: path index out of bounds"),
    ],
)
def test_array_rejects_bad_names_and_paths(
    data: iris.Data, name: str, path: tuple[int, ...], error: type[Exception], message: str
) -> None:
    with pytest.raises(error, match=message):
        data.array(name, *path)


@pytest.mark.parametrize("name", RECTANGULAR)
def test_array_with_numpy_returns_a_rectangular_ndarray(data: iris.Data, name: str) -> None:
    numpy = pytest.importorskip("numpy")
    array = data.array(name)
    assert isinstance(array, numpy.ndarray)
    assert array.tolist() == VALUES[name]
    expected_kind = {"i": "i", "f": "f", "s": "U", "k": "i"}[name[0]]
    assert array.dtype.kind == expected_kind
    assert array.ndim == next(e.depth for e in data.entries if e.name == name)


def test_array_with_numpy_owns_writable_memory(data: iris.Data) -> None:
    numpy = pytest.importorskip("numpy")
    array = data.array("i2")
    assert isinstance(array, numpy.ndarray)
    assert array.dtype == numpy.int64
    array[0, 0] = 100
    assert data.ints("i2", 0)[0] == 1


def test_array_with_numpy_follows_a_path(data: iris.Data) -> None:
    numpy = pytest.importorskip("numpy")
    array = data.array("f3", 1)
    assert isinstance(array, numpy.ndarray)
    assert array.shape == (1, 2)
    assert array.tolist() == [[1.25, 1.75]]


def test_array_with_numpy_returns_lists_for_ragged_arrays(data: iris.Data) -> None:
    pytest.importorskip("numpy")
    assert data.array("r") == [[9], [10, 11]]


def test_array_with_numpy_returns_lists_when_the_first_row_is_empty() -> None:
    pytest.importorskip("numpy")
    data = iris.Spec.parse("repeat 2 { k:int  r[k]:float }").read(b"0 1 2.5")
    assert data.array("r") == [[], [2.5]]


def test_array_with_numpy_gives_empty_arrays_a_zero_shape() -> None:
    numpy = pytest.importorskip("numpy")
    data = iris.Spec.parse("n:int  w[n][3]:int  s[n]:word").read(b"0")
    for name, ndim in [("w", 2), ("s", 1)]:
        array = data.array(name)
        assert isinstance(array, numpy.ndarray)
        assert array.shape == (0,) * ndim


def test_catalog_data_supports_the_same_access() -> None:
    spec = iris.Spec.parse('instances { line "instance {name:word}"  value: int }')
    catalog = spec.catalog(b"instance first\n1\ninstance second\n2\n")
    assert [entry.name for entry in catalog.entries] == list(catalog.to_dict())


def test_repr_lists_entry_names() -> None:
    data = iris.Spec.parse("n:int row[n]:int").read(b"2 1 2")
    assert repr(data) == "<gecode_iris.Data: n, row>"
