"""Driver misuse, which the conformance runner never sends."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

import pytest

DRIVER = Path(__file__).with_name("conformance_driver.py")


@pytest.fixture
def spec(tmp_path: Path) -> str:
    path = tmp_path / "spec.iris"
    path.write_text("param p = 1\nn: int\n")
    return str(path)


@pytest.fixture
def instance(tmp_path: Path) -> str:
    path = tmp_path / "input.txt"
    path.write_text("3\n")
    return str(path)


def run_driver(*args: str) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(
        [sys.executable, "-I", str(DRIVER), *args], capture_output=True, check=False
    )


@pytest.mark.parametrize(
    "options",
    [
        pytest.param(["--verbose", "1"], id="unknown option"),
        pytest.param(["--version", "0.1", "--version", "0.1"], id="duplicate option"),
        pytest.param(["--params"], id="missing option value"),
        pytest.param(["--version", "1.0"], id="unsupported version"),
        pytest.param(["--format", "dzn"], id="format outside catalog"),
        pytest.param(["--params", '{"p": "1", "p": "2"}'], id="duplicate parameter"),
        pytest.param(["--params", '{"p": 1}'], id="non-string parameter"),
        pytest.param(["--params", '{"p": "\\ud800"}'], id="lone surrogate"),
    ],
)
def test_misuse_exits_2_with_empty_stdout(spec: str, instance: str, options: list[str]) -> None:
    result = run_driver("read", spec, instance, *options)
    assert (result.returncode, result.stdout) == (2, b"")
    assert result.stderr.startswith(b"conformance-driver: ")


def test_parse_rejects_parameters(spec: str) -> None:
    result = run_driver("parse", spec, "--params", '{"p": "2"}')
    assert (result.returncode, result.stdout) == (2, b"")


def test_parse_accepts_an_empty_parameter_map(spec: str) -> None:
    result = run_driver("parse", spec, "--params", "{}", "--version", "0.1")
    assert (result.returncode, result.stdout) == (0, b'{"iris":"0.1","parsed":true}\n')


@pytest.mark.parametrize(
    "args",
    [
        pytest.param([], id="no operation"),
        pytest.param(["check"], id="unknown operation"),
        pytest.param(["read"], id="missing spec"),
        pytest.param(["read", "spec.iris"], id="missing input"),
        pytest.param(["read", "spec.iris", "--params", "{}"], id="option as input"),
    ],
)
def test_missing_arguments_are_misuse(args: list[str]) -> None:
    result = run_driver(*args)
    assert (result.returncode, result.stdout) == (2, b"")
