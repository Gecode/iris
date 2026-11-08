"""Build reader specs from Python code, with typed handles.

A :class:`SpecBuilder` is the root scope of a spec. Each statement is a method
call, a block is a ``with`` statement, and every definition returns a handle
that later statements use in expressions and that ``data[handle]`` reads from
the data with the type it promises.

A handle's type parameter is the value ``data[handle]`` gives: ``int`` for an
integer, ``list[int]`` for an array or for an integer read in a repeat, and
``int | None`` for an entry that may be absent. Scopes carry their depth in
their class, ``Scope0`` to ``Scope3``, and their presence in their parameter:
``Required`` when their statements always execute and ``None`` when they may
not. Inside more than three repeats, ``ScopeN`` gives ``DynField`` handles.

Python operators build Iris expressions, except for division, remainder and
power, which are :func:`div`, :func:`rem` and :func:`pow`, and the logical
operators, which are ``&``, ``|`` and ``~``. Line patterns are t-strings on
Python 3.14 and later, and tuples of text and parts on every version.

A mistake that a builder can find raises :class:`BuildError` at the call that
made it, or from ``build()``, and names the caller's file and line. Errors
while reading a built spec name the call site of their statement in
:attr:`gecode_iris.Error.call_site`.
"""

import builtins
import sys
from collections.abc import Callable
from typing import Any, Generic, Literal, NoReturn, Protocol, TypeAlias, TypeVar, overload

from gecode_iris import CallSite as CallSite
from gecode_iris import Error, Spec, Value
from gecode_iris.build import gecode as gecode

_T_co = TypeVar("_T_co", covariant=True)
_N_co = TypeVar("_N_co", covariant=True)
_S_co = TypeVar("_S_co", covariant=True)
_R_co = TypeVar("_R_co", covariant=True)
_O_co = TypeVar("_O_co", covariant=True)
_I_co = TypeVar("_I_co", covariant=True)
_U = TypeVar("_U")
_I = TypeVar("_I")
_F = TypeVar("_F", bound=Field[Any])
_C = TypeVar("_C", bound=Callable[..., Any])

Required: TypeAlias = NoReturn
"""The presence of a scope whose statements always execute: ``T | Required``
is ``T``."""

Maybe: TypeAlias = None
"""The presence of a scope whose statements may not execute."""

BuildErrorKind: TypeAlias = Literal[
    "scope", "provenance", "redefinition", "unprintable", "binding", "spec"
]
"""The value of ``BuildError.kind``: the builder check that failed, or
``spec`` when the language rejected the emitted text."""

# Expressions

_Int: TypeAlias = int | IntExpr | DynExpr
_Numeric: TypeAlias = int | float | IntExpr | FloatExpr | NumExpr | DynExpr
_Operand: TypeAlias = int | float | str | Expr
_Condition: TypeAlias = int | Expr

class _Domain(Protocol[_I_co]):
    """What a comprehension ranges over; its body gets ``_I_co``."""

    def _iris_item(self, node: Any, /) -> _I_co: ...

class Expr:
    """An Iris expression. Operators build larger expressions; ``bool()`` is an
    error, so that a comparison is never taken for a Python condition."""

    def __bool__(self) -> NoReturn: ...
    def __hash__(self) -> int: ...
    def dynamic(self) -> DynExpr:
        """This expression with no static type, accepted anywhere."""

class IntExpr(Expr):
    """An integer expression."""

    def __add__(self, other: _Int) -> IntExpr: ...
    def __radd__(self, other: int) -> IntExpr: ...
    def __sub__(self, other: _Int) -> IntExpr: ...
    def __rsub__(self, other: int) -> IntExpr: ...
    def __mul__(self, other: _Int) -> IntExpr: ...
    def __rmul__(self, other: int) -> IntExpr: ...
    def __and__(self, other: _Int) -> IntExpr: ...
    def __rand__(self, other: int) -> IntExpr: ...
    def __or__(self, other: _Int) -> IntExpr: ...
    def __ror__(self, other: int) -> IntExpr: ...
    def __lt__(self, other: _Numeric) -> IntExpr: ...
    def __le__(self, other: _Numeric) -> IntExpr: ...
    def __gt__(self, other: _Numeric) -> IntExpr: ...
    def __ge__(self, other: _Numeric) -> IntExpr: ...
    def __eq__(self, other: _Numeric) -> IntExpr: ...  # type: ignore[override]  # ty: ignore[invalid-method-override]
    def __ne__(self, other: _Numeric) -> IntExpr: ...  # type: ignore[override]  # ty: ignore[invalid-method-override]
    def __neg__(self) -> IntExpr: ...
    def __invert__(self) -> IntExpr: ...
    def _iris_item(self, node: Any, /) -> IntLookup: ...

class FloatExpr(Expr):
    """A float expression."""

    def __lt__(self, other: _Numeric) -> IntExpr: ...
    def __le__(self, other: _Numeric) -> IntExpr: ...
    def __gt__(self, other: _Numeric) -> IntExpr: ...
    def __ge__(self, other: _Numeric) -> IntExpr: ...
    def __eq__(self, other: _Numeric) -> IntExpr: ...  # type: ignore[override]  # ty: ignore[invalid-method-override]
    def __ne__(self, other: _Numeric) -> IntExpr: ...  # type: ignore[override]  # ty: ignore[invalid-method-override]
    def __neg__(self) -> FloatExpr: ...
    def _iris_item(self, node: Any, /) -> FloatLookup: ...

class NumExpr(Expr):
    """A ``num`` expression: an int or a float, decided by the data."""

    def __lt__(self, other: _Numeric) -> IntExpr: ...
    def __le__(self, other: _Numeric) -> IntExpr: ...
    def __gt__(self, other: _Numeric) -> IntExpr: ...
    def __ge__(self, other: _Numeric) -> IntExpr: ...
    def __eq__(self, other: _Numeric) -> IntExpr: ...  # type: ignore[override]  # ty: ignore[invalid-method-override]
    def __ne__(self, other: _Numeric) -> IntExpr: ...  # type: ignore[override]  # ty: ignore[invalid-method-override]
    def __neg__(self) -> NumExpr: ...
    def _iris_item(self, node: Any, /) -> NumLookup: ...

class StrExpr(Expr):
    """A string expression."""

    def __eq__(self, other: str | StrExpr | DynExpr) -> IntExpr: ...  # type: ignore[override]  # ty: ignore[invalid-method-override]
    def __ne__(self, other: str | StrExpr | DynExpr) -> IntExpr: ...  # type: ignore[override]  # ty: ignore[invalid-method-override]
    def _iris_item(self, node: Any, /) -> StrLookup: ...

class _Fields:
    """``field``: a member of the record an expression selects."""

    @overload
    def field(self, handle: IntField[Any]) -> IntLookup: ...
    @overload
    def field(self, handle: FloatField[Any]) -> FloatLookup: ...
    @overload
    def field(self, handle: NumField[Any]) -> NumLookup: ...
    @overload
    def field(self, handle: StrField[Any]) -> StrLookup: ...
    @overload
    def field(self, handle: DynField) -> DynExpr: ...

