"""Conformance driver that reads with a format's hand-written builder.

The first argument names a format directory under ``formats/``, such as
``dimacs-cnf``; the rest is the protocol of tests/conformance_driver.py. The
driver builds the spec with the function of the same name, ``dimacs_cnf``, in
``python/examples/build_dimacs_cnf.py`` and answers with it in place of the
case's own spec, so the runner judges each case of the format against what the
builder builds:

    python -I tests/format_builder_driver.py jobshop read SPEC INPUT
"""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path
from types import ModuleType

import gecode_iris as iris

TESTS = Path(__file__).resolve().parent
EXAMPLES = TESTS.parent / "examples"


def load_module(name: str, path: Path) -> ModuleType:
    """Import the file at ``path``, which ``-I`` keeps off ``sys.path``,
    without writing ``__pycache__`` beside it: the site's build requires every
    file in ``python/examples`` to appear in the guide."""
    sys.dont_write_bytecode = True
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None, path
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def built_spec(format_name: str) -> iris.Spec:
    function = format_name.replace("-", "_")
    builder = load_module(f"build_{function}", EXAMPLES / f"build_{function}.py")
    spec: iris.Spec = getattr(builder, function)().spec
    return spec


def main(args: list[str]) -> int:
    driver = load_module("conformance_driver", TESTS / "conformance_driver.py")
    if not args:
        return driver.report(driver.EXIT_MISUSE, "expected a format name")
    format_name, rest = args[0], args[1:]
    spec = built_spec(format_name)
    status: int = driver.main(rest, load=lambda _path: spec)
    return status


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
