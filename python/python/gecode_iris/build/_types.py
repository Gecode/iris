"""Reading types and the options of a declaration."""

from __future__ import annotations

from dataclasses import dataclass, replace
from typing import Any, ClassVar, Generic, NamedTuple, TypeAlias, TypeVar

from gecode_iris.build._expr import operand
from gecode_iris.build._ops import Ops, ValueTag, postfix

_T = TypeVar("_T")


class BaseSpec(NamedTuple):
    """``int``, ``float``, ``num``, ``word`` or ``rest``."""

    name: str


class AlphabetBlank(NamedTuple):
    characters: str


class AlphabetStrip(NamedTuple):
    characters: str


class AlphabetBase(NamedTuple):
    value: int


AlphabetOption: TypeAlias = AlphabetBlank | AlphabetStrip | AlphabetBase


class AlphabetSpec(NamedTuple):
    symbols: str
    options: tuple[AlphabetOption, ...]


class NamedSpec(NamedTuple):
    """A use of the named type that the statement ``declaration`` declares."""

    name: str
    owner: int
    declaration: int


class RangeSpec(NamedTuple):
    lower: Ops | None
    upper: Ops | None
    inclusive: bool


TypeSpec: TypeAlias = BaseSpec | AlphabetSpec | NamedSpec | RangeSpec


@dataclass(frozen=True)
class Declared:
    """A type with the dimensions and options of a declaration."""

    ty: TypeSpec
    dimensions: tuple[Ops, ...] = ()
    layout: Ops | None = None
    offset: Ops | None = None

    def is_open_ended(self) -> bool:
        """Whether the text of this type ends in ``..``, with no upper bound."""
        return (
            self.layout is None
            and self.offset is None
            and isinstance(self.ty, RangeSpec)
            and self.ty.upper is None
        )


def ops(value: object) -> Ops:
    return postfix(operand(value)._node)


class Type(Generic[_T]):
    """A reading type. ``dim`` adds a dimension, and ``layout`` and ``offset``
    add the declaration's options."""

    __slots__ = ("_declared",)
    _tag: ClassVar[ValueTag]

    def __init__(self, declared: Declared) -> None:
        self._declared = declared

    def _with(self, declared: Declared) -> Any:
        return _PLAIN[self._tag](declared)

    def dim(self, size: object) -> Any:
        """Add a dimension: ``x[size]: type``."""
        declared = self._declared
        return self._with(replace(declared, dimensions=(*declared.dimensions, ops(size))))

    def dims(self, *sizes: object) -> Any:
        """Add several dimensions, as many as known only at run time."""
        declared = self._declared
        added = tuple(ops(size) for size in sizes)
        return self._with(replace(declared, dimensions=(*declared.dimensions, *added)))

    def layout(self, layout: object) -> Any:
        """``layout expression``."""
        return self._with(replace(self._declared, layout=ops(layout)))

    def offset(self, offset: object) -> Any:
        """``offset expression``."""
        return self._with(replace(self._declared, offset=ops(offset)))

    def until(self, end: object) -> ListReading[Any]:
        """A list of this type ending at ``end``: a sentinel, or ``ib.eol``."""
        if self._declared.dimensions:
            raise TypeError("a list reads scalars: its type has no dimensions")
        terminator = None if isinstance(end, Eol) else ops(end)
        return ListReading(self._tag, self._declared, terminator)

    def __repr__(self) -> str:
        return f"<{type(self).__name__}>"


class IntType(Type[_T]):
    """A type that reads integers."""

    __slots__ = ()
    _tag = "int"


class FloatType(Type[_T]):
    """A type that reads floats."""

    __slots__ = ()
    _tag = "float"


class NumType(Type[_T]):
    """A type that reads ints or floats, decided by the data."""

    __slots__ = ()
    _tag = "num"


class StrType(Type[_T]):
    """A type that reads strings."""

    __slots__ = ()
    _tag = "str"


_PLAIN: dict[ValueTag, type[Type[Any]]] = {
    "int": IntType,
    "float": FloatType,
    "num": NumType,
    "str": StrType,
}