class DynExpr(_Fields, Expr):
    """An expression with no static type, which every operator accepts."""

    def __add__(self, other: _Int) -> IntExpr: ...
    def __radd__(self, other: int) -> IntExpr: ...
    def __sub__(self, other: _Int) -> IntExpr: ...
    def __rsub__(self, other: int) -> IntExpr: ...
    def __mul__(self, other: _Int) -> IntExpr: ...
    def __rmul__(self, other: int) -> IntExpr: ...
    def __and__(self, other: _Int) -> IntExpr: ...
    def __rand__(self, other: int) -> IntExpr: ...
    def __or__(self, other: _Int) -> IntExpr: ...
    def __ror__(self, other: int) -> IntExpr: ...
    def __lt__(self, other: _Numeric) -> IntExpr: ...
    def __le__(self, other: _Numeric) -> IntExpr: ...
    def __gt__(self, other: _Numeric) -> IntExpr: ...
    def __ge__(self, other: _Numeric) -> IntExpr: ...
    def __eq__(self, other: _Operand) -> IntExpr: ...  # type: ignore[override]  # ty: ignore[invalid-method-override]
    def __ne__(self, other: _Operand) -> IntExpr: ...  # type: ignore[override]  # ty: ignore[invalid-method-override]
    def __neg__(self) -> DynExpr: ...
    def __invert__(self) -> IntExpr: ...
    def __getitem__(self, index: _Int) -> DynExpr: ...
    def _iris_item(self, node: Any, /) -> DynExpr: ...

class IntLookup(_Fields, IntExpr):
    """An integer lookup, such as an index or a field: it may be an array."""

    def __getitem__(self, index: _Int) -> IntLookup: ...

class FloatLookup(_Fields, FloatExpr):
    """A float lookup, which may be an array."""

    def __getitem__(self, index: _Int) -> FloatLookup: ...

class NumLookup(_Fields, NumExpr):
    """A ``num`` lookup, which may be an array."""

    def __getitem__(self, index: _Int) -> NumLookup: ...

class StrLookup(_Fields, StrExpr):
    """A string lookup, which may be an array."""

    def __getitem__(self, index: _Int) -> StrLookup: ...

class Field(Expr, Generic[_T_co]):
    """A typed handle: the name of an entry. ``data[handle]`` reads its value,
    of type ``_T_co``.

    In the builder a handle is an expression; after reading it is a key.
    """

    @property
    def name(self) -> str:
        """The entry name."""

    def _iris_key(self) -> tuple[str, str | None, int | None, bool]: ...

class IntField(IntLookup, Field[_T_co]):
    """A handle of an integer entry."""

class FloatField(FloatLookup, Field[_T_co]):
    """A handle of a float entry."""

class NumField(NumLookup, Field[_T_co]):
    """A handle of a ``num`` entry."""

class StrField(StrLookup, Field[_T_co]):
    """A handle of a string entry."""

class DynField(DynExpr, Field[Value | None]):
    """A handle whose value type or depth is open."""

def declared(name: str, kind: type[_F]) -> _F:
    """A handle for the entry ``name`` of a text spec, of the handle type
    ``kind``, such as ``ib.IntField[list[int]]`` or ``ib.DynField``.

    ``iris gen --lang python SPEC`` prints one per entry. The handle reads data
    as a builder's handle does, but no builder accepts it, since it comes from
    none."""

@overload
def lit(value: int) -> IntExpr: ...  # pyright: ignore[reportOverlappingOverload]
@overload
def lit(value: float) -> FloatExpr: ...
@overload
def lit(value: str) -> StrExpr: ...
def div(left: _Int, right: _Int) -> IntExpr:
    """``left / right``, Iris integer division."""

def rem(left: _Int, right: _Int) -> IntExpr:
    """``left % right``, the Iris remainder."""

def pow(base: _Int, exponent: _Int) -> IntExpr:
    """``base ^ exponent``."""

# Records and comprehensions

class Record:
    """The records of a repeat. ``record[field]`` is the field of the current
    record, ``record.list`` all the records, and a comprehension over a record
    gives its body one record at a time."""

    @overload
    def __getitem__(self, handle: IntField[Any]) -> IntLookup: ...
    @overload
    def __getitem__(self, handle: FloatField[Any]) -> FloatLookup: ...
    @overload
    def __getitem__(self, handle: NumField[Any]) -> NumLookup: ...
    @overload
    def __getitem__(self, handle: StrField[Any]) -> StrLookup: ...
    @overload
    def __getitem__(self, handle: DynField) -> DynExpr: ...
    @property
    def list(self) -> RecordList:
        """All the records of this repeat, printed as its ``as`` name."""

    def records(self, inner: Record) -> RecordList:
        """The records of the nested repeat ``inner`` in the current record."""

    def _iris_item(self, node: Any, /) -> RecordExpr: ...

class RecordList:
    """A list of records: all the records of a repeat, or a nested list.
    ``records[i]`` selects one."""

    def __getitem__(self, index: _Int) -> RecordExpr: ...
    def _iris_item(self, node: Any, /) -> RecordExpr: ...

class RecordExpr:
    """One record: selected by index, or a comprehension's variable.
    ``record[field]`` looks up one of its fields."""

    @overload
    def __getitem__(self, handle: IntField[Any]) -> IntLookup: ...
    @overload
    def __getitem__(self, handle: FloatField[Any]) -> FloatLookup: ...
    @overload
    def __getitem__(self, handle: NumField[Any]) -> NumLookup: ...
    @overload
    def __getitem__(self, handle: StrField[Any]) -> StrLookup: ...
    @overload
    def __getitem__(self, handle: DynField) -> DynExpr: ...
    def at(self, index: _Int) -> RecordExpr:
        """``self[index]``, for a record that is itself a list."""

    def records(self, inner: Record) -> RecordList:
        """The records of the nested repeat ``inner`` in this record."""

class _Over(Generic[_I_co]):
    def _iris_item(self, node: Any, /) -> _I_co: ...

def over(domain: _Domain[_I], name: str | None = None) -> _Over[_I]:
    """``domain``, with its comprehension variable printed as ``name``."""

def all(domain: _Domain[_I], body: Callable[[_I], _Condition]) -> IntExpr:
    """``all(x in domain: body)``."""

def any(domain: _Domain[_I], body: Callable[[_I], _Condition]) -> IntExpr:
    """``any(x in domain: body)``."""

@overload
def sum(domain: _Domain[_I], body: Callable[[_I], _Condition], /) -> IntExpr:
    """``sum(x in domain: body)``."""

@overload
def sum(*arguments: float | Expr) -> IntExpr:
    """``sum(a, b, ...)``."""

def count(domain: _Domain[_I], body: Callable[[_I], _Condition] | None = None) -> IntExpr:
    """``count(domain)``, or ``count(x in domain: body)`` given a function."""

def size(domain: _Domain[Any]) -> IntExpr:
    """``size(domain)``."""

def min(*arguments: float | Expr) -> IntExpr:
    """``min(a, b, ...)``."""

def max(*arguments: float | Expr) -> IntExpr:
    """``max(a, b, ...)``."""

def distinct(*arguments: float | Expr) -> IntExpr:
    """``distinct(a, b, ...)``."""

def defined(name: Expr | Record | RecordList | RecordExpr) -> IntExpr:
    """``defined(name)``: a handle, record, record list or comprehension
    variable. ``defined(record)`` tests the record list."""

# Types

class Type(Generic[_U]):
    """A reading type, whose parameter is the value it reads, with a list for
    each dimension."""

