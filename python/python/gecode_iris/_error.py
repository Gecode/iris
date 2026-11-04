"""The exception for Iris language errors."""

from __future__ import annotations


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
    message: str
    line: int | None
    column: int | None
    path: tuple[int, ...] | None
    origin: str | None

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
    ) -> None:
        super().__init__(text)
        self.code = code
        self.message = message
        self.line = line
        self.column = column
        self.path = path
        self.origin = origin
        self._document = document

    def to_json(self) -> str:
        """Return the canonical JSON error document."""
        return self._document

    def __reduce__(self) -> tuple[type[Error], tuple[object, ...]]:
        text = self.args[0]
        fields = (self.code, self.message, self.line, self.column, self.path, self.origin)
        return type(self), (text, *fields, self._document)
