"""Line patterns: a ``str``, a tuple of text and parts, or a t-string."""

from __future__ import annotations

import sys
from typing import Any, NamedTuple, TypeAlias

from gecode_iris.build._expr import Field
from gecode_iris.build._ops import ValueTag
from gecode_iris.build._types import Declared, Type, scalar_declared

CaptureSpec: TypeAlias = "Declared | None"
"""A capture's type, or ``None`` for ``{x}``, which reads a word."""


class TextPiece(NamedTuple):
    text: str


class CapturePiece(NamedTuple):
    """A handle from ``capture``, printed from its declaration."""

    name: str
    owner: int


class BindPiece(NamedTuple):
    """``{x:type}`` through an existing handle."""

    name: str
    owner: int
    value: ValueTag
    capture: Declared | None


class ReferencePiece(NamedTuple):
    """``{$x}``."""

    name: str
    owner: int


class ValidatePiece(NamedTuple):
    """``{:type}``."""

    capture: Declared


class BlankPiece(NamedTuple):
    """``{}``."""


class DiscardPiece(NamedTuple):
    """``{_}``."""


class DiscardAsPiece(NamedTuple):
    """``{_:type}``."""

    capture: Declared


class RestOfLinePiece(NamedTuple):
    """The trailing ``...``."""


Piece: TypeAlias = (
    TextPiece
    | CapturePiece
    | BindPiece
    | ReferencePiece
    | ValidatePiece
    | BlankPiece
    | DiscardPiece
    | DiscardAsPiece
    | RestOfLinePiece
)


class Part:
    """A part of a pattern other than text or a capture handle."""

    __slots__ = ("_piece",)

    def __init__(self, piece: Piece) -> None:
        self._piece = piece

    def __repr__(self) -> str:
        return f"<Part {type(self._piece).__name__}>"


class Discard:
    """The discard ``_``: a target that reads without storing, and ``{_}`` in
    a pattern."""

    __slots__ = ()

    def __repr__(self) -> str:
        return "ib.discard()"


_DISCARD = Discard()


def discard() -> Discard:
    """``_``: read a value without storing it, or ``{_}`` in a pattern."""
    return _DISCARD


def capture_spec(ty: object | None) -> Declared | None:
    """The type of a capture: any type without dimensions, or ``None``."""
    return None if ty is None else scalar_declared(ty, "a capture's type")


def capture_tag(ty: object | None) -> ValueTag:
    if ty is None:
        return "str"
    assert isinstance(ty, Type)
    return ty._tag


def bind(handle: Field[Any], ty: object | None = None) -> Part:
    """``{x:type}``, binding a forward or redefined handle; without a type,
    ``{x}``, which reads a word."""
    if not isinstance(handle, Field):
        raise TypeError(f"bind takes a handle, not {type(handle).__name__}")
    spec = capture_spec(ty)
    return Part(BindPiece(handle._name, handle._owner, capture_tag(ty), spec))


def ref(handle: Field[Any]) -> Part:
    """``{$x}``, matching the current value of ``x``."""
    if not isinstance(handle, Field):
        raise TypeError(f"ref takes a handle, not {type(handle).__name__}")
    return Part(ReferencePiece(handle._name, handle._owner))


def validate(ty: object) -> Part:
    """``{:type}``, which checks the text without storing it."""
    return Part(ValidatePiece(scalar_declared(ty, "a capture's type")))


def blank() -> Part:
    """``{}``, which matches a word without storing it."""
    return Part(BlankPiece())


def discard_as(ty: object) -> Part:
    """``{_:type}``, a discarded capture with a type."""
    return Part(DiscardAsPiece(scalar_declared(ty, "a capture's type")))


rest_of_line = Part(RestOfLinePiece())
"""The trailing ``...`` of a pattern."""


def _part(value: object) -> Piece:
    if isinstance(value, str):
        return TextPiece(value)
    if isinstance(value, Field):
        return CapturePiece(value._name, value._owner)
    if isinstance(value, Part):
        return value._piece
    if isinstance(value, Discard):
        return DiscardPiece()
    raise TypeError(
        f"a pattern part is text, a capture handle, a part such as ib.ref(h), or "
        f"ib.discard(), not {type(value).__name__}"
    )


def _pieces(pattern: object) -> list[Piece]:
    if isinstance(pattern, str):
        return [TextPiece(pattern)]
    if isinstance(pattern, tuple):
        return [_part(value) for value in pattern]
    if sys.version_info >= (3, 14):
        from string.templatelib import Interpolation, Template

        if isinstance(pattern, Template):
            pieces: list[Piece] = []
            for item in pattern:
                if isinstance(item, Interpolation):
                    if item.conversion is not None or item.format_spec:
                        raise TypeError(
                            f"the pattern interpolation {{{item.expression}}} has a "
                            "conversion or format spec, which patterns do not take"
                        )
                    pieces.append(_part(item.value))
                else:
                    pieces.append(TextPiece(item))
            return pieces
    raise TypeError(
        "a pattern is a str, a tuple of text and parts, or a t-string on Python 3.14 "
        f"or later, not {type(pattern).__name__}"
    )


def pattern_pieces(pattern: object) -> list[Piece]:
    """The pieces of a pattern, with adjacent text joined."""
    pieces: list[Piece] = []
    for piece in _pieces(pattern):
        if isinstance(piece, TextPiece) and pieces and isinstance(pieces[-1], TextPiece):
            pieces[-1] = TextPiece(pieces[-1].text + piece.text)
        elif not (isinstance(piece, TextPiece) and not piece.text):
            pieces.append(piece)
    return pieces
