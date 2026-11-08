"""Render the Python API reference for the site, from the stubs and docstrings.

Run it with pdoc and the built ``gecode_iris`` package installed:

    python docs/reference.py OUTPUT.json

The output holds the reference of ``gecode_iris`` and that of its spec
builder, ``gecode_iris.build``, each as an HTML fragment and the headings of
its outline. The site's ``scripts/prepare-python-api.mjs`` runs this script
during every build.
"""

from __future__ import annotations

import ast
import importlib.machinery
import importlib.util
import json
import re
import sys
import warnings
from collections.abc import Generator
from contextlib import contextmanager
from functools import cache, partial
from html import unescape
from pathlib import Path
from types import GenericAlias, ModuleType, SimpleNamespace
from typing import cast, get_origin
from unittest import mock

import pdoc.doc
import pdoc.doc_ast
import pdoc.doc_pyi
import pdoc.render

import gecode_iris
import gecode_iris._cli
import gecode_iris._data
import gecode_iris._error
import gecode_iris.build
import gecode_iris.build.gecode

if sys.version_info >= (3, 11):
    from typing import get_overloads
else:  # pdoc renders the reference on the Python that the site build installs
    raise SystemExit("the reference needs Python 3.11 or newer for typing.get_overloads")

TEMPLATES = Path(__file__).parent / "templates"
HEADING = re.compile(r'<h([23]) id="([^"]+)">(.*?)</h\1>', re.DOTALL)
SIGNATURE = re.compile(r'<div class="api-signature">.*?</div>', re.DOTALL)
TYPE_VARIABLE = re.compile(
    r'(?:<span class="o">[~+]</span>(?=<span class="n">_?[A-Z])|(?<![\w\])])[~+](?=_?[A-Z]\w*\b))'
)
STUB = "gecode_iris._native"
BUILDER = "gecode_iris.build"
# The site's URLs of the two pages, which replace the links between them that
# pdoc writes relative to its own files.
PAGES = {"gecode_iris": "/using/python/api/", BUILDER: "/using/python/api/build/"}
RELATIVE_LINKS = {
    "gecode_iris": {"gecode_iris/build.html": PAGES[BUILDER]},
    BUILDER: {"../gecode_iris.html": PAGES["gecode_iris"]},
}


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
    """Let pdoc read ``Entry``, ``Error`` and ``CallSite`` from their modules.

    The package sets their ``__module__`` to ``gecode_iris``, so
    ``inspect.getsource`` looks for them in the package's ``__init__.py``, and
    pdoc would miss the docstrings below their attributes.
    """
    sources: dict[object, str] = {
        gecode_iris.Entry: class_source(gecode_iris._data, "Entry"),
        gecode_iris.Error: class_source(gecode_iris._error, "Error"),
        gecode_iris.CallSite: class_source(gecode_iris._error, "CallSite"),
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


def import_in_dependency_order(module_name: str, path: Path) -> ModuleType:
    """Import a stub whose statements name classes that later ones define.

    A stub may refer forward, as in ``TypeVar("_F", bound=Field[Any])`` before
    ``class Field``, which type checkers accept and Python does not. Run each
    top-level statement once the names it uses exist, and fail with the error
    of the first statement that never can.
    """
    loader = importlib.machinery.SourceFileLoader(module_name, str(path))
    spec = importlib.util.spec_from_file_location(module_name, path, loader=loader)
    assert spec is not None
    module = importlib.util.module_from_spec(spec)
    tree = ast.parse(path.read_text(encoding="utf-8"), str(path))
    # A string alone would become the docstring of the module, and pdoc reads
    # the docstrings of attributes from the source.
    pending = [
        statement
        for statement in tree.body
        if not (isinstance(statement, ast.Expr) and isinstance(statement.value, ast.Constant))
    ]

    def run(statement: ast.stmt) -> None:
        # The stub is the package's own source, which pdoc would also execute.
        exec(compile(ast.Module([statement], []), str(path), "exec"), module.__dict__)  # noqa: S102

    while pending:
        deferred = []
        for statement in pending:
            try:
                run(statement)
            except NameError:
                deferred.append(statement)
        if len(deferred) == len(pending):
            run(deferred[0])
        pending = deferred
    module.__doc__ = ast.get_docstring(tree)
    return module


@contextmanager
def builder_stub() -> Generator[dict[str, ModuleType]]:
    """Let pdoc import the builder's stub, and keep it for its overloads."""
    imported: dict[str, ModuleType] = {}
    original = pdoc.doc_pyi._import_stub_file

    def import_stub_file(module_name: str, path: Path) -> ModuleType:
        if module_name != BUILDER:
            return original(module_name, path)
        imported[module_name] = import_in_dependency_order(module_name, path)
        return imported[module_name]

    # As for Spec and Data, signatures name Value instead of expanding it.
    value = named("gecode_iris", "Value")
    with (
        mock.patch.object(pdoc.doc_pyi, "_import_stub_file", import_stub_file),
        mock.patch.object(gecode_iris, "Value", value),
    ):
        yield imported


def overloads(
    stubs: dict[str, ModuleType], member: pdoc.doc.Doc[object]
) -> list[pdoc.doc.Function]:
    """The overloads of a function or method in its module's stub, so that the
    reference shows each typed signature instead of the implementation's.

    A method's overloads may sit in the stub on a base class, which can be
    private, such as ``_Fields.field`` for ``IntLookup.field``.
    """
    stub = stubs.get(member.modulename)
    if stub is None or not isinstance(member, pdoc.doc.Function):
        return []
    *owners, name = member.qualname.split(".")
    if owners:
        cls = stub.__dict__.get(owners[0])
        candidates = [f"{base.__qualname__}.{name}" for base in getattr(cls, "__mro__", ())]
    else:
        candidates = [name]
    for qualname in candidates:
        stand_in = SimpleNamespace(__module__=stub.__name__, __qualname__=qualname)
        if functions := get_overloads(stand_in):
            documented = [
                pdoc.doc.Function(member.modulename, member.qualname, function, member.taken_from)
                for function in functions
            ]
            # pdoc resolves the annotations in the module that the functions
            # name, which must be the stub to know its private aliases.
            with mock.patch.dict(sys.modules, {stub.__name__: stub}):
                for function in documented:
                    assert function.signature
            return documented
    return []


def members(stubs: dict[str, ModuleType], cls: pdoc.doc.Class) -> list[pdoc.doc.Doc[object]]:
    """The members that the section of a class documents.

    Without a stub these are its own members. With one, the stub decides, as
    it decides the types: the class's own members in the stub, and those it
    inherits from a private base that no public base provides, such as the
    ``record`` that ``Scope1`` takes from ``_Nested``. A public base documents
    its members itself.
    """
    stub = stubs.get(cls.modulename)
    declared = {} if stub is None else stub_declarations(stub)
    if cls.qualname not in declared:
        return list(cls.own_members)
    assert stub is not None
    stub_class = stub.__dict__[cls.qualname]
    bases = [base for base in stub_class.__mro__ if base.__name__ in declared]
    public = {
        name
        for base in bases[1:]
        if not base.__name__.startswith("_")
        for name in declared[base.__name__]
    }
    names = list(declared[cls.qualname])
    for base in bases[1:]:
        if base.__name__.startswith("_"):
            names += [name for name in declared[base.__name__] if name not in public]
    return [cls.members[name] for name in dict.fromkeys(names) if name in cls.members]


def bases(stubs: dict[str, ModuleType], cls: pdoc.doc.Class) -> list[tuple[str, str, str]]:
    """The bases that the signature of a class shows, as ``Class.bases`` gives
    them: in the stub, if it has the class, with each private base replaced by
    its own bases, such as ``Scope[_N_co]`` for ``_Nested[_N_co]``."""
    stub = stubs.get(cls.modulename)
    stub_class = None if stub is None else stub.__dict__.get(cls.qualname)
    if stub is None or not isinstance(stub_class, type) or stub_class.__module__ != stub.__name__:
        return cls.bases

    def public(base: object) -> list[tuple[str, str, str]]:
        origin = get_origin(base) or base
        assert isinstance(origin, type)
        if origin.__module__ == stub.__name__ and origin.__name__.startswith("_"):
            return [shown for inner in direct_bases(origin) for shown in public(inner)]
        return [
            (
                origin.__module__,
                origin.__qualname__,
                str(base) if get_origin(base) else f"{origin.__module__}.{origin.__qualname__}",
            )
        ]

    shown = [entry for base in direct_bases(stub_class) for entry in public(base)]
    return list(dict.fromkeys(shown))


def direct_bases(cls: type) -> list[object]:
    """The bases of a class as its definition writes them, with their type
    arguments."""
    return [base for base in getattr(cls, "__orig_bases__", cls.__bases__) if base is not object]


@cache
def stub_declarations(stub: ModuleType) -> dict[str, list[str]]:
    """The names that each top-level class of a stub declares, in their order
    in the source: methods, properties and annotated attributes."""
    assert stub.__file__ is not None
    tree = ast.parse(Path(stub.__file__).read_text(encoding="utf-8"))
    declared: dict[str, list[str]] = {}
    for node in tree.body:
        if isinstance(node, ast.ClassDef):
            names = declared.setdefault(node.name, [])
            for statement in node.body:
                if isinstance(statement, ast.FunctionDef):
                    names.append(statement.name)
                elif isinstance(statement, ast.AnnAssign) and isinstance(
                    statement.target, ast.Name
                ):
                    names.append(statement.target.id)
    return declared


def inherited_anchors(module: pdoc.doc.Module, html: str) -> str:
    """Point a link to an inherited member, such as ``#Scope0.forward``, at the
    base whose section documents it, here ``#Scope.forward``."""
    ids = set(re.findall(r'\bid="([^"]+)"', html))

    def documented(link: re.Match[str]) -> str:
        owner, name = link["owner"], link["name"]
        if f"{owner}.{name}" in ids:
            return link[0]
        cls = getattr(module.obj, owner, None)
        for base in getattr(cls, "__mro__", ())[1:]:
            if f"{base.__name__}.{name}" in ids:
                return f'href="#{base.__name__}.{name}"'
        raise SystemExit(f"{module.modulename}: no section documents {owner}.{name}")

    return re.sub(r'href="#(?P<owner>[A-Za-z]\w*)\.(?P<name>\w+)"', documented, html)


def page(module: pdoc.doc.Module, all_modules: dict[str, pdoc.doc.Module]) -> dict[str, object]:
    html = pdoc.render.html_module(module, all_modules).strip()
    # Signatures name a type variable as Python shows it, such as ~_U or
    # +_N_co; the reference names it as the stub writes it.
    html = SIGNATURE.sub(lambda block: TYPE_VARIABLE.sub("", block[0]), html)
    for relative, absolute in RELATIVE_LINKS[module.modulename].items():
        html = re.sub(f'href="{re.escape(relative)}(?=[#"])', f'href="{absolute}', html)
    html = inherited_anchors(module, html)
    pages = "|".join(re.escape(url) for url in PAGES.values())
    if stray := re.findall(f'href="(?!#|(?:{pages})[#"])([^"]*)"', html):
        raise SystemExit(f"{module.modulename}: links outside the reference: {stray}")
    headings = [
        {"depth": int(depth), "slug": slug, "text": unescape(re.sub(r"<[^>]+>", "", text))}
        for depth, slug, text in HEADING.findall(html)
    ]
    return {"headings": headings, "html": html}


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
    with builder_stub() as stubs:
        builder = pdoc.doc.Module(gecode_iris.build)
        # pdoc applies the stub when it first lists the members.
        assert builder.members
    template_globals["members"] = partial(members, stubs)
    template_globals["class_bases"] = partial(bases, stubs)
    template_globals["overloads"] = partial(overloads, stubs)
    all_modules = {package.modulename: package, builder.modulename: builder}
    with defining_sources():
        apply_stub(package)
        template_globals["appendix"] = "command"
        template_globals["command"] = pdoc.doc.Module(gecode_iris._cli)
        reference = page(package, all_modules)
        template_globals["appendix"] = "gecode"
        template_globals["gecode"] = pdoc.doc.Module(gecode_iris.build.gecode)
        reference["build"] = page(builder, all_modules)
    return {"pdoc": pdoc.__version__, **reference}


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: reference.py OUTPUT.json")
    # pdoc evaluates Entry's annotations in the package, which does not import
    # Literal, and then shows them as written, which is what the reference wants.
    warnings.filterwarnings("ignore", r"Error parsing type annotation .*Literal\[", UserWarning)
    # The builder's expressions are unhashable in the stub, where pdoc expects
    # the method of the implementation; neither is part of the reference.
    warnings.filterwarnings("ignore", r"Error processing type stub for .*\.__hash__:", UserWarning)
    Path(sys.argv[1]).write_text(json.dumps(render(), indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
