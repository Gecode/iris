"""The Spec and Data API: parsing, reading, catalogs, parameters and rendering."""

from __future__ import annotations

import json
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from typing import Any

import pytest

import gecode_iris as iris

REPOSITORY = Path(__file__).resolve().parents[2]
ROW_SPEC = "n:int row[n]:int"
ROW_JSON = (
    '{"iris":"0.1","entries":['
    '{"name":"n","type":"int","depth":0,"value":3},'
    '{"name":"row","type":"int","depth":1,"value":[10,20,30]}]}\n'
)
NAMED_INSTANCES_SPEC = """
instances {
  line "instance {name:word}"
  value: int
}
"""
NAMED_INSTANCES_INPUT = b"instance first\n1\ninstance second\n2\n"


def read_row(spec: iris.Spec) -> str:
    return spec.read(b"3 10 20 30").to_json()


def test_parse_from_str() -> None:
    assert read_row(iris.Spec.parse(ROW_SPEC)) == ROW_JSON


def test_parse_from_bytes() -> None:
    assert read_row(iris.Spec.parse(ROW_SPEC.encode())) == ROW_JSON


def test_parse_origin_defaults_to_string_marker() -> None:
    with pytest.raises(iris.Error) as raised:
        iris.Spec.parse("x:integer")
    assert raised.value.origin == "<string>"


def test_parse_uses_the_given_origin() -> None:
    with pytest.raises(iris.Error) as raised:
        iris.Spec.parse("x:integer", origin="inline.iris")
    assert raised.value.origin == "inline.iris"


def test_parse_reports_invalid_utf8_in_bytes() -> None:
    with pytest.raises(iris.Error) as raised:
        iris.Spec.parse(b"n:int # \xff")
    assert raised.value.code == "E-SPEC-UTF8"


def test_load_uses_the_path_as_origin(tmp_path: Path) -> None:
    path = tmp_path / "row.iris"
    path.write_text(ROW_SPEC)
    assert read_row(iris.Spec.load(path)) == ROW_JSON
    path.write_text("x:integer")
    with pytest.raises(iris.Error) as raised:
        iris.Spec.load(str(path))
    assert raised.value.origin == str(path)


def test_read_from_str_and_pathlib_paths(tmp_path: Path) -> None:
    path = tmp_path / "row.txt"
    path.write_bytes(b"3 10 20 30")
    spec = iris.Spec.parse(ROW_SPEC)
    assert spec.read(str(path)).to_json() == ROW_JSON
    assert spec.read(path).to_json() == ROW_JSON


def test_read_path_uses_it_as_origin(tmp_path: Path) -> None:
    path = tmp_path / "short.txt"
    path.write_bytes(b"3 10")
    with pytest.raises(iris.Error) as raised:
        iris.Spec.parse(ROW_SPEC).read(path)
    assert raised.value.origin == str(path)


def test_read_bytes_origin_defaults_to_input_marker() -> None:
    with pytest.raises(iris.Error) as raised:
        iris.Spec.parse(ROW_SPEC).read(b"3 10")
    assert raised.value.origin == "<input>"


def test_read_bytes_uses_the_given_origin() -> None:
    with pytest.raises(iris.Error) as raised:
        iris.Spec.parse(ROW_SPEC).read(b"3 10", origin="row.txt")
    assert raised.value.origin == "row.txt"


def test_read_rejects_origin_with_a_path(tmp_path: Path) -> None:
    with pytest.raises(TypeError, match="origin"):
        iris.Spec.parse(ROW_SPEC).read(tmp_path / "row.txt", origin="row.txt")


def test_missing_file_raises_file_not_found(tmp_path: Path) -> None:
    missing = tmp_path / "missing.txt"
    with pytest.raises(FileNotFoundError) as raised:
        iris.Spec.parse(ROW_SPEC).read(missing)
    assert raised.value.filename == str(missing)
    with pytest.raises(FileNotFoundError):
        iris.Spec.load(missing)


def test_catalog_lists_named_instances() -> None:
    catalog = json.loads(
        iris.Spec.parse(NAMED_INSTANCES_SPEC).catalog(NAMED_INSTANCES_INPUT).to_json()
    )
    entries = {entry["name"]: entry["value"] for entry in catalog["entries"]}
    assert entries["instances"] == 2
    assert entries["name"] == ["first", "second"]


def test_params_select_an_instance() -> None:
    spec = iris.Spec.parse(NAMED_INSTANCES_SPEC)
    data = json.loads(spec.read(NAMED_INSTANCES_INPUT, params={"instance": "second"}).to_json())
    entries = {entry["name"]: entry["value"] for entry in data["entries"]}
    assert entries == {"name": "second", "value": 2}


def test_params_pass_values_as_exact_strings() -> None:
    spec = iris.Spec.parse('param code = ""')
    data = json.loads(spec.read(b"", params={"code": "0007"}).to_json())
    assert data["entries"][0] == {"name": "code", "type": "string", "depth": 0, "value": "0007"}


@pytest.mark.parametrize(
    ("params", "message"),
    [
        ({"n": 3}, "parameter 'n' must be a str, not int"),
        ({3: "n"}, "parameter names must be str, not int"),
        ([("n", "3")], "params must be a mapping"),
    ],
)
def test_params_must_map_str_to_str(params: Any, message: str) -> None:
    spec = iris.Spec.parse("param n = 1\nm:int")
    with pytest.raises(TypeError, match=message):
        spec.read(b"1", params=params)


def test_unknown_param_is_a_language_error() -> None:
    with pytest.raises(iris.Error) as raised:
        iris.Spec.parse(ROW_SPEC).read(b"3 10 20 30", params={"m": "1"})
    assert raised.value.code == "E-PARAM-UNKNOWN"


def test_to_json_matches_a_format_case() -> None:
    case = REPOSITORY / "formats" / "dimacs-cnf" / "cases" / "three-variables"
    data = iris.Spec.load(case / "spec.iris").read(case / "input.txt")
    assert data.to_json() == (case / "expected.json").read_text(encoding="utf-8")


def test_to_dzn_matches_a_conformance_case() -> None:
    case = REPOSITORY / "conformance" / "cases" / "0.1" / "smoke" / "dzn"
    params = json.loads((case / "params.json").read_text(encoding="utf-8"))
    data = iris.Spec.load(case / "spec.iris").read(case / "input.bin", params=params)
    assert data.to_dzn() == (case / "expected.dzn").read_text(encoding="utf-8")


def test_one_spec_reads_concurrently() -> None:
    spec = iris.Spec.parse(ROW_SPEC)
    with ThreadPoolExecutor(max_workers=8) as pool:
        results = list(pool.map(lambda _: read_row(spec), range(64)))
    assert results == [ROW_JSON] * 64
