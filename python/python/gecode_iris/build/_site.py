"""Call sites: where in the host program each statement was called."""

from __future__ import annotations

import sys
from bisect import bisect_right
from collections.abc import Callable
from types import CodeType, FrameType
from typing import TypeVar

from gecode_iris._error import CallSite

_F = TypeVar("_F", bound=Callable[..., object])

_PACKAGE = __name__.rpartition(".")[0]
_TRACKED: set[CodeType] = set()
_UNKNOWN = CallSite("<unknown>", 0, "<unknown>")


def track_caller(function: _F) -> _F:
    """Make a helper record its caller as the call site of its statements.

    Statements record the line of user code that called them. A helper that
    writes statements for its caller can pass on that caller's line, as Rust's
    ``#[track_caller]`` does, by using this decorator.
    """
    code = getattr(function, "__code__", None)
    if not isinstance(code, CodeType):
        raise TypeError("track_caller takes a Python function")
    _TRACKED.add(code)
    return function


def _is_internal(module: object) -> bool:
    return isinstance(module, str) and (module == _PACKAGE or module.startswith(_PACKAGE + "."))


def caller() -> CallSite:
    """The innermost frame outside this package and outside tracked helpers."""
    frame: FrameType | None = sys._getframe(1)
    while frame is not None and (
        _is_internal(frame.f_globals.get("__name__")) or frame.f_code in _TRACKED
    ):
        frame = frame.f_back
    if frame is None:
        return _UNKNOWN
    return CallSite(frame.f_code.co_filename, frame.f_lineno, frame.f_code.co_name)


class CallSites:
    """The call site of each statement in a built spec's text.

    The native ``Spec`` calls ``at`` for the byte offset of a failure's
    statement, so that its ``Error`` names a call site.
    """

    __slots__ = ("_sites", "_starts")

    def __init__(self, starts: list[int], sites: list[CallSite]) -> None:
        self._starts = starts
        self._sites = sites

    def at(self, offset: int) -> CallSite | None:
        """The call site of the statement that holds byte ``offset``."""
        after = bisect_right(self._starts, offset)
        return self._sites[after - 1] if after else None