class Alphabet(IntType[int]):
    """``alphabet "symbols"``: reads each symbol as its index. ``blank``,
    ``strip`` and ``base`` add options in call order."""

    __slots__ = ()

    def _option(self, option: AlphabetOption) -> Alphabet:
        spec = self._declared.ty
        assert isinstance(spec, AlphabetSpec)
        return Alphabet(replace(self._declared, ty=spec._replace(options=(*spec.options, option))))

    def blank(self, characters: str) -> Alphabet:
        """``blank "characters"``."""
        return self._option(AlphabetBlank(characters))

    def strip(self, characters: str) -> Alphabet:
        """``strip "characters"``."""
        return self._option(AlphabetStrip(characters))

    def base(self, value: int) -> Alphabet:
        """``base value``."""
        if isinstance(value, bool) or not isinstance(value, int):
            raise TypeError("an alphabet base is an int")
        return self._option(AlphabetBase(value))


def alphabet(
    symbols: str,
    *,
    blank: str | None = None,
    strip: str | None = None,
    base: int | None = None,
) -> Alphabet:
    """``alphabet "symbols"``, with the options given, in this order."""
    result = Alphabet(Declared(AlphabetSpec(symbols, ())))
    if blank is not None:
        result = result.blank(blank)
    if strip is not None:
        result = result.strip(strip)
    if base is not None:
        result = result.base(base)
    return result


class ListReading(Generic[_T]):
    """The type and end of a ``list`` statement."""

    __slots__ = ("_declared", "_end", "_tag")

    def __init__(self, tag: ValueTag, declared: Declared, end: Ops | None) -> None:
        self._tag = tag
        self._declared = declared
        self._end = end
        """The sentinel, or ``None`` for ``until eol``."""

    def offset(self, offset: object) -> ListReading[_T]:
        """``offset expression``, after the list's end."""
        return ListReading(self._tag, replace(self._declared, offset=ops(offset)), self._end)


class Eol:
    """The end-of-line marker of a list."""

    __slots__ = ()

    def __repr__(self) -> str:
        return "ib.eol"


eol = Eol()
int_: IntType[int] = IntType(Declared(BaseSpec("int")))
float_: FloatType[float] = FloatType(Declared(BaseSpec("float")))
num: NumType[int | float] = NumType(Declared(BaseSpec("num")))
word: StrType[str] = StrType(Declared(BaseSpec("word")))
rest: StrType[str] = StrType(Declared(BaseSpec("rest")))


def _bound(value: object) -> Ops:
    expr = operand(value)
    if expr._tag not in ("int", "num", "dyn"):
        raise TypeError(f"a range bound is an int or num, not {type(expr).__name__}")
    return postfix(expr._node)


def int_range(lo: object = None, hi: object = None, *, below: object = None) -> IntType[int]:
    """``lo..hi``, both included, or ``lo..<below``. Either end may be left
    out, but not both."""
    if hi is not None and below is not None:
        raise TypeError("give `hi` or `below`, not both")
    if lo is None and hi is None and below is None:
        raise TypeError("a range needs at least one bound; use `ib.int_` for any integer")
    lower = None if lo is None else _bound(lo)
    upper_value = below if below is not None else hi
    upper = None if upper_value is None else _bound(upper_value)
    return IntType(Declared(RangeSpec(lower, upper, below is None)))


def at_least(lo: object) -> IntType[int]:
    """``lo..``, the integers from ``lo`` up."""
    return int_range(lo)


def declared_of(ty: object, role: str) -> Declared:
    if not isinstance(ty, Type):
        raise TypeError(f"{role} is a type such as ib.int_, not {type(ty).__name__}")
    return ty._declared


def scalar_declared(ty: object, role: str) -> Declared:
    declared = declared_of(ty, role)
    if declared.dimensions:
        raise TypeError(f"{role} has no dimensions")
    return declared


def named_type(ty: Type[Any], declared: Declared) -> Any:
    """A type of the same value type as ``ty`` with another declaration."""
    return _PLAIN[ty._tag](declared)