class IntType(Type[_U]):
    """A type that reads integers."""

    def dim(self, size: _Int) -> IntType[list[_U]]:
        """Add a dimension: ``x[size]: type``."""

    @overload
    def dims(self, size: _Int, /) -> IntType[list[_U]]: ...
    @overload
    def dims(self, first: _Int, second: _Int, /) -> IntType[list[list[_U]]]: ...
    @overload
    def dims(self, first: _Int, second: _Int, third: _Int, /) -> IntType[list[list[list[_U]]]]: ...
    @overload
    def dims(self, *sizes: _Int) -> IntType[Any]: ...
    def layout(self, layout: _Operand) -> IntType[_U]:
        """``layout expression``: a matrix layout name such as ``"LOWER_COL"``."""

    def offset(self, offset: _Int) -> IntType[_U]:
        """``offset expression``."""

    def until(self, end: _Operand | Eol) -> ListReading[IntType[_U]]:
        """A list of this type ending at ``end``: a sentinel, or ``ib.eol``."""

class FloatType(Type[_U]):
    """A type that reads floats."""

    def dim(self, size: _Int) -> FloatType[list[_U]]:
        """Add a dimension: ``x[size]: type``."""

    @overload
    def dims(self, size: _Int, /) -> FloatType[list[_U]]: ...
    @overload
    def dims(self, first: _Int, second: _Int, /) -> FloatType[list[list[_U]]]: ...
    @overload
    def dims(
        self, first: _Int, second: _Int, third: _Int, /
    ) -> FloatType[list[list[list[_U]]]]: ...
    @overload
    def dims(self, *sizes: _Int) -> FloatType[Any]: ...
    def layout(self, layout: _Operand) -> FloatType[_U]:
        """``layout expression``: a matrix layout name such as ``"LOWER_COL"``."""

    def offset(self, offset: _Int) -> FloatType[_U]:
        """``offset expression``."""

    def until(self, end: _Operand | Eol) -> ListReading[FloatType[_U]]:
        """A list of this type ending at ``end``: a sentinel, or ``ib.eol``."""

class NumType(Type[_U]):
    """A type that reads ints or floats, decided by the data."""

    def dim(self, size: _Int) -> NumType[list[_U]]:
        """Add a dimension: ``x[size]: type``."""

    @overload
    def dims(self, size: _Int, /) -> NumType[list[_U]]: ...
    @overload
    def dims(self, first: _Int, second: _Int, /) -> NumType[list[list[_U]]]: ...
    @overload
    def dims(self, first: _Int, second: _Int, third: _Int, /) -> NumType[list[list[list[_U]]]]: ...
    @overload
    def dims(self, *sizes: _Int) -> NumType[Any]: ...
    def layout(self, layout: _Operand) -> NumType[_U]:
        """``layout expression``: a matrix layout name such as ``"LOWER_COL"``."""

    def offset(self, offset: _Int) -> NumType[_U]:
        """``offset expression``."""

    def until(self, end: _Operand | Eol) -> ListReading[NumType[_U]]:
        """A list of this type ending at ``end``: a sentinel, or ``ib.eol``."""

class StrType(Type[_U]):
    """A type that reads strings."""

    def dim(self, size: _Int) -> StrType[list[_U]]:
        """Add a dimension: ``x[size]: type``."""

    @overload
    def dims(self, size: _Int, /) -> StrType[list[_U]]: ...
    @overload
    def dims(self, first: _Int, second: _Int, /) -> StrType[list[list[_U]]]: ...
    @overload
    def dims(self, first: _Int, second: _Int, third: _Int, /) -> StrType[list[list[list[_U]]]]: ...
    @overload
    def dims(self, *sizes: _Int) -> StrType[Any]: ...
    def layout(self, layout: _Operand) -> StrType[_U]:
        """``layout expression``: a matrix layout name such as ``"LOWER_COL"``."""

    def offset(self, offset: _Int) -> StrType[_U]:
        """``offset expression``."""

    def until(self, end: _Operand | Eol) -> ListReading[StrType[_U]]:
        """A list of this type ending at ``end``: a sentinel, or ``ib.eol``."""

class Alphabet(IntType[int]):
    """``alphabet "symbols"``: reads each symbol as its index. ``blank``,
    ``strip`` and ``base`` add options in call order."""

    def blank(self, characters: str) -> Alphabet:
        """``blank "characters"``."""

    def strip(self, characters: str) -> Alphabet:
        """``strip "characters"``."""

    def base(self, value: int) -> Alphabet:
        """``base value``."""

def alphabet(
    symbols: str,
    *,
    blank: str | None = None,
    strip: str | None = None,
    base: int | None = None,
) -> Alphabet:
    """``alphabet "symbols"``, with the options given, in this order."""

class ListReading(Generic[_U]):
    """The type and end of a ``list`` statement; the parameter is the type."""

    def offset(self, offset: _Int) -> ListReading[_U]:
        """``offset expression``, after the list's end."""

class Eol:
    """The end-of-line marker of a list."""

eol: Eol
"""``until eol``: a list that ends with its line."""
int_: IntType[int]
"""``int``."""
float_: FloatType[float]
"""``float``."""
num: NumType[int | float]
"""``num``: an int or a float, decided by the data."""
word: StrType[str]
"""``word``: one token."""
rest: StrType[str]
"""``rest``: the rest of the line."""

def int_range(
    lo: _Int | None = None, hi: _Int | None = None, *, below: _Int | None = None
) -> IntType[int]:
    """``lo..hi``, both included, or ``lo..<below``. Either end may be left
    out, but not both."""

def at_least(lo: _Int) -> IntType[int]:
    """``lo..``, the integers from ``lo`` up."""

# Patterns

class Part:
    """A part of a pattern other than text or a capture handle."""

class Discard:
    """The discard ``_``: a target that reads without storing, and ``{_}`` in
    a pattern."""

def discard() -> Discard:
    """``_``: read a value without storing it, or ``{_}`` in a pattern."""

def bind(handle: Field[Any], ty: Type[Any] | None = None) -> Part:
    """``{x:type}``, binding a forward or redefined handle; without a type,
    ``{x}``, which reads a word."""

def ref(handle: Field[Any]) -> Part:
    """``{$x}``, matching the current value of ``x``."""

def validate(ty: Type[Any]) -> Part:
    """``{:type}``, which checks the text without storing it."""

def blank() -> Part:
    """``{}``, which matches a word without storing it."""

def discard_as(ty: Type[Any]) -> Part:
    """``{_:type}``, a discarded capture with a type."""

rest_of_line: Part
"""The trailing ``...`` of a pattern."""

_PatternPart: TypeAlias = str | Field[Any] | Part | Discard

if sys.version_info >= (3, 14):
    from string.templatelib import Template

    _Pattern: TypeAlias = str | tuple[_PatternPart, ...] | Template
else:
    _Pattern: TypeAlias = str | tuple[_PatternPart, ...]

# Errors and call sites

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
    ) -> None: ...
    @classmethod
    def from_error(cls, error: Error, text: str) -> BuildError:
        """The error for emitted text that the language rejected."""

    @property
    def emitted_line(self) -> str | None:
        """The line of emitted text that the language error points at."""

def track_caller(function: _C) -> _C:
    """Make a helper function report its caller's line as the call site of
    the statements it writes, as builder methods do."""

# Scopes and blocks

class Block(Generic[_S_co]):
    """The guard of a block statement: ``with`` gives the block's scope. A
    guard that is never entered writes an empty block."""

    def __enter__(self) -> _S_co: ...
    def __exit__(self, *exception: object) -> None: ...

class SectionsGuard(Block[Arms[_R_co, _O_co]], Generic[_R_co, _O_co]):
    """The guard of ``sections``: clauses chain, and ``with`` gives the arms."""

    def until(
        self, *terminators: str, store: str | Field[Any] | Discard | None = None
    ) -> SectionsGuard[_R_co, _O_co]:
        """``until "text"...``, with ``store x`` keeping the terminator; to read
        it, pass a handle from ``forward``."""

    def skip_other(self) -> SectionsGuard[_R_co, _O_co]:
        """``skip other``."""

