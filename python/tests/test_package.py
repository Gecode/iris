"""Packaging facts that the binding's build configuration must keep consistent."""

from __future__ import annotations

import re
from importlib.metadata import metadata
from pathlib import Path

CARGO_MANIFEST = Path(__file__).resolve().parents[1] / "Cargo.toml"


def test_requires_python_matches_the_abi3_floor() -> None:
    feature = re.search(r'"abi3-py3(\d+)"', CARGO_MANIFEST.read_text(encoding="utf-8"))
    assert feature is not None
    assert metadata("gecode-iris")["Requires-Python"] == f">=3.{feature[1]}"
