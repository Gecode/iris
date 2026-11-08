"""The hand-written builders in ``python/examples``, one per format in
``formats/``.

The installed ``iris conformance`` runner judges every case of a format through
tests/format_builder_driver.py, which answers with the spec the format's builder
builds instead of the case's own. So each case is judged as the Rust test
rust/gecode-iris/tests/format_builders.rs judges it: the same case loading,
parameters, operation, and comparison of data or error.
"""

from __future__ import annotations

import re
import shlex
import shutil
import subprocess
import sys
import sysconfig
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
FORMATS = ROOT / "formats"
DRIVER = Path(__file__).resolve().parent / "format_builder_driver.py"
BUILDERS = ["dimacs-cnf", "jobshop", "nonogram", "sudoku", "tsplib", "tsptw"]


def test_there_is_one_builder_per_format() -> None:
    assert sorted(path.name for path in FORMATS.iterdir() if path.is_dir()) == BUILDERS
    builders = sorted(path.stem for path in (ROOT / "python/examples").glob("build_*.py"))
    assert builders == sorted(f"build_{name.replace('-', '_')}" for name in BUILDERS)


@pytest.mark.parametrize("format_name", BUILDERS)
def test_every_case_reads_through_the_builder(format_name: str) -> None:
    script = shutil.which("iris", path=sysconfig.get_path("scripts"))
    assert script is not None, "the iris console script is not installed"
    driver = shlex.join([sys.executable, "-I", str(DRIVER), format_name])
    result = subprocess.run(
        [script, "conformance", driver, f"formats/{format_name}"],
        cwd=ROOT,
        capture_output=True,
        check=False,
        text=True,
        encoding="utf-8",
    )
    assert result.returncode == 0, result.stdout + result.stderr
    summary = re.search(r"^Summary: total=(\d+) pass=(\d+) ", result.stdout, re.MULTILINE)
    assert summary is not None, result.stdout
    total, passed = int(summary[1]), int(summary[2])
    assert total >= 2 and passed == total, result.stdout