class Arms(Generic[_R_co, _O_co]):
    """The block of a ``sections`` statement, which holds only arms: ``_R_co``
    is the scope of a required arm and ``_O_co`` of an optional one."""

    @overload
    def arm(
        self, pattern: _Pattern, *, optional: Literal[False] = False, last: bool = False
    ) -> ArmGuard[_R_co]: ...
    @overload
    def arm(self, pattern: _Pattern, *, optional: bool, last: bool = False) -> ArmGuard[_O_co]: ...

class ArmGuard(Generic[_S_co]):
    """The guard of an arm: ``with`` gives the arm's block."""

    def __enter__(self) -> _S_co: ...
    def __exit__(self, *exception: object) -> None: ...

class Scope(Generic[_N_co]):
    """An open block, with the statements whose types do not depend on its
    depth. A scope accepts statements only while it is the innermost open
    block."""

    def set(self, handle: Field[Any], value: _Operand) -> None:
        """``let x = value`` through a handle, keeping its type.

        The value must have the handle's value type and, where both are known,
        its depth. To change either, use ``let`` with the handle as the target,
        which returns a ``DynField``.
        """

    def check(self, condition: _Condition, message: str | None = None) -> None:
        """``check condition``, with an optional message for when it fails."""

    def expect(self, *words: str) -> None:
        """``expect "text"...``."""

    def line(self, pattern: _Pattern) -> None:
        """``line "pattern"``."""

    def find(self, pattern: _Pattern) -> None:
        """``find "pattern"``."""

    def skip_line(self) -> None:
        """``skip line``."""

    def skip_lines(self, count: _Int) -> None:
        """``skip count lines``."""

    def index(self, position: _Int) -> None:
        """``index position``."""

    @overload
    def define_type(self, name: str, ty: IntType[_U]) -> IntType[_U]: ...
    @overload
    def define_type(self, name: str, ty: FloatType[_U]) -> FloatType[_U]: ...
    @overload
    def define_type(self, name: str, ty: NumType[_U]) -> NumType[_U]: ...
    @overload
    def define_type(self, name: str, ty: StrType[_U]) -> StrType[_U]: ...
    def forward(self, name: str, kind: type[_F]) -> _F:
        """A handle for a name that a later statement defines, of the handle
        type ``kind``, such as ``ib.IntField[list[int]]`` or ``ib.DynField``."""

    def undefined(self, name: str) -> DynField:
        """A handle for a name deliberately left undefined."""

    def forward_record(self, each: str, list: str) -> Record:
        """The record of a repeat that comes later in the text, printed with
        the names given here. Bind it with ``record_as``."""

class _Nested(Scope[_N_co]):
    @property
    def record(self) -> Record:
        """The record of the nearest repeat."""

    def each_name(self, name: str) -> None:
        """Print this repeat's ``each`` name as ``name``, which may be ``_``."""

    def as_name(self, name: str) -> None:
        """Print this repeat's ``as`` name as ``name``, which may be ``_``."""

    def record_as(self, record: Record) -> None:
        """Bind this repeat to a record from ``forward_record``."""

class Scope0(Scope[_N_co]):
    """A scope outside every repeat. ``_N`` is ``Required`` when its
    statements always execute and ``None`` when they may not."""

    @overload
    def read(self, target: str, ty: IntType[_U]) -> IntField[_U | _N_co]: ...
    @overload
    def read(self, target: str, ty: FloatType[_U]) -> FloatField[_U | _N_co]: ...
    @overload
    def read(self, target: str, ty: NumType[_U]) -> NumField[_U | _N_co]: ...
    @overload
    def read(self, target: str, ty: StrType[_U]) -> StrField[_U | _N_co]: ...
    @overload
    def read(self, target: Field[Any], ty: Type[Any]) -> DynField: ...
    @overload
    def read(self, target: Discard, ty: Type[Any]) -> None: ...
    @overload
    def else_read_if(self, target: str, ty: IntType[_U]) -> IntField[_U | None]: ...
    @overload
    def else_read_if(self, target: str, ty: FloatType[_U]) -> FloatField[_U | None]: ...
    @overload
    def else_read_if(self, target: str, ty: NumType[_U]) -> NumField[_U | None]: ...
    @overload
    def else_read_if(self, target: str, ty: StrType[_U]) -> StrField[_U | None]: ...
    @overload
    def else_read_if(self, target: Field[Any], ty: Type[Any]) -> DynField: ...
    @overload
    def list(
        self, target: str, reading: ListReading[IntType[_U]]
    ) -> IntField[builtins.list[_U] | _N_co]: ...
    @overload
    def list(
        self, target: str, reading: ListReading[FloatType[_U]]
    ) -> FloatField[builtins.list[_U] | _N_co]: ...
    @overload
    def list(
        self, target: str, reading: ListReading[NumType[_U]]
    ) -> NumField[builtins.list[_U] | _N_co]: ...
    @overload
    def list(
        self, target: str, reading: ListReading[StrType[_U]]
    ) -> StrField[builtins.list[_U] | _N_co]: ...
    @overload
    def list(self, target: Field[Any], reading: ListReading[Any]) -> DynField: ...
    @overload
    def list(self, target: Discard, reading: ListReading[Any]) -> None: ...
    @overload
    def let(self, target: str, value: IntField[_U]) -> IntField[_U | _N_co]: ...
    @overload
    def let(self, target: str, value: IntLookup) -> IntField[Value | _N_co]: ...
    @overload
    def let(self, target: str, value: int | IntExpr) -> IntField[int | _N_co]: ...
    @overload
    def let(self, target: str, value: FloatField[_U]) -> FloatField[_U | _N_co]: ...
    @overload
    def let(self, target: str, value: FloatLookup) -> FloatField[Value | _N_co]: ...
    @overload
    def let(self, target: str, value: float | FloatExpr) -> FloatField[float | _N_co]: ...
    @overload
    def let(self, target: str, value: NumField[_U]) -> NumField[_U | _N_co]: ...
    @overload
    def let(self, target: str, value: NumLookup) -> NumField[Value | _N_co]: ...
    @overload
    def let(self, target: str, value: NumExpr) -> NumField[int | float | _N_co]: ...
    @overload
    def let(self, target: str, value: StrField[_U]) -> StrField[_U | _N_co]: ...
    @overload
    def let(self, target: str, value: StrLookup) -> StrField[Value | _N_co]: ...
    @overload
    def let(self, target: str, value: str | StrExpr) -> StrField[str | _N_co]: ...
    @overload
    def let(self, target: str, value: DynExpr) -> DynField: ...
    @overload
    def let(self, target: Field[Any], value: _Operand) -> DynField: ...
    @overload
    def param(self, target: str | Field[Any]) -> DynField: ...
    @overload
    def param(self, target: str, default: IntLookup) -> IntField[Value | _N_co]: ...
    @overload
    def param(self, target: str, default: int | IntExpr) -> IntField[int | _N_co]: ...
    @overload
    def param(self, target: str, default: FloatLookup) -> FloatField[Value | _N_co]: ...
    @overload
    def param(self, target: str, default: float | FloatExpr) -> FloatField[float | _N_co]: ...
    @overload
    def param(self, target: str, default: NumLookup) -> NumField[Value | _N_co]: ...
    @overload
    def param(self, target: str, default: NumExpr) -> NumField[int | float | _N_co]: ...
    @overload
    def param(self, target: str, default: StrLookup) -> StrField[Value | _N_co]: ...
    @overload
    def param(self, target: str, default: str | StrExpr) -> StrField[str | _N_co]: ...
    @overload
    def param(self, target: str, default: DynExpr) -> DynField: ...
    @overload
    def param(self, target: Field[Any], default: _Operand) -> DynField: ...
    @overload
    def capture(self, name: str, ty: None = None) -> StrField[str | _N_co]: ...
    @overload
    def capture(self, name: str, ty: IntType[_U]) -> IntField[_U | _N_co]: ...
    @overload
    def capture(self, name: str, ty: FloatType[_U]) -> FloatField[_U | _N_co]: ...
    @overload
    def capture(self, name: str, ty: NumType[_U]) -> NumField[_U | _N_co]: ...
    @overload
    def capture(self, name: str, ty: StrType[_U]) -> StrField[_U | _N_co]: ...
    @overload
    def optional_capture(self, name: str, ty: None = None) -> StrField[str | None]: ...
    @overload
    def optional_capture(self, name: str, ty: IntType[_U]) -> IntField[_U | None]: ...
    @overload
    def optional_capture(self, name: str, ty: FloatType[_U]) -> FloatField[_U | None]: ...
    @overload
    def optional_capture(self, name: str, ty: NumType[_U]) -> NumField[_U | None]: ...
    @overload
    def optional_capture(self, name: str, ty: StrType[_U]) -> StrField[_U | None]: ...
    def repeat(self, count: _Int) -> Block[Scope1[_N_co]]: ...
    def repeat_until_eof(self) -> Block[Scope1[_N_co]]: ...
    def repeat_until(
        self, *terminators: str, store: str | Field[Any] | Discard | None = None
    ) -> Block[Scope1[_N_co]]: ...
    def sections(
        self,
        *,
        until: str | tuple[str, ...] = (),
        skip_other: bool = False,
        store: str | Field[Any] | Discard | None = None,
    ) -> SectionsGuard[Scope0[_N_co], Scope0[None]]: ...
    def if_(self, condition: _Condition) -> Block[Scope0[None]]: ...
    def else_if(self, condition: _Condition) -> Block[Scope0[None]]: ...
    def else_(self) -> Block[Scope0[None]]: ...

