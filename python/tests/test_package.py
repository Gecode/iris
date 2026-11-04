"""Packaging facts that the binding's build configuration must keep consistent."""

from __future__ import annotations

import ast
import re
from importlib.metadata import metadata
from importlib.resources import files
from pathlib import Path

CARGO_MANIFEST = Path(__file__).resolve().parents[1] / "Cargo.toml"


def test_requires_python_matches_the_abi3_floor() -> None:
    feature = re.search(r'"abi3-py3(\d+)"', CARGO_MANIFEST.read_text(encoding="utf-8"))
    assert feature is not None
    assert metadata("gecode-iris")["Requires-Python"] == f">=3.{feature[1]}"


# Special methods that are part of the public API.
DUNDERS = {"__getitem__", "__contains__"}


def stub_tree() -> ast.Module:
    stub = files("gecode_iris") / "_native.pyi"
    return ast.parse(stub.read_text(encoding="utf-8"))


def stub_members() -> dict[str, set[str]]:
    """The classes in the extension's stub and the public members of each."""
    tree = stub_tree()
    return {
        node.name: {
            member.name
            for member in node.body
            if isinstance(member, ast.FunctionDef)
            and (not member.name.startswith("_") or member.name in DUNDERS)
        }
        for node in tree.body
        if isinstance(node, ast.ClassDef)
    }


def test_the_package_is_typed() -> None:
    assert (files("gecode_iris") / "py.typed").is_file()


def test_the_stub_declares_every_public_name_of_the_extension() -> None:
    from gecode_iris import _native

    runtime = {
        name: {member for member in dir(cls) if not member.startswith("_") or member in DUNDERS}
        for name, cls in vars(_native).items()
        if isinstance(cls, type)
    }
    assert stub_members() == runtime


def test_the_stub_declares_every_function_of_the_extension() -> None:
    from gecode_iris import _native

    declared = {node.name for node in stub_tree().body if isinstance(node, ast.FunctionDef)}
    runtime = {
        name
        for name, value in vars(_native).items()
        if callable(value) and not isinstance(value, type) and not name.startswith("_")
    }
    assert declared == runtime
