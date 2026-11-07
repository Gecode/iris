"""The exception for Iris language errors."""

from __future__ import annotations

from typing import NamedTuple


class CallSite(NamedTuple):
    """A place in the host program: where a builder statement was called.

    ``str(site)`` is ``file:line``.
    """

    filename: str
    """The file of the calling code, as its code object names it."""
    line: int
    """The line of the call, counting from 1."""
    function: str
    """The name of the calling function, such as ``<module>``."""

    def __str__(self) -> str:
        return f"{self.filename}:{self.line}"


# The package exports this class, so pickles name it there.
CallSite.__module__ = "gecode_iris"


class Error(Exception):
    """A language error from parsing a reader spec or reading an input.

    ``str(error)`` is the full diagnostic. The location is the input
    location when there is one, and otherwise the reader spec location;
    ``line``, ``column`` and ``origin`` are each ``None`` when unknown.
    ``path`` is the record path of a failed check over records.

    Host failures, such as a missing file, raise built-in exceptions
    instead and have no error code.
    """

    # The package exports this class, so pickles and tracebacks name it there.
    __module__ = "gecode_iris"

    code: str
    """The stable error code, such as ``E-CHECK-FAILED``."""
    message: str
    """The message, without location or excerpt."""
    line: int | None
    """The line of the location, counting from 1."""
    column: int | None
    """The byte column of the location, counting from 1."""
    path: tuple[int, ...] | None
    """The record path of a failed check over records."""
    origin: str | None
    """The input or reader spec origin of the location, such as its path."""
    call_site: CallSite | None
    """Where the statement the error belongs to was called, for a spec built
    with ``gecode_iris.build``; ``None`` for a spec parsed from text."""

    def __init__(
        self,
        text: str,
        code: str,
        message: str,
        line: int | None,
        column: int | None,
        path: tuple[int, ...] | None,
        origin: str | None,
        document: str,
        call_site: CallSite | None = None,
    ) -> None:
        super().__init__(text)
        self.code = code
        self.message = message
        self.line = line
        self.column = column
        self.path = path
        self.origin = origin
        self.call_site = call_site
        self._document = document

    def to_json(self) -> str:
        """Return the canonical JSON error document."""
        return self._document

    def __reduce__(self) -> tuple[type[Error], tuple[object, ...]]:
        text = self.args[0]
        fields = (self.code, self.message, self.line, self.column, self.path, self.origin)
        return type(self), (text, *fields, self._document, self.call_site)