class Scope1(_Nested[_N_co]):
    """A scope inside one repeat: its definitions are lists."""

    @overload
    def read(self, target: str, ty: IntType[_U]) -> IntField[builtins.list[_U] | _N_co]: ...
    @overload
    def read(self, target: str, ty: FloatType[_U]) -> FloatField[builtins.list[_U] | _N_co]: ...
    @overload
    def read(self, target: str, ty: NumType[_U]) -> NumField[builtins.list[_U] | _N_co]: ...
    @overload
    def read(self, target: str, ty: StrType[_U]) -> StrField[builtins.list[_U] | _N_co]: ...
    @overload
    def read(self, target: Field[Any], ty: Type[Any]) -> DynField: ...
    @overload
    def read(self, target: Discard, ty: Type[Any]) -> None: ...
    @overload
    def else_read_if(self, target: str, ty: IntType[_U]) -> IntField[builtins.list[_U] | None]: ...
    @overload
    def else_read_if(
        self, target: str, ty: FloatType[_U]
    ) -> FloatField[builtins.list[_U] | None]: ...
    @overload
    def else_read_if(self, target: str, ty: NumType[_U]) -> NumField[builtins.list[_U] | None]: ...
    @overload
    def else_read_if(self, target: str, ty: StrType[_U]) -> StrField[builtins.list[_U] | None]: ...
    @overload
    def else_read_if(self, target: Field[Any], ty: Type[Any]) -> DynField: ...
    @overload
    def list(
        self, target: str, reading: ListReading[IntType[_U]]
    ) -> IntField[builtins.list[builtins.list[_U]] | _N_co]: ...
    @overload
    def list(
        self, target: str, reading: ListReading[FloatType[_U]]
    ) -> FloatField[builtins.list[builtins.list[_U]] | _N_co]: ...
    @overload
    def list(
        self, target: str, reading: ListReading[NumType[_U]]
    ) -> NumField[builtins.list[builtins.list[_U]] | _N_co]: ...
    @overload
    def list(
        self, target: str, reading: ListReading[StrType[_U]]
    ) -> StrField[builtins.list[builtins.list[_U]] | _N_co]: ...
    @overload
    def list(self, target: Field[Any], reading: ListReading[Any]) -> DynField: ...
    @overload
    def list(self, target: Discard, reading: ListReading[Any]) -> None: ...
    @overload
    def let(self, target: str, value: IntLookup) -> IntField[Value | _N_co]: ...
    @overload
    def let(self, target: str, value: int | IntExpr) -> IntField[builtins.list[int] | _N_co]: ...
    @overload
    def let(self, target: str, value: FloatLookup) -> FloatField[Value | _N_co]: ...
    @overload
    def let(
        self, target: str, value: float | FloatExpr
    ) -> FloatField[builtins.list[float] | _N_co]: ...
    @overload
    def let(self, target: str, value: NumLookup) -> NumField[Value | _N_co]: ...
    @overload
    def let(self, target: str, value: NumExpr) -> NumField[builtins.list[int | float] | _N_co]: ...
    @overload
    def let(self, target: str, value: StrLookup) -> StrField[Value | _N_co]: ...
    @overload
    def let(self, target: str, value: str | StrExpr) -> StrField[builtins.list[str] | _N_co]: ...
    @overload
    def let(self, target: str, value: DynExpr) -> DynField: ...
    @overload
    def let(self, target: Field[Any], value: _Operand) -> DynField: ...
    @overload
    def param(self, target: str | Field[Any]) -> DynField: ...
    @overload
    def param(self, target: str, default: IntLookup) -> IntField[Value | _N_co]: ...
    @overload
    def param(
        self, target: str, default: int | IntExpr
    ) -> IntField[builtins.list[int] | _N_co]: ...
    @overload
    def param(self, target: str, default: FloatLookup) -> FloatField[Value | _N_co]: ...
    @overload
    def param(
        self, target: str, default: float | FloatExpr
    ) -> FloatField[builtins.list[float] | _N_co]: ...
    @overload
    def param(self, target: str, default: NumLookup) -> NumField[Value | _N_co]: ...
    @overload
    def param(
        self, target: str, default: NumExpr
    ) -> NumField[builtins.list[int | float] | _N_co]: ...
    @overload
    def param(self, target: str, default: StrLookup) -> StrField[Value | _N_co]: ...
    @overload
    def param(
        self, target: str, default: str | StrExpr
    ) -> StrField[builtins.list[str] | _N_co]: ...
    @overload
    def param(self, target: str, default: DynExpr) -> DynField: ...
    @overload
    def param(self, target: Field[Any], default: _Operand) -> DynField: ...
    @overload
    def capture(self, name: str, ty: None = None) -> StrField[builtins.list[str] | _N_co]: ...
    @overload
    def capture(self, name: str, ty: IntType[_U]) -> IntField[builtins.list[_U] | _N_co]: ...
    @overload
    def capture(self, name: str, ty: FloatType[_U]) -> FloatField[builtins.list[_U] | _N_co]: ...
    @overload
    def capture(self, name: str, ty: NumType[_U]) -> NumField[builtins.list[_U] | _N_co]: ...
    @overload
    def capture(self, name: str, ty: StrType[_U]) -> StrField[builtins.list[_U] | _N_co]: ...
    @overload
    def optional_capture(
        self, name: str, ty: None = None
    ) -> StrField[builtins.list[str] | None]: ...
    @overload
    def optional_capture(
        self, name: str, ty: IntType[_U]
    ) -> IntField[builtins.list[_U] | None]: ...
    @overload
    def optional_capture(
        self, name: str, ty: FloatType[_U]
    ) -> FloatField[builtins.list[_U] | None]: ...
    @overload
    def optional_capture(
        self, name: str, ty: NumType[_U]
    ) -> NumField[builtins.list[_U] | None]: ...
    @overload
    def optional_capture(
        self, name: str, ty: StrType[_U]
    ) -> StrField[builtins.list[_U] | None]: ...
    def repeat(self, count: _Int) -> Block[Scope2[_N_co]]: ...
    def repeat_until_eof(self) -> Block[Scope2[_N_co]]: ...
    def repeat_until(
        self, *terminators: str, store: str | Field[Any] | Discard | None = None
    ) -> Block[Scope2[_N_co]]: ...
    def sections(
        self,
        *,
        until: str | tuple[str, ...] = (),
        skip_other: bool = False,
        store: str | Field[Any] | Discard | None = None,
    ) -> SectionsGuard[Scope1[_N_co], Scope1[None]]: ...
    def if_(self, condition: _Condition) -> Block[Scope1[None]]: ...
    def else_if(self, condition: _Condition) -> Block[Scope1[None]]: ...
    def else_(self) -> Block[Scope1[None]]: ...

