"""The attributes and behavior of ``iris.Error``."""

from __future__ import annotations

import json
import pickle

import pytest

import gecode_iris as iris

CHECK_SPEC = """n: int
m: int
repeat n as jobs {
  repeat m as steps { x: int }
}
check x > 0 "positive"
"""
CHECK_INPUT = b"2 3\n1 2 3\n4 5 0\n"


def parse_error() -> iris.Error:
    with pytest.raises(iris.Error) as raised:
        iris.Spec.parse("n: int\nx: integer y: int\n", origin="types.iris")
    return raised.value


def check_failure() -> iris.Error:
    spec = iris.Spec.parse(CHECK_SPEC, origin="jobs.iris")
    with pytest.raises(iris.Error) as raised:
        spec.read(CHECK_INPUT, origin="jobs.txt")
    return raised.value


def test_parse_error_attributes() -> None:
    error = parse_error()
    assert error.code == "E-SPEC-UNKNOWN-TYPE"
    assert error.message == "unknown type 'integer'"
    # Reader spec errors locate a line; many, like this one, have no column.
    assert (error.origin, error.line, error.column) == ("types.iris", 2, None)
    assert error.path is None
    assert str(error).startswith("types.iris:2: unknown type 'integer'")


def test_check_failure_attributes() -> None:
    error = check_failure()
    assert error.code == "E-CHECK-FAILED"
    assert error.message == "check failed for record 3 of 'steps' in record 2 of 'jobs': positive"
    assert (error.origin, error.line, error.column) == ("jobs.txt", 3, 5)
    assert error.path == (1, 2)
    assert str(error).startswith(f"jobs.txt:3:5: {error.message}")


def test_to_json_is_the_canonical_error_document() -> None:
    document = json.loads(check_failure().to_json())
    assert document == {
        "code": "E-CHECK-FAILED",
        "message": "check failed for record 3 of 'steps' in record 2 of 'jobs': positive",
        "input_location": {"origin": "jobs.txt", "line": 3, "column": 5},
        "spec_location": {"origin": "jobs.iris", "line": 6, "column": None},
        "context": None,
        "record_path": [1, 2],
    }


def test_errors_survive_pickling() -> None:
    error = check_failure()
    copy = pickle.loads(pickle.dumps(error))
    assert type(copy) is iris.Error
    assert str(copy) == str(error)
    fields = ("code", "message", "line", "column", "path", "origin")
    assert [getattr(copy, name) for name in fields] == [getattr(error, name) for name in fields]
    assert copy.to_json() == error.to_json()


def test_error_is_exported_from_the_package() -> None:
    assert iris.Error.__module__ == "gecode_iris"
    assert issubclass(iris.Error, Exception)
