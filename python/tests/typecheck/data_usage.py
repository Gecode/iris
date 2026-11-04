"""Uses the documented API with precise types; pyright and mypy accept it.

The type checkers read this file and never run it.
"""

from __future__ import annotations

from pathlib import Path

import gecode_iris as iris


def load(spec_path: Path, input_path: Path) -> iris.Data:
    spec = iris.Spec.load(spec_path)
    return spec.read(input_path, params={"instance": "la01"})


def parse_and_read() -> tuple[iris.Data, iris.Data]:
    spec = iris.Spec.parse("n:int row[n]:int", origin="row.iris")
    data = spec.read(b"3 10 20 30", origin="row.txt")
    catalog = iris.Spec.parse(b"n:int").catalog(str(Path("input.txt")))
    return data, catalog


def access(data: iris.Data) -> None:
    n = data["n_jobs"]
    if isinstance(n, int):
        jobs: int = n + 1
        print(jobs)
    machines: list[int] = data.ints("machine", 3)
    last_step: list[int] = data.ints("machine", -1, 0)
    weights = data.array("weight")
    if isinstance(weights, list):
        first: iris.Value = weights[0]
        print(first)
    else:
        print(weights.shape, weights.sum())
    values: dict[str, iris.Value] = data.to_dict()
    text: str = data.to_json() + data.to_dzn()
    present: bool = "name" in data
    print(machines, last_step, values, text, present)


def describe(data: iris.Data) -> list[str]:
    lines: list[str] = []
    for entry in data.entries:
        name, value_type, depth = entry
        if entry.value_type == "string":
            lines.append(f"{name}: text at depth {depth}")
        else:
            lines.append(f"{name}: {value_type}{'[]' * entry.depth}")
    return lines


def walk(value: iris.Value) -> int:
    if isinstance(value, list):
        return sum(walk(item) for item in value)
    return 1


def report(source: bytes) -> str | None:
    try:
        iris.Spec.parse("x:int").read(source)
    except iris.Error as error:
        code: str = error.code
        line: int | None = error.line
        column: int | None = error.column
        path: tuple[int, ...] | None = error.path
        origin: str | None = error.origin
        print(code, line, column, path, origin, error.message)
        return error.to_json()
    return None