class Scope2(_Nested[_N_co]):
    """A scope inside two repeats."""

    @overload
    def read(
        self, target: str, ty: IntType[_U]
    ) -> IntField[builtins.list[builtins.list[_U]] | _N_co]: ...
    @overload
    def read(
        self, target: str, ty: FloatType[_U]
    ) -> FloatField[builtins.list[builtins.list[_U]] | _N_co]: ...
    @overload
    def read(
        self, target: str, ty: NumType[_U]
    ) -> NumField[builtins.list[builtins.list[_U]] | _N_co]: ...
    @overload
    def read(
        self, target: str, ty: StrType[_U]
    ) -> StrField[builtins.list[builtins.list[_U]] | _N_co]: ...
    @overload
    def read(self, target: Field[Any], ty: Type[Any]) -> DynField: ...
    @overload
    def read(self, target: Discard, ty: Type[Any]) -> None: ...
    @overload
    def else_read_if(
        self, target: str, ty: IntType[_U]
    ) -> IntField[builtins.list[builtins.list[_U]] | None]: ...
    @overload
    def else_read_if(
        self, target: str, ty: FloatType[_U]
    ) -> FloatField[builtins.list[builtins.list[_U]] | None]: ...
    @overload
    def else_read_if(
        self, target: str, ty: NumType[_U]
    ) -> NumField[builtins.list[builtins.list[_U]] | None]: ...
    @overload
    def else_read_if(
        self, target: str, ty: StrType[_U]
    ) -> StrField[builtins.list[builtins.list[_U]] | None]: ...
    @overload
    def else_read_if(self, target: Field[Any], ty: Type[Any]) -> DynField: ...
    @overload
    def list(
        self, target: str, reading: ListReading[IntType[_U]]
    ) -> IntField[builtins.list[builtins.list[builtins.list[_U]]] | _N_co]: ...
    @overload
    def list(
        self, target: str, reading: ListReading[FloatType[_U]]
    ) -> FloatField[builtins.list[builtins.list[builtins.list[_U]]] | _N_co]: ...
    @overload
    def list(
        self, target: str, reading: ListReading[NumType[_U]]
    ) -> NumField[builtins.list[builtins.list[builtins.list[_U]]] | _N_co]: ...
    @overload
    def list(
        self, target: str, reading: ListReading[StrType[_U]]
    ) -> StrField[builtins.list[builtins.list[builtins.list[_U]]] | _N_co]: ...
    @overload
    def list(self, target: Field[Any], reading: ListReading[Any]) -> DynField: ...
    @overload
    def list(self, target: Discard, reading: ListReading[Any]) -> None: ...
    @overload
    def let(self, target: str, value: IntLookup) -> IntField[Value | _N_co]: ...
    @overload
    def let(
        self, target: str, value: int | IntExpr
    ) -> IntField[builtins.list[builtins.list[int]] | _N_co]: ...
    @overload
    def let(self, target: str, value: FloatLookup) -> FloatField[Value | _N_co]: ...
    @overload
    def let(
        self, target: str, value: float | FloatExpr
    ) -> FloatField[builtins.list[builtins.list[float]] | _N_co]: ...
    @overload
    def let(self, target: str, value: NumLookup) -> NumField[Value | _N_co]: ...
    @overload
    def let(
        self, target: str, value: NumExpr
    ) -> NumField[builtins.list[builtins.list[int | float]] | _N_co]: ...
    @overload
    def let(self, target: str, value: StrLookup) -> StrField[Value | _N_co]: ...
    @overload
    def let(
        self, target: str, value: str | StrExpr
    ) -> StrField[builtins.list[builtins.list[str]] | _N_co]: ...
    @overload
    def let(self, target: str, value: DynExpr) -> DynField: ...
    @overload
    def let(self, target: Field[Any], value: _Operand) -> DynField: ...
    @overload
    def param(self, target: str | Field[Any]) -> DynField: ...
    @overload
    def param(self, target: str, default: IntLookup) -> IntField[Value | _N_co]: ...
    @overload
    def param(
        self, target: str, default: int | IntExpr
    ) -> IntField[builtins.list[builtins.list[int]] | _N_co]: ...
    @overload
    def param(self, target: str, default: FloatLookup) -> FloatField[Value | _N_co]: ...
    @overload
    def param(
        self, target: str, default: float | FloatExpr
    ) -> FloatField[builtins.list[builtins.list[float]] | _N_co]: ...
    @overload
    def param(self, target: str, default: NumLookup) -> NumField[Value | _N_co]: ...
    @overload
    def param(
        self, target: str, default: NumExpr
    ) -> NumField[builtins.list[builtins.list[int | float]] | _N_co]: ...
    @overload
    def param(self, target: str, default: StrLookup) -> StrField[Value | _N_co]: ...
    @overload
    def param(
        self, target: str, default: str | StrExpr
    ) -> StrField[builtins.list[builtins.list[str]] | _N_co]: ...
    @overload
    def param(self, target: str, default: DynExpr) -> DynField: ...
    @overload
    def param(self, target: Field[Any], default: _Operand) -> DynField: ...
    @overload
    def capture(
        self, name: str, ty: None = None
    ) -> StrField[builtins.list[builtins.list[str]] | _N_co]: ...
    @overload
    def capture(
        self, name: str, ty: IntType[_U]
    ) -> IntField[builtins.list[builtins.list[_U]] | _N_co]: ...
    @overload
    def capture(
        self, name: str, ty: FloatType[_U]
    ) -> FloatField[builtins.list[builtins.list[_U]] | _N_co]: ...
    @overload
    def capture(
        self, name: str, ty: NumType[_U]
    ) -> NumField[builtins.list[builtins.list[_U]] | _N_co]: ...
    @overload
    def capture(
        self, name: str, ty: StrType[_U]
    ) -> StrField[builtins.list[builtins.list[_U]] | _N_co]: ...
    @overload
    def optional_capture(
        self, name: str, ty: None = None
    ) -> StrField[builtins.list[builtins.list[str]] | None]: ...
    @overload
    def optional_capture(
        self, name: str, ty: IntType[_U]
    ) -> IntField[builtins.list[builtins.list[_U]] | None]: ...
    @overload
    def optional_capture(
        self, name: str, ty: FloatType[_U]
    ) -> FloatField[builtins.list[builtins.list[_U]] | None]: ...
    @overload
    def optional_capture(
        self, name: str, ty: NumType[_U]
    ) -> NumField[builtins.list[builtins.list[_U]] | None]: ...
    @overload
    def optional_capture(
        self, name: str, ty: StrType[_U]
    ) -> StrField[builtins.list[builtins.list[_U]] | None]: ...
    def repeat(self, count: _Int) -> Block[Scope3[_N_co]]: ...
    def repeat_until_eof(self) -> Block[Scope3[_N_co]]: ...
    def repeat_until(
        self, *terminators: str, store: str | Field[Any] | Discard | None = None
    ) -> Block[Scope3[_N_co]]: ...
    def sections(
        self,
        *,
        until: str | tuple[str, ...] = (),
        skip_other: bool = False,
        store: str | Field[Any] | Discard | None = None,
    ) -> SectionsGuard[Scope2[_N_co], Scope2[None]]: ...
    def if_(self, condition: _Condition) -> Block[Scope2[None]]: ...
    def else_if(self, condition: _Condition) -> Block[Scope2[None]]: ...
    def else_(self) -> Block[Scope2[None]]: ...

