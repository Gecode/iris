"""Conformance driver for the Python binding.

Speaks the protocol in conformance/README.md, "Four driver commands", through
the public ``gecode_iris`` API only, mirroring rust/iris-cli's iris-driver.
Run it with the interpreter of an environment where the package is installed;
``-I`` keeps the source tree off ``sys.path``:

    python -I tests/conformance_driver.py read SPEC INPUT --params '{"n": "3"}'
"""

from __future__ import annotations

import json
import sys
from collections.abc import Callable
from dataclasses import dataclass

import gecode_iris as iris

PARSE_SUCCESS = '{"iris":"0.1","parsed":true}\n'
SUPPORTED_VERSION = "0.1"
INPUT_OPERATIONS = ("read", "catalog", "dzn")

EXIT_SUCCESS = 0
EXIT_LANGUAGE_ERROR = 1
EXIT_MISUSE = 2
EXIT_HOST_FAILURE = 3


class Misuse(Exception):
    """The command line breaks the protocol."""


@dataclass(frozen=True)
class Request:
    operation: str
    spec: str
    # The input path; None exactly for parse.
    input: str | None
    params: dict[str, str]
    dzn: bool


def main(args: list[str], load: Callable[[str], iris.Spec] = iris.Spec.load) -> int:
    """Answer the request in ``args``, with ``load`` turning its spec path into a
    spec; tests/format_builder_driver.py builds the spec instead."""
    try:
        request = parse_request(args)
    except Misuse as error:
        return report(EXIT_MISUSE, str(error))
    try:
        status, document = outcome(request, load)
        write(document)
    except NotImplementedError as error:
        return report(EXIT_MISUSE, str(error))
    except Exception as error:  # noqa: BLE001 - the protocol maps every other failure to 3.
        return report(EXIT_HOST_FAILURE, f"{type(error).__name__}: {error}")
    return status


def outcome(request: Request, load: Callable[[str], iris.Spec]) -> tuple[int, str]:
    """Return the exit status and its document; a language error is a result."""
    try:
        return EXIT_SUCCESS, success_document(request, load)
    except iris.Error as error:
        return EXIT_LANGUAGE_ERROR, error.to_json()


def success_document(request: Request, load: Callable[[str], iris.Spec]) -> str:
    """Return the success document: parse JSON, data JSON or dzn text."""
    spec = load(request.spec)
    if request.input is None:
        return PARSE_SUCCESS
    if request.operation == "catalog":
        data = spec.catalog(request.input, params=request.params)
    else:
        data = spec.read(request.input, params=request.params)
    return data.to_dzn() if request.dzn else data.to_json()


def parse_request(args: list[str]) -> Request:
    if not args or args[0] not in ("parse", *INPUT_OPERATIONS):
        raise Misuse("expected parse, read, catalog, or dzn")
    operation, rest = args[0], args[1:]
    arity = 2 if operation in INPUT_OPERATIONS else 1
    paths = rest[:arity]
    if len(paths) < arity or any(path.startswith("--") for path in paths):
        expected = "a reader-spec path and an input path" if arity == 2 else "a reader-spec path"
        raise Misuse(f"{operation} requires {expected}")
    allowed = {"--params", "--version"} | ({"--format"} if operation == "catalog" else set())
    options = parse_options(rest[arity:], allowed)

    version = options.get("--version", SUPPORTED_VERSION)
    if version != SUPPORTED_VERSION:
        raise Misuse(f"unsupported language version {version}; supported: {SUPPORTED_VERSION}")
    params = parse_params(options.get("--params", "{}"))
    if operation == "parse" and params:
        raise Misuse("parse does not accept caller parameters")
    output = options.get("--format", "json")
    if output not in ("json", "dzn"):
        raise Misuse("expected catalog format json or dzn")
    return Request(
        operation=operation,
        spec=paths[0],
        input=paths[1] if arity == 2 else None,
        params=params,
        dzn=operation == "dzn" or output == "dzn",
    )


def parse_options(args: list[str], allowed: set[str]) -> dict[str, str]:
    """Read ``--name value`` pairs, each allowed name at most once."""
    options: dict[str, str] = {}
    pairs = iter(args)
    for name in pairs:
        if name not in allowed:
            raise Misuse(f"unexpected argument {name}")
        if name in options:
            raise Misuse(f"duplicate option {name}")
        value = next(pairs, None)
        if value is None:
            raise Misuse(f"missing value for {name}")
        options[name] = value
    return options


def parse_params(text: str) -> dict[str, str]:
    """Decode a JSON object of strings with unique names, as UTF-8 text."""
    try:
        params = json.loads(text, object_pairs_hook=unique_members)
    except ValueError as error:
        raise Misuse(f"invalid params: {error}") from None
    if not isinstance(params, dict):
        raise Misuse("invalid params: expected a string-to-string parameter object")
    for name, value in params.items():
        if not isinstance(value, str):
            raise Misuse(f"invalid params: parameter {name!r} is not a string")
        if not (is_utf8(name) and is_utf8(value)):
            raise Misuse("invalid params: parameters must be valid Unicode")
    return params


def unique_members(members: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for name, value in members:
        if name in result:
            raise Misuse("invalid params: duplicate parameter")
        result[name] = value
    return result


def is_utf8(text: str) -> bool:
    """JSON escapes can produce lone surrogates, which UTF-8 cannot encode."""
    try:
        text.encode("utf-8")
    except UnicodeEncodeError:
        return False
    return True


def write(document: str) -> None:
    """Write the whole document once, as UTF-8 bytes without newline translation."""
    sys.stdout.buffer.write(document.encode("utf-8"))
    sys.stdout.buffer.flush()


def report(status: int, message: str) -> int:
    print(f"conformance-driver: {message}", file=sys.stderr)
    return status


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
