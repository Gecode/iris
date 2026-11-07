"""Packaging facts that the binding's build configuration must keep consistent."""

from __future__ import annotations

import ast
import re
from importlib.metadata import metadata
from importlib.resources import files
from pathlib import Path
from typing import NamedTuple

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


class StubClass(NamedTuple):
    """A class of the builder's stub: the names of its bases and its public
    members."""

    bases: list[str]
    members: set[str]


class BuilderStub(NamedTuple):
    names: set[str]
    classes: dict[str, StubClass]


def builder_stub() -> BuilderStub:
    """The public names and classes of the builder's stub. A name imported
    into the stub counts only when it is re-exported as ``import x as x``."""
    stub = files("gecode_iris.build") / "__init__.pyi"
    names: set[str] = set()
    classes: dict[str, StubClass] = {}
    for node in ast.parse(stub.read_text(encoding="utf-8")).body:
        if isinstance(node, (ast.ClassDef, ast.FunctionDef)):
            names.add(node.name)
        elif isinstance(node, ast.AnnAssign) and isinstance(node.target, ast.Name):
            names.add(node.target.id)
        elif isinstance(node, ast.Assign):
            names.update(target.id for target in node.targets if isinstance(target, ast.Name))
        elif isinstance(node, ast.ImportFrom):
            names.update(alias.name for alias in node.names if alias.asname == alias.name)
        if isinstance(node, ast.ClassDef):
            bases = [ast.unparse(base).split("[")[0] for base in node.bases]
            members = {
                member.name if isinstance(member, ast.FunctionDef) else ast.unparse(member.target)
                for member in node.body
                if isinstance(member, (ast.FunctionDef, ast.AnnAssign))
            }
            public = {name for name in members if not name.startswith("_")}
            classes[node.name] = StubClass(bases, public)
    return BuilderStub({name for name in names if not name.startswith("_")}, classes)


def test_the_builder_stub_declares_every_exported_name() -> None:
    import gecode_iris.build as ib

    assert builder_stub().names == set(vars(ib)["__all__"])


def test_the_builder_stub_declares_the_members_of_each_class() -> None:
    """The stub may declare a runtime member on the subclasses instead, with
    their precise types, but it declares nothing that the runtime lacks."""
    import gecode_iris.build as ib

    classes = builder_stub().classes

    def inherited(name: str) -> set[str]:
        bases, members = classes[name]
        return members.union(*(inherited(base) for base in bases if base in classes))

    def below(name: str) -> set[str]:
        subclasses = [sub for sub, stub in classes.items() if name in stub.bases]
        return inherited(name).union(*(below(sub) for sub in subclasses))

    for name in sorted(set(vars(ib)["__all__"]) & classes.keys()):
        cls = getattr(ib, name)
        foreign = set[str]().union(
            *(dir(base) for base in cls.__mro__ if not base.__module__.startswith(ib.__name__))
        )
        annotated = set[str]().union(
            *(vars(base).get("__annotations__", {}) for base in cls.__mro__)
        )
        runtime = {m for m in dir(cls) if not m.startswith("_") and m not in foreign}
        assert runtime <= below(name), name
        assert inherited(name) <= runtime | annotated, name
