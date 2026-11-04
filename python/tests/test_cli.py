"""The installed ``iris`` console script, compared with the Rust binary's recorded output."""

from __future__ import annotations

import os
import re
import shutil
import signal
import subprocess
import sys
import sysconfig
from collections.abc import Callable
from pathlib import Path
from types import FrameType
from typing import Any

import pytest

from gecode_iris import _cli

ROOT = Path(__file__).resolve().parents[2]
DIMACS = "formats/dimacs-cnf/dimacs-cnf.iris"
DIMACS_CASES = ROOT / "formats/dimacs-cnf/cases"
JOBSHOP = "formats/jobshop/jobshop.iris"


def script() -> str:
    """Return the path of the installed console script."""
    path = shutil.which("iris", path=sysconfig.get_path("scripts"))
    assert path is not None, "the iris console script is not installed"
    return path


def iris(*args: str, stdin: bytes = b"") -> subprocess.CompletedProcess[bytes]:
    """Run the console script from the repository root, so origins are relative paths."""
    return subprocess.run(
        [script(), *args], cwd=ROOT, input=stdin, capture_output=True, check=False
    )


def test_check_reports_the_format() -> None:
    result = iris("check", DIMACS)
    assert (result.returncode, result.stdout, result.stderr) == (
        0,
        b"formats/dimacs-cnf/dimacs-cnf.iris: ok (format dimacs-cnf)\n",
        b"",
    )


def test_check_renders_a_language_error(tmp_path: Path) -> None:
    spec = tmp_path / "bad.iris"
    spec.write_text("x: array[k] of int\n", newline="\n")
    result = iris("check", str(spec))
    assert (result.returncode, result.stdout) == (1, b"")
    assert result.stderr.decode() == (
        f"{spec}:1: expected '..' or '..<' after the lower bound of a range, found 'of'\n"
        "1 | x: array[k] of int\n"
        "  |             ^\n"
    )


def test_read_json_prints_the_canonical_document() -> None:
    case = DIMACS_CASES / "three-variables"
    result = iris("read", DIMACS, str(case / "input.txt"), "--json")
    assert (result.returncode, result.stderr) == (0, b"")
    assert result.stdout == (case / "expected.json").read_bytes()


def test_read_json_reports_an_input_error() -> None:
    input_path = "formats/dimacs-cnf/cases/literal-out-of-range/input.txt"
    result = iris("read", DIMACS, input_path, "--json")
    assert (result.returncode, result.stdout) == (1, b"")
    assert result.stderr == (
        b"formats/dimacs-cnf/cases/literal-out-of-range/input.txt:2:3:"
        b" expected an int in -3..3, found '4'\n"
        b"     2 | 1 4 0\n"
        b"       |   ^\n"
        b"while reading: repeat [1 of 1] > clause\n"
        b"spec: formats/dimacs-cnf/dimacs-cnf.iris:7\n"
    )


def test_list_names_the_instances() -> None:
    result = iris("list", JOBSHOP, "formats/jobshop/cases/or-library/input.txt")
    assert (result.returncode, result.stdout, result.stderr) == (
        0,
        b"formats/jobshop/cases/or-library/input.txt: 1 instance\n  1 handmade-or\n",
        b"",
    )


def test_fmt_check_accepts_a_formatted_spec() -> None:
    result = iris("fmt", "--check", DIMACS)
    assert (result.returncode, result.stdout, result.stderr) == (0, b"", b"")


def test_fmt_check_reports_an_unformatted_spec(tmp_path: Path) -> None:
    spec = tmp_path / "spec.iris"
    spec.write_text("n:int\n", newline="\n")
    result = iris("fmt", "--check", str(spec))
    assert (result.returncode, result.stdout, result.stderr.decode()) == (
        1,
        b"",
        f"{spec}: would reformat\n",
    )
    assert spec.read_bytes() == b"n:int\n"


def test_fmt_reads_standard_input() -> None:
    result = iris("fmt", "-", stdin=b"n:int\n")
    assert (result.returncode, result.stdout, result.stderr) == (0, b"n: int\n", b"")


def test_usage_errors_name_the_program_and_exit_2() -> None:
    result = iris("read", "spec.iris")
    assert (result.returncode, result.stdout) == (2, b"")
    # The name is the file name of the script, as for the Rust binary. On
    # Windows the console-script launcher strips ".exe" from sys.argv[0].
    assert result.stderr == (
        b"error: the following required arguments were not provided:\n"
        b"  <INPUT>...\n"
        b"\n"
        b"Usage: iris read <SPEC> <INPUT>...\n"
        b"\n"
        b"For more information, try '--help'.\n"
    )


@pytest.mark.parametrize("option", ["--version", "-V"])
def test_version_goes_to_standard_output(option: str) -> None:
    result = iris(option)
    assert (result.returncode, result.stderr) == (0, b"")
    assert re.fullmatch(rb"iris \d+\.\d+\.\d+\S* \(languages: 0\.1\)\n", result.stdout)


def test_a_panic_exits_101_without_a_python_traceback() -> None:
    read_end, write_end = os.pipe()
    os.close(read_end)
    try:
        result = subprocess.run(
            [script(), "check", DIMACS],
            cwd=ROOT,
            stdout=write_end,
            stderr=subprocess.PIPE,
            check=False,
        )
    finally:
        os.close(write_end)
    assert result.returncode == 101
    assert b"panicked at" in result.stderr
    # Windows describes the closed pipe in its own words.
    broken_pipe = b"The pipe is being closed" if sys.platform == "win32" else b"Broken pipe"
    assert b"failed printing to stdout: " + broken_pipe in result.stderr
    assert b"Traceback" not in result.stderr


@pytest.mark.parametrize(
    ("inherited", "expected"),
    [
        (signal.default_int_handler, signal.SIG_DFL),
        (signal.SIG_IGN, signal.SIG_IGN),
        (signal.SIG_DFL, signal.SIG_DFL),
    ],
)
def test_sigint_handling_follows_the_inherited_handler(
    monkeypatch: pytest.MonkeyPatch,
    inherited: Callable[[int, FrameType | None], Any] | int,
    expected: object,
) -> None:
    seen: list[object] = []

    def run_cli(argv: list[str]) -> int:
        seen.append(signal.getsignal(signal.SIGINT))
        return 0

    monkeypatch.setattr(_cli, "run_cli", run_cli)
    previous = signal.signal(signal.SIGINT, inherited)
    try:
        with pytest.raises(SystemExit) as exit_info:
            _cli.main()
    finally:
        signal.signal(signal.SIGINT, previous)
    assert (exit_info.value.code, seen) == (0, [expected])
