"""The examples of the site's Python guide, run against the installed package.

The site renders each file in ``python/examples`` with its recorded output, so
these tests keep the guide true. A ``.py`` example runs in this interpreter, and
a ``.sh`` example is one ``iris`` command line, run with the console script.
Both run from the repository root, since they read files under ``formats/``.
"""

from __future__ import annotations

import shlex
import shutil
import subprocess
import sys
import sysconfig
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
EXAMPLES = ROOT / "python/examples"
NEEDS_NUMPY = "# Needs numpy.\n"


def examples(suffix: str) -> list[Path]:
    return sorted(EXAMPLES.glob(f"*{suffix}"))


def expected_output(example: Path) -> str:
    return example.with_suffix(".out").read_text(encoding="utf-8")


def run(argv: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        argv, cwd=ROOT, capture_output=True, check=False, text=True, encoding="utf-8"
    )


def test_every_example_has_a_recorded_output() -> None:
    sources = {path.stem for path in [*examples(".py"), *examples(".sh")]}
    assert sources, "python/examples is empty"
    assert {path.stem for path in examples(".out")} == sources


@pytest.mark.parametrize("example", examples(".py"), ids=lambda path: path.name)
def test_python_example(example: Path) -> None:
    if example.read_text(encoding="utf-8").startswith(NEEDS_NUMPY):
        pytest.importorskip("numpy")
    result = run([sys.executable, "-I", str(example)])
    assert (result.returncode, result.stderr) == (0, "")
    assert result.stdout == expected_output(example)


@pytest.mark.parametrize("example", examples(".sh"), ids=lambda path: path.name)
def test_command_example(example: Path) -> None:
    program, *args = shlex.split(example.read_text(encoding="utf-8"))
    assert program == "iris"
    script = shutil.which("iris", path=sysconfig.get_path("scripts"))
    assert script is not None, "the iris console script is not installed"
    result = run([script, *args])
    assert (result.returncode, result.stderr) == (0, "")
    assert result.stdout == expected_output(example)