class Scope3(_Nested[_N_co]):
    """A scope inside three repeats, the deepest with typed handles."""

    @overload
    def read(
        self, target: str, ty: IntType[_U]
    ) -> IntField[builtins.list[builtins.list[builtins.list[_U]]] | _N_co]: ...
    @overload
    def read(
        self, target: str, ty: FloatType[_U]
    ) -> FloatField[builtins.list[builtins.list[builtins.list[_U]]] | _N_co]: ...
    @overload
    def read(
        self, target: str, ty: NumType[_U]
    ) -> NumField[builtins.list[builtins.list[builtins.list[_U]]] | _N_co]: ...
    @overload
    def read(
        self, target: str, ty: StrType[_U]
    ) -> StrField[builtins.list[builtins.list[builtins.list[_U]]] | _N_co]: ...
    @overload
    def read(self, target: Field[Any], ty: Type[Any]) -> DynField: ...
    @overload
    def read(self, target: Discard, ty: Type[Any]) -> None: ...
    @overload
    def else_read_if(
        self, target: str, ty: IntType[_U]
    ) -> IntField[builtins.list[builtins.list[builtins.list[_U]]] | None]: ...
    @overload
    def else_read_if(
        self, target: str, ty: FloatType[_U]
    ) -> FloatField[builtins.list[builtins.list[builtins.list[_U]]] | None]: ...
    @overload
    def else_read_if(
        self, target: str, ty: NumType[_U]
    ) -> NumField[builtins.list[builtins.list[builtins.list[_U]]] | None]: ...
    @overload
    def else_read_if(
        self, target: str, ty: StrType[_U]
    ) -> StrField[builtins.list[builtins.list[builtins.list[_U]]] | None]: ...
    @overload
    def else_read_if(self, target: Field[Any], ty: Type[Any]) -> DynField: ...
    @overload
    def list(
        self, target: str, reading: ListReading[IntType[_U]]
    ) -> IntField[builtins.list[builtins.list[builtins.list[builtins.list[_U]]]] | _N_co]: ...
    @overload
    def list(
        self, target: str, reading: ListReading[FloatType[_U]]
    ) -> FloatField[builtins.list[builtins.list[builtins.list[builtins.list[_U]]]] | _N_co]: ...
    @overload
    def list(
        self, target: str, reading: ListReading[NumType[_U]]
    ) -> NumField[builtins.list[builtins.list[builtins.list[builtins.list[_U]]]] | _N_co]: ...
    @overload
    def list(
        self, target: str, reading: ListReading[StrType[_U]]
    ) -> StrField[builtins.list[builtins.list[builtins.list[builtins.list[_U]]]] | _N_co]: ...
    @overload
    def list(self, target: Field[Any], reading: ListReading[Any]) -> DynField: ...
    @overload
    def list(self, target: Discard, reading: ListReading[Any]) -> None: ...
    @overload
    def let(self, target: str, value: IntLookup) -> IntField[Value | _N_co]: ...
    @overload
    def let(
        self, target: str, value: int | IntExpr
    ) -> IntField[builtins.list[builtins.list[builtins.list[int]]] | _N_co]: ...
    @overload
    def let(self, target: str, value: FloatLookup) -> FloatField[Value | _N_co]: ...
    @overload
    def let(
        self, target: str, value: float | FloatExpr
    ) -> FloatField[builtins.list[builtins.list[builtins.list[float]]] | _N_co]: ...
    @overload
    def let(self, target: str, value: NumLookup) -> NumField[Value | _N_co]: ...
    @overload
    def let(
        self, target: str, value: NumExpr
    ) -> NumField[builtins.list[builtins.list[builtins.list[int | float]]] | _N_co]: ...
    @overload
    def let(self, target: str, value: StrLookup) -> StrField[Value | _N_co]: ...
    @overload
    def let(
        self, target: str, value: str | StrExpr
    ) -> StrField[builtins.list[builtins.list[builtins.list[str]]] | _N_co]: ...
    @overload
    def let(self, target: str, value: DynExpr) -> DynField: ...
    @overload
    def let(self, target: Field[Any], value: _Operand) -> DynField: ...
    @overload
    def param(self, target: str | Field[Any]) -> DynField: ...
    @overload
    def param(self, target: str, default: IntLookup) -> IntField[Value | _N_co]: ...
    @overload
    def param(
        self, target: str, default: int | IntExpr
    ) -> IntField[builtins.list[builtins.list[builtins.list[int]]] | _N_co]: ...
    @overload
    def param(self, target: str, default: FloatLookup) -> FloatField[Value | _N_co]: ...
    @overload
    def param(
        self, target: str, default: float | FloatExpr
    ) -> FloatField[builtins.list[builtins.list[builtins.list[float]]] | _N_co]: ...
    @overload
    def param(self, target: str, default: NumLookup) -> NumField[Value | _N_co]: ...
    @overload
    def param(
        self, target: str, default: NumExpr
    ) -> NumField[builtins.list[builtins.list[builtins.list[int | float]]] | _N_co]: ...
    @overload
    def param(self, target: str, default: StrLookup) -> StrField[Value | _N_co]: ...
    @overload
    def param(
        self, target: str, default: str | StrExpr
    ) -> StrField[builtins.list[builtins.list[builtins.list[str]]] | _N_co]: ...
    @overload
    def param(self, target: str, default: DynExpr) -> DynField: ...
    @overload
    def param(self, target: Field[Any], default: _Operand) -> DynField: ...
    @overload
    def capture(
        self, name: str, ty: None = None
    ) -> StrField[builtins.list[builtins.list[builtins.list[str]]] | _N_co]: ...
    @overload
    def capture(
        self, name: str, ty: IntType[_U]
    ) -> IntField[builtins.list[builtins.list[builtins.list[_U]]] | _N_co]: ...
    @overload
    def capture(
        self, name: str, ty: FloatType[_U]
    ) -> FloatField[builtins.list[builtins.list[builtins.list[_U]]] | _N_co]: ...
    @overload
    def capture(
        self, name: str, ty: NumType[_U]
    ) -> NumField[builtins.list[builtins.list[builtins.list[_U]]] | _N_co]: ...
    @overload
    def capture(
        self, name: str, ty: StrType[_U]
    ) -> StrField[builtins.list[builtins.list[builtins.list[_U]]] | _N_co]: ...
    @overload
    def optional_capture(
        self, name: str, ty: None = None
    ) -> StrField[builtins.list[builtins.list[builtins.list[str]]] | None]: ...
    @overload
    def optional_capture(
        self, name: str, ty: IntType[_U]
    ) -> IntField[builtins.list[builtins.list[builtins.list[_U]]] | None]: ...
    @overload
    def optional_capture(
        self, name: str, ty: FloatType[_U]
    ) -> FloatField[builtins.list[builtins.list[builtins.list[_U]]] | None]: ...
    @overload
    def optional_capture(
        self, name: str, ty: NumType[_U]
    ) -> NumField[builtins.list[builtins.list[builtins.list[_U]]] | None]: ...
    @overload
    def optional_capture(
        self, name: str, ty: StrType[_U]
    ) -> StrField[builtins.list[builtins.list[builtins.list[_U]]] | None]: ...
    def repeat(self, count: _Int) -> Block[ScopeN[_N_co]]: ...
    def repeat_until_eof(self) -> Block[ScopeN[_N_co]]: ...
    def repeat_until(
        self, *terminators: str, store: str | Field[Any] | Discard | None = None
    ) -> Block[ScopeN[_N_co]]: ...
    def sections(
        self,
        *,
        until: str | tuple[str, ...] = (),
        skip_other: bool = False,
        store: str | Field[Any] | Discard | None = None,
    ) -> SectionsGuard[Scope3[_N_co], Scope3[None]]: ...
    def if_(self, condition: _Condition) -> Block[Scope3[None]]: ...
    def else_if(self, condition: _Condition) -> Block[Scope3[None]]: ...
    def else_(self) -> Block[Scope3[None]]: ...

