"""Render the Python API reference for the site, from the stubs and docstrings.

Run it with pdoc and the built ``gecode_iris`` package installed:

    python docs/reference.py OUTPUT.json

The output holds the reference as an HTML fragment and the headings of its
outline. The site's ``scripts/prepare-python-api.mjs`` runs this script during
every build.
"""

from __future__ import annotations

import ast
import json
import re
import sys
import warnings
from collections.abc import Generator
from contextlib import contextmanager
from html import unescape
from pathlib import Path
from types import GenericAlias, ModuleType
from typing import cast
from unittest import mock

import pdoc.doc
import pdoc.doc_ast
import pdoc.doc_pyi
import pdoc.render

import gecode_iris
import gecode_iris._cli
import gecode_iris._data
import gecode_iris._error

TEMPLATES = Path(__file__).parent / "templates"
HEADING = re.compile(r'<h([23]) id="([^"]+)">(.*?)</h\1>', re.DOTALL)
STUB = "gecode_iris._native"


def named(module: str, name: str) -> type:
    """A stand-in type that annotations show as ``module.name``, and that takes
    type arguments like a generic."""
    return type(name, (), {"__module__": module, "__class_getitem__": classmethod(GenericAlias)})


def numpy_stand_in() -> dict[str, ModuleType]:
    """Modules that give the stub's ``numpy.typing`` import a named ``NDArray``.

    numpy is optional, and with these stand-ins the output is the same with and
    without it.
    """
    numpy_typing = ModuleType("numpy.typing")
    numpy_typing.__dict__["NDArray"] = named("numpy.typing", "NDArray")
    return {"numpy": ModuleType("numpy"), "numpy.typing": numpy_typing}


def apply_stub(package: pdoc.doc.Module) -> None:
    """Give ``Spec`` and ``Data`` the signatures and docstrings of ``_native.pyi``.

    These classes come from the compiled module, whose own signatures have no
    types. pdoc reads only the stub of the module it documents, and the classes
    are documented in the package, so apply the stub here. ``Value`` becomes a
    named alias, so that signatures name it instead of expanding it.
    """
    path = pdoc.doc_pyi.find_stub_file(STUB)
    if path is None:
        raise SystemExit(f"no stub file for {STUB}")
    stand_ins = numpy_stand_in()
    with mock.patch.dict(sys.modules, stand_ins):
        stub = pdoc.doc_pyi._import_stub_file(STUB, path)
    stub.__dict__["Value"] = named("gecode_iris", "Value")
    # With the stub in sys.modules, pdoc does not import it a second time.
    with mock.patch.dict(sys.modules, {**stand_ins, STUB: stub}):
        stub_documentation = pdoc.doc.Module(stub)
        for name in ("Spec", "Data"):
            member = package.get(name)
            assert member is not None, name
            pdoc.doc_pyi._patch_doc(member, stub_documentation)


def class_source(module: ModuleType, name: str) -> str:
    """The source of a top-level class, read from the file of its module."""
    assert module.__file__ is not None
    text = Path(module.__file__).read_text(encoding="utf-8")
    tree = ast.parse(text)
    node = next(n for n in tree.body if isinstance(n, ast.ClassDef) and n.name == name)
    source = ast.get_source_segment(text, node)
    assert source is not None, name
    return source


@contextmanager
def defining_sources() -> Generator[None]:
    """Let pdoc read ``Entry`` and ``Error`` from the modules that define them.

    The package sets their ``__module__`` to ``gecode_iris``, so
    ``inspect.getsource`` looks for them in the package's ``__init__.py``, and
    pdoc would miss the docstrings below their attributes.
    """
    sources: dict[object, str] = {
        gecode_iris.Entry: class_source(gecode_iris._data, "Entry"),
        gecode_iris.Error: class_source(gecode_iris._error, "Error"),
    }
    original = pdoc.doc_ast._get_source

    def get_source(obj: object) -> str:
        try:
            known = obj in sources
        except TypeError:  # unhashable, so not cached either
            return original.__wrapped__(obj)
        return sources[obj] if known else original(obj)

    with mock.patch.object(pdoc.doc_ast, "_get_source", get_source):
        yield


def render() -> dict[str, object]:
    pdoc.render.configure(
        docformat="restructuredtext",
        include_undocumented=False,
        search=False,
        show_source=False,
        template_directory=TEMPLATES,
    )
    package = pdoc.doc.Module(gecode_iris)
    # Value is a re-exported module attribute, documented where it is defined.
    value = package.get("Value")
    assert value is not None
    value.docstring = pdoc.doc_ast.walk_tree(gecode_iris._data).var_docstrings["Value"]
    template_globals = cast(dict[str, object], pdoc.render.env.globals)
    template_globals["command"] = pdoc.doc.Module(gecode_iris._cli)
    with defining_sources():
        apply_stub(package)
        html = pdoc.render.html_module(package, {package.modulename: package}).strip()
    headings = [
        {"depth": int(depth), "slug": slug, "text": unescape(re.sub(r"<[^>]+>", "", text))}
        for depth, slug, text in HEADING.findall(html)
    ]
    return {"pdoc": pdoc.__version__, "headings": headings, "html": html}


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: reference.py OUTPUT.json")
    # pdoc evaluates Entry's annotations in the package, which does not import
    # Literal, and then shows them as written, which is what the reference wants.
    warnings.filterwarnings("ignore", r"Error parsing type annotation .*Literal\[", UserWarning)
    Path(sys.argv[1]).write_text(json.dumps(render(), indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
