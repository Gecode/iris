"""What ``build()`` and ``source()`` raise."""

from __future__ import annotations

import unicodedata
from typing import Literal, TypeAlias

from gecode_iris._error import CallSite, Error

BuildErrorKind: TypeAlias = Literal[
    "scope", "provenance", "redefinition", "unprintable", "binding", "spec"
]

_ESCAPES = {"\t": "\\t", "\r": "\\r", "\n": "\\n"}


def escape_controls(text: str) -> str:
    """Escape control characters as the Rust reader does, so that a terminal
    never acts on them."""
    if text.isprintable():
        return text
    return "".join(
        _ESCAPES.get(c, f"\\u{{{ord(c):x}}}") if unicodedata.category(c) == "Cc" else c
        for c in text
    )


class BuildError(Exception):
    """Why a reader spec could not be built.

    ``kind`` names the builder check that failed, or is ``"spec"`` when the
    emitted text failed to parse; then ``error`` is the language error, which
    is also the ``__cause__``. ``str(error)`` shows the language code and
    message, the line of emitted text, and the call site.
    """

    kind: BuildErrorKind
    """The check that failed: ``scope``, ``provenance``, ``redefinition``,
    ``unprintable``, ``binding``, or ``spec`` for a language error."""
    message: str
    """The message of a builder check, without the call site. For a ``spec``
    error it is the language diagnostic, with its location and excerpt, less
    the call site line."""
    call_site: CallSite | None
    """The host code of the statement that failed."""
    text: str | None
    """The emitted text, when the language rejected it."""
    error: Error | None
    """The language error, when the emitted text was rejected."""

    def __init__(
        self,
        kind: BuildErrorKind,
        message: str,
        call_site: CallSite | None,
        text: str | None = None,
        error: Error | None = None,
    ) -> None:
        super().__init__(kind, message, call_site, text, error)
        self.kind = kind
        self.message = message
        self.call_site = call_site
        self.text = text
        self.error = error

    @classmethod
    def from_error(cls, error: Error, text: str) -> BuildError:
        """The error for emitted text that the language rejected."""
        message = str(error)
        if error.call_site is not None:
            line = "\ncall site: " + escape_controls(str(error.call_site))
            message = message.replace(line, "", 1)
        return cls("spec", message, error.call_site, text, error)

    @property
    def emitted_line(self) -> str | None:
        """The line of emitted text that the language error points at."""
        if self.text is None or self.error is None or self.error.line is None:
            return None
        lines = self.text.split("\n")
        if self.text.endswith("\n"):
            lines.pop()
        index = self.error.line - 1
        return lines[index].removesuffix("\r") if 0 <= index < len(lines) else None

    def __str__(self) -> str:
        parts = [f"{self.error.code}: " if self.error is not None else "", self.message]
        line = self.emitted_line
        if self.error is not None and self.error.line is not None and line is not None:
            parts.append(f"\n{self.error.line:>6} | {escape_controls(line)}")
        if self.call_site is not None:
            parts.append(f"\ncall site: {escape_controls(str(self.call_site))}")
        return "".join(parts)