class ScopeN(_Nested[_N_co]):
    """A scope inside more repeats than handles can type: its definitions
    give ``DynField``, and dimensions still nest."""

    @overload
    def read(self, target: str, ty: IntType[_U]) -> DynField: ...
    @overload
    def read(self, target: str, ty: FloatType[_U]) -> DynField: ...
    @overload
    def read(self, target: str, ty: NumType[_U]) -> DynField: ...
    @overload
    def read(self, target: str, ty: StrType[_U]) -> DynField: ...
    @overload
    def read(self, target: Field[Any], ty: Type[Any]) -> DynField: ...
    @overload
    def read(self, target: Discard, ty: Type[Any]) -> None: ...
    @overload
    def else_read_if(self, target: str, ty: IntType[_U]) -> DynField: ...
    @overload
    def else_read_if(self, target: str, ty: FloatType[_U]) -> DynField: ...
    @overload
    def else_read_if(self, target: str, ty: NumType[_U]) -> DynField: ...
    @overload
    def else_read_if(self, target: str, ty: StrType[_U]) -> DynField: ...
    @overload
    def else_read_if(self, target: Field[Any], ty: Type[Any]) -> DynField: ...
    @overload
    def list(self, target: str, reading: ListReading[IntType[_U]]) -> DynField: ...
    @overload
    def list(self, target: str, reading: ListReading[FloatType[_U]]) -> DynField: ...
    @overload
    def list(self, target: str, reading: ListReading[NumType[_U]]) -> DynField: ...
    @overload
    def list(self, target: str, reading: ListReading[StrType[_U]]) -> DynField: ...
    @overload
    def list(self, target: Field[Any], reading: ListReading[Any]) -> DynField: ...
    @overload
    def list(self, target: Discard, reading: ListReading[Any]) -> None: ...
    @overload
    def let(self, target: str, value: int | IntExpr) -> DynField: ...
    @overload
    def let(self, target: str, value: float | FloatExpr) -> DynField: ...
    @overload
    def let(self, target: str, value: NumExpr) -> DynField: ...
    @overload
    def let(self, target: str, value: str | StrExpr) -> DynField: ...
    @overload
    def let(self, target: str, value: DynExpr) -> DynField: ...
    @overload
    def let(self, target: Field[Any], value: _Operand) -> DynField: ...
    @overload
    def param(self, target: str | Field[Any]) -> DynField: ...
    @overload
    def param(self, target: str, default: int | IntExpr) -> DynField: ...
    @overload
    def param(self, target: str, default: float | FloatExpr) -> DynField: ...
    @overload
    def param(self, target: str, default: NumExpr) -> DynField: ...
    @overload
    def param(self, target: str, default: str | StrExpr) -> DynField: ...
    @overload
    def param(self, target: str, default: DynExpr) -> DynField: ...
    @overload
    def param(self, target: Field[Any], default: _Operand) -> DynField: ...
    @overload
    def capture(self, name: str, ty: None = None) -> DynField: ...
    @overload
    def capture(self, name: str, ty: IntType[_U]) -> DynField: ...
    @overload
    def capture(self, name: str, ty: FloatType[_U]) -> DynField: ...
    @overload
    def capture(self, name: str, ty: NumType[_U]) -> DynField: ...
    @overload
    def capture(self, name: str, ty: StrType[_U]) -> DynField: ...
    @overload
    def optional_capture(self, name: str, ty: None = None) -> DynField: ...
    @overload
    def optional_capture(self, name: str, ty: IntType[_U]) -> DynField: ...
    @overload
    def optional_capture(self, name: str, ty: FloatType[_U]) -> DynField: ...
    @overload
    def optional_capture(self, name: str, ty: NumType[_U]) -> DynField: ...
    @overload
    def optional_capture(self, name: str, ty: StrType[_U]) -> DynField: ...
    def repeat(self, count: _Int) -> Block[ScopeN[_N_co]]: ...
    def repeat_until_eof(self) -> Block[ScopeN[_N_co]]: ...
    def repeat_until(
        self, *terminators: str, store: str | Field[Any] | Discard | None = None
    ) -> Block[ScopeN[_N_co]]: ...
    def sections(
        self,
        *,
        until: str | tuple[str, ...] = (),
        skip_other: bool = False,
        store: str | Field[Any] | Discard | None = None,
    ) -> SectionsGuard[ScopeN[_N_co], ScopeN[None]]: ...
    def if_(self, condition: _Condition) -> Block[ScopeN[None]]: ...
    def else_if(self, condition: _Condition) -> Block[ScopeN[None]]: ...
    def else_(self) -> Block[ScopeN[None]]: ...

class SpecBuilder(Scope0[Required]):
    """Builds a reader spec. It is the root scope, with the settings and
    statements that only the top level allows."""

    def __init__(self, format: str | None = None) -> None: ...
    def format(self, name: str) -> None:
        """``format name``, a name of this format."""

    def comment(self, *markers: str) -> None:
        """``comment "text"...``."""

    def stop(self, *markers: str) -> None:
        """``stop "text"...``."""

    def tokens(self, characters: str) -> None:
        """``tokens "characters"``."""

    def separators(self, characters: str) -> None:
        """``separators "characters"``."""

    def ignore_trailing(self) -> None:
        """``ignore trailing``."""

    def instances(self) -> Block[Scope0[Required]]:
        """``instances { ... }``."""

    def source(self) -> str:
        """The text this builder emits, without parsing it."""

    def build(self) -> Spec:
        """Print the text and parse it. The spec is an ordinary one, whose
        errors while reading name the call site of their statement."""
