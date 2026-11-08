"""Scopes, the guards of block statements, and the builder."""

from __future__ import annotations

import types
import typing
from collections.abc import Callable
from typing import Any, Generic, TypeVar, cast

from gecode_iris._data import Value
from gecode_iris._error import CallSite, Error
from gecode_iris._native import Spec
from gecode_iris.build._check import check_bindings
from gecode_iris.build._error import BuildError
from gecode_iris.build._expr import (
    FIELDS,
    DynField,
    Expr,
    Field,
    FloatField,
    IntField,
    NumField,
    Record,
    StrField,
    operand,
)
from gecode_iris.build._ops import ValueTag, describe, postfix
from gecode_iris.build._pattern import Discard, capture_spec, capture_tag, pattern_pieces
from gecode_iris.build._print import Printed, print_spec
from gecode_iris.build._site import caller
from gecode_iris.build._state import (
    NO_BUILDER,
    ROOT_BLOCK,
    Arm,
    Capture,
    Check,
    Declaration,
    Definition,
    Else,
    ElseDeclaration,
    ElseIf,
    Format,
    Forward,
    If,
    IgnoreTrailing,
    Index,
    Instances,
    Let,
    Line,
    ListStmt,
    Param,
    Repeat,
    RepeatCount,
    SectionClause,
    Sections,
    Shape,
    SkipLine,
    SkipLines,
    SkipOther,
    State,
    Stmt,
    Strings,
    Times,
    TypeDefinition,
    Undefined,
    Until,
    UntilEof,
)
from gecode_iris.build._types import (
    Declared,
    ListReading,
    NamedSpec,
    declared_of,
    named_type,
    scalar_declared,
)

N = TypeVar("N")
_S = TypeVar("_S")
_R = TypeVar("_R")
_O = TypeVar("_O")

ORIGIN = "builder.iris"
"""The origin reported for the text a builder emits."""

STATIC_DEPTHS = 3
"""Scopes up to this depth give typed handles; deeper ones give ``DynField``."""

_MISSING: Any = object()


def _ops(value: object) -> list[Any]:
    return postfix(operand(value)._node)


def _condition(value: object) -> list[Any]:
    if isinstance(value, bool):
        raise TypeError(
            "a condition is an Iris expression, not a bool: a comparison of mismatched "
            "types, such as an int with a string, gives a Python bool"
        )
    return _ops(value)


def _strings(values: tuple[object, ...], role: str) -> tuple[str, ...]:
    strings: list[str] = []
    for value in values:
        if not isinstance(value, str):
            raise TypeError(f"{role} are str, not {type(value).__name__}")
        strings.append(value)
    return tuple(strings)


def let_levels(value: Expr, depth: int) -> int | None:
    """The array levels a ``let`` of ``value`` adds in a scope at ``depth``, as
    far as its type says: a lookup may be an array, except a bare handle at
    the top level, whose depth is its own."""
    if value._tag == "dyn":
        return None
    if isinstance(value, Field):
        return value._levels if depth == 0 else None
    return None if value._open else 0


class Scope(Generic[N]):
    """An open block. ``N`` is ``None`` when its statements may not execute,
    and ``Required`` when they always do.

    A scope accepts statements only while it is the innermost open block.
    """

    __slots__ = ("_block", "_depth", "_record", "_required", "_state")

    def __init__(
        self, state: State, block: int, record: int | None, depth: int, required: bool
    ) -> None:
        self._state = state
        self._block = block
        self._record = record
        self._depth = depth
        self._required = required

    def _push(self, stmt: Stmt) -> int:
        return self._state.push(self._block, stmt)

    def _handle(self, tag: ValueTag, name: str, levels: int | None, required: bool) -> Any:
        """The handle of a new name defined in this scope."""
        if self._depth > STATIC_DEPTHS:
            return DynField(name, self._state.id, None, False)
        return FIELDS[tag](name, self._state.id, levels, required)

    def _define(
        self,
        target: object,
        value: ValueTag,
        levels: int | None,
        statement: Callable[[Definition], Stmt],
        handle_levels: int | None,
        required: bool | None = None,
    ) -> Any:
        """Resolve ``target``, push its statement, and return its handle."""
        name = _target_name(self._state, target)
        self._push(statement(Definition(name, value, levels)))
        if name is None:
            return None
        if isinstance(target, Field):
            return DynField(name, self._state.id, None, False)
        presence = self._required if required is None else required
        return self._handle(value, name, handle_levels, presence)

    def set(self, handle: Field[Any], value: Any) -> None:
        """``let x = value`` through a handle, keeping its type.

        The value must have the handle's value type and, where both are known,
        its depth. To change either, use ``let`` with the handle as the target,
        which returns a ``DynField``.
        """
        if not isinstance(handle, Field):
            raise TypeError(f"`set` takes a handle, not {type(handle).__name__}")
        expr = operand(value)
        if not isinstance(handle, DynField) and expr._tag != handle._tag:
            raise TypeError(
                f"`set` keeps the handle's value type {describe(handle._tag)}, but this value "
                f"is {describe(expr._tag)}; to change it, redefine the name with "
                "`let(handle, value)`"
            )
        own = let_levels(expr, self._depth)
        total = None if own is None else self._depth + own
        if handle._levels is not None and total is not None and handle._levels != total:
            raise TypeError(
                f"`set` keeps the handle's depth {handle._levels}, but this value has depth "
                f"{total}; to change the shape, redefine the name with `let(handle, value)`"
            )
        value_ops = postfix(expr._node)
        self._define(handle, handle._tag, own, lambda definition: Let(definition, value_ops), None)

    def check(self, condition: Any, message: str | None = None) -> None:
        """``check condition``, with an optional message for when it fails."""
        if message is not None and not isinstance(message, str):
            raise TypeError("a check's message is a str")
        self._push(Check(_condition(condition), message))

    def expect(self, *words: str) -> None:
        """``expect "text"...``."""
        self._push(Strings("expect", _strings(words, "expected words")))

    def line(self, pattern: Any) -> None:
        """``line "pattern"``."""
        self._push(Line("line", pattern_pieces(pattern)))

    def find(self, pattern: Any) -> None:
        """``find "pattern"``."""
        self._push(Line("find", pattern_pieces(pattern)))

    def skip_line(self) -> None:
        """``skip line``."""
        self._push(SkipLine())

    def skip_lines(self, count: Any) -> None:
        """``skip count lines``."""
        self._push(SkipLines(_ops(count)))

    def index(self, position: Any) -> None:
        """``index position``."""
        self._push(Index(_ops(position)))

    def define_type(self, name: str, ty: Any) -> Any:
        """``type name = type``, usable wherever a type is. The type it returns
        belongs to this declaration: ``build()`` reports a use where another
        declaration of the name is visible."""
        declared = scalar_declared(ty, "a named type")
        state = self._state
        name = state.definition_name(name)
        state.types.add(name)
        declaration = self._push(TypeDefinition(name, declared))
        return named_type(ty, Declared(NamedSpec(name, state.id, declaration)))

    def _guard(self, opener: Stmt, record: int | None, depth: int, required: bool) -> Block[Any]:
        """Push a block statement with an empty body, and return the guard
        that gives it one."""
        site = caller()
        state = self._state
        index = state.push(self._block, opener, site)
        state.push_close(self._block, index, site)
        return Block(self, index, lambda block: _scope(state, block, record, depth, required))

    def forward(self, name: str, kind: type[Any]) -> Any:
        """A handle for a name that a later statement defines, of the handle
        type ``kind``, such as ``ib.IntField[list[int]]`` or ``ib.DynField``.

        ``build()`` reports a forward handle that nothing defines, and a
        definition that does not have its type, unless an earlier one always
        precedes it. ``forward("_", ib.StrField[str])`` is the handle of the
        entry that ``store _`` keeps.
        """
        handle_type, shape = handle_kind(kind, "forward")
        state = self._state
        name = state.declare(state.spelled_name(name), Forward(shape))
        return handle_type(name, state.id, shape.levels, shape.required)

    def undefined(self, name: str) -> DynField:
        """A handle for a name deliberately left undefined, as in
        ``check !defined(name)``. Defining it fails ``build()``."""
        state = self._state
        name = state.declare(state.spelled_name(name), Undefined())
        return DynField(name, state.id, None, False)

    def forward_record(self, each: str, list: str) -> Record:
        """The record of a repeat that comes later in the text, printed with
        the names given here. Bind it with ``record_as``."""
        state = self._state
        each, list = state.spelled_name(each), state.spelled_name(list)
        record = state.new_record()
        info = state.records[record]
        info.each, info.list, info.forward = each, list, caller()
        return Record(record, state.id)

    def __repr__(self) -> str:
        return f"<{type(self).__name__}>"


class _Statements(Scope[N]):
    """The statements whose handles and blocks depend on the depth: the
    classes of each depth give them their types."""

    __slots__ = ()

    def read(self, target: Any, ty: Any) -> Any:
        """``x: type``, reading a value into ``target``: a new name, a handle to
        bind or redefine, or ``ib.discard()``."""
        declared = declared_of(ty, "a read's type")
        own = len(declared.dimensions)
        return self._define(
            target,
            ty._tag,
            own,
            lambda definition: Declaration(definition, declared),
            self._depth + own,
        )

    def list(self, target: Any, reading: ListReading[Any]) -> Any:
        """``list x: type until end``."""
        if not isinstance(reading, ListReading):
            raise TypeError("a list reads `type.until(end)`, such as `ib.int_.until(0)`")
        return self._define(
            target,
            reading._tag,
            1,
            lambda definition: ListStmt(definition, reading._declared, reading._end),
            self._depth + 1,
        )

    def let(self, target: Any, value: Any) -> Any:
        """``let x = value``."""
        expr = operand(value)
        own = let_levels(expr, self._depth)
        value_ops = postfix(expr._node)
        return self._define(
            target,
            expr._tag,
            own,
            lambda definition: Let(definition, value_ops),
            None if own is None else self._depth + own,
        )

    def param(self, target: Any, default: Any = _MISSING) -> Any:
        """``param p``, whose type the caller decides, or ``param p = default``."""
        if default is _MISSING:
            name = _target_name(self._state, target)
            self._push(Param(Definition(name, "dyn", None), None))
            if name is None:
                return None
            required = self._required and not isinstance(target, Field)
            return DynField(name, self._state.id, None, required)
        expr = operand(default)
        scalar = expr._tag != "dyn" and not expr._open
        default_ops = postfix(expr._node)
        return self._define(
            target,
            expr._tag,
            0 if scalar else None,
            lambda definition: Param(definition, default_ops),
            self._depth if scalar else None,
        )

    def _repeat(self, count: RepeatCount) -> Block[Any]:
        record = self._state.new_record()
        return self._guard(Repeat(count, record), record, self._depth + 1, self._required)

    def repeat(self, count: Any) -> Block[Any]:
        """``repeat count { ... }``, whose block is the next depth."""
        return self._repeat(Times(_ops(count)))

    def repeat_until_eof(self) -> Block[Any]:
        """``repeat until eof { ... }``."""
        return self._repeat(UntilEof())

    def repeat_until(self, *terminators: str, store: Any = None) -> Block[Any]:
        """``repeat until "text"... { ... }``. ``store`` keeps the terminator
        that ended it in a name; to read it, pass a handle from ``forward``."""
        _strings(terminators, "terminators")
        definition = None if store is None else _store_definition(self._state, store)
        return self._repeat(Until(terminators, definition))

    def sections(
        self, *, until: str | tuple[str, ...] = (), skip_other: bool = False, store: Any = None
    ) -> SectionsGuard[Any, Any]:
        """``sections``: entering it gives the block of arms. ``until`` and
        ``skip_other`` add clauses in that order; chain ``.until()`` and
        ``.skip_other()`` for any other order."""
        site = caller()
        state = self._state
        index = state.push(self._block, Sections([]), site)
        state.push_close(self._block, index, site)
        guard: SectionsGuard[Any, Any] = SectionsGuard(self, index)
        terminators = (until,) if isinstance(until, str) else until
        if terminators:
            guard.until(*terminators, store=store)
        elif store is not None:
            state.fail("unprintable", "`store` needs an `until` before it")
        if skip_other:
            guard.skip_other()
        return guard

    def if_(self, condition: Any) -> Block[Any]:
        """``if condition { ... }``. What its block defines may be absent."""
        return self._guard(If(_condition(condition)), self._record, self._depth, False)

    def _continue_if(self, form: str) -> None:
        state = self._state
        closed = state.closed
        if (
            closed is None
            or closed.close != len(state.stmts) - 1
            or closed.parent != self._block
            or not isinstance(state.stmts[closed.opener], (If, ElseIf))
        ):
            state.fail(
                "scope",
                f"`{form}` must directly follow the block of an `if_` or `else_if` "
                "in the same scope",
            )

    def else_if(self, condition: Any) -> Block[Any]:
        """``else if condition { ... }``, directly after an ``if_`` block."""
        self._continue_if("else_if")
        return self._guard(ElseIf(_condition(condition)), self._record, self._depth, False)

    def else_(self) -> Block[Any]:
        """``else { ... }``, directly after an ``if_`` or ``else_if`` block."""
        self._continue_if("else_")
        return self._guard(Else(), self._record, self._depth, False)

    def else_read_if(self, target: Any, ty: Any) -> Any:
        """``else if:type``: the else branch is a declaration of the name ``if``,
        which the target must have."""
        self._continue_if("else_read_if")
        declared = declared_of(ty, "a read's type")
        own = len(declared.dimensions)
        name = target._name if isinstance(target, Field) else target
        if name != "if":
            self._state.fail(
                "unprintable",
                "`else_read_if` declares the name `if`, which its target must have",
            )
        return self._define(
            target,
            ty._tag,
            own,
            lambda definition: ElseDeclaration(definition, declared),
            self._depth + own,
            required=False,
        )

    def capture(self, name: str, ty: Any = None) -> Any:
        """A handle for a pattern to bind, at this scope's depth and presence.
        Without a type the capture is ``{x}``, which reads a word."""
        return self._capture(name, ty, self._required)

    def optional_capture(self, name: str, ty: Any = None) -> Any:
        """``capture`` for an optional arm or a conditional block."""
        return self._capture(name, ty, False)

    def _capture(self, name: str, ty: Any, required: bool) -> Any:
        state = self._state
        spec = capture_spec(ty)
        tag = capture_tag(ty)
        state.check_capture(spec)
        name = state.definition_name(name)
        state.declare(name, Capture(spec, Shape(tag, self._depth, required)))
        return self._handle(tag, name, self._depth, required)


class _Nested(_Statements[N]):
    """A scope inside at least one repeat."""

    __slots__ = ()

    def _record_id(self) -> int:
        assert self._record is not None
        return self._record

    @property
    def record(self) -> Record:
        """The record of the nearest repeat."""
        return Record(self._record_id(), self._state.id)

    def each_name(self, name: str) -> None:
        """Print this repeat's ``each`` name as ``name``, which may be ``_``."""
        state = self._state
        state.records[state.record_id(self._record_id())].each = state.spelled_name(name)

    def as_name(self, name: str) -> None:
        """Print this repeat's ``as`` name as ``name``, which may be ``_``."""
        state = self._state
        state.records[state.record_id(self._record_id())].list = state.spelled_name(name)

    def record_as(self, record: Record) -> None:
        """Bind this repeat to a record from ``forward_record``."""
        state = self._state
        if not isinstance(record, Record):
            raise TypeError(f"`record_as` takes a record, not {type(record).__name__}")
        state.check_record(record._owner)
        info = state.records[record._id] if record._id < len(state.records) else None
        if info is None or info.forward is None or info.bound:
            state.fail(
                "binding",
                "`record_as` needs a record from `forward_record` that is not bound yet",
            )
        info.bound = True
        state.records[self._record_id()].link = record._id


class Scope0(_Statements[N]):
    """A scope outside every repeat."""

    __slots__ = ()


class Scope1(_Nested[N]):
    """A scope inside one repeat."""

    __slots__ = ()


class Scope2(_Nested[N]):
    """A scope inside two repeats."""

    __slots__ = ()


class Scope3(_Nested[N]):
    """A scope inside three repeats."""

    __slots__ = ()


class ScopeN(_Nested[N]):
    """A scope inside more repeats than handles can type: its definitions
    give ``DynField``."""

    __slots__ = ()


_DEPTHS: tuple[type[_Statements[Any]], ...] = (Scope0, Scope1, Scope2, Scope3)


def _scope(state: State, block: int, record: int | None, depth: int, required: bool) -> Scope[Any]:
    kind = _DEPTHS[depth] if depth <= STATIC_DEPTHS else ScopeN
    return kind(state, block, record, depth, required)


class Block(Generic[_S]):
    """The guard of a block statement: ``with`` gives the block's scope."""

    __slots__ = ("_inner", "_opener", "_parent", "_site")

    def __init__(self, parent: Scope[Any], opener: int, inner: Callable[[int], _S]) -> None:
        self._parent = parent
        self._opener = opener
        self._inner = inner
        self._site: CallSite | None = None

    def __enter__(self) -> _S:
        state = self._parent._state
        self._site = caller()
        state.reopen(self._opener + 1)
        return self._inner(state.open_block())

    def __exit__(self, *exception: object) -> None:
        assert self._site is not None
        self._parent._state.close_block(self._parent._block, self._opener, self._site)


class SectionsGuard(Block["Arms[_R, _O]"], Generic[_R, _O]):
    """The guard of ``sections``: clauses chain, and ``with`` gives the arms.
    ``_R`` is the scope of a required arm and ``_O`` of an optional one."""

    __slots__ = ()

    def __init__(self, parent: Scope[Any], opener: int) -> None:
        super().__init__(
            parent,
            opener,
            lambda block: Arms(
                parent._state, block, parent._record, parent._depth, parent._required
            ),
        )

    def _clause(self, clause: SectionClause) -> SectionsGuard[_R, _O]:
        state = self._parent._state
        state.check_last(self._opener + 1)
        stmt = state.stmts[self._opener]
        assert isinstance(stmt, Sections)
        stmt.clauses.append(clause)
        return self

    def until(self, *terminators: str, store: Any = None) -> SectionsGuard[_R, _O]:
        """``until "text"...``, with ``store x`` keeping the terminator; to read
        it, pass a handle from ``forward``."""
        _strings(terminators, "terminators")
        state = self._parent._state
        state.check_last(self._opener + 1)
        definition = None if store is None else _store_definition(state, store)
        return self._clause(Until(terminators, definition))

    def skip_other(self) -> SectionsGuard[_R, _O]:
        """``skip other``."""
        return self._clause(SkipOther())


class Arms(Generic[_R, _O]):
    """The block of a ``sections`` statement, which holds only arms: ``_R`` is
    the scope of a required arm and ``_O`` of an optional one."""

    __slots__ = ("_block", "_depth", "_record", "_required", "_state")

    def __init__(
        self, state: State, block: int, record: int | None, depth: int, required: bool
    ) -> None:
        self._state = state
        self._block = block
        self._record = record
        self._depth = depth
        self._required = required

    def arm(self, pattern: Any, *, optional: bool = False, last: bool = False) -> ArmGuard[Any]:
        """An arm with this pattern; ``with`` gives it a block. What an optional
        arm defines may be absent, and sections end after a ``last`` arm."""
        state = self._state
        index = state.push(self._block, Arm(pattern_pieces(pattern), optional, last))
        required = self._required and not optional
        return ArmGuard(self, index, required)


class ArmGuard(Generic[_S]):
    """The guard of an arm: ``with`` gives the arm's block."""

    __slots__ = ("_arms", "_index", "_required", "_site")

    def __init__(self, arms: Arms[Any, Any], index: int, required: bool) -> None:
        self._arms = arms
        self._index = index
        self._required = required
        self._site: CallSite | None = None

    def __enter__(self) -> _S:
        arms = self._arms
        state = arms._state
        self._site = caller()
        state.check_last(self._index)
        stmt = state.stmts[self._index]
        assert isinstance(stmt, Arm)
        stmt.body = True
        block = state.open_block()
        return cast(_S, _scope(state, block, arms._record, arms._depth, self._required))

    def __exit__(self, *exception: object) -> None:
        assert self._site is not None
        self._arms._state.close_block(self._arms._block, self._index, self._site)


class SpecBuilder(Scope0[Any]):
    """Builds a reader spec. It is the root scope, with the settings and
    statements that only the top level allows."""

    __slots__ = ()

    def __init__(self, format: str | None = None) -> None:
        super().__init__(State(), ROOT_BLOCK, None, 0, True)
        if format is not None:
            self.format(format)

    def format(self, name: str) -> None:
        """``format name``, a name of this format."""
        if not isinstance(name, str):
            raise TypeError("a format name is a str")
        self._push(Format(name))

    def comment(self, *markers: str) -> None:
        """``comment "text"...``."""
        self._push(Strings("comment", _strings(markers, "comment markers")))

    def stop(self, *markers: str) -> None:
        """``stop "text"...``."""
        self._push(Strings("stop", _strings(markers, "stop markers")))

    def tokens(self, characters: str) -> None:
        """``tokens "characters"``."""
        self._push(Strings("tokens", _strings((characters,), "token characters")))

    def separators(self, characters: str) -> None:
        """``separators "characters"``."""
        self._push(Strings("separators", _strings((characters,), "separator characters")))

    def ignore_trailing(self) -> None:
        """``ignore trailing``."""
        self._push(IgnoreTrailing())

    def instances(self) -> Block[Any]:
        """``instances { ... }``."""
        return self._guard(Instances(), None, 0, True)

    def source(self) -> str:
        """The text this builder emits, without parsing it."""
        return self._printed().text

    def _printed(self) -> Printed:
        state = self._state
        if state.issue is not None:
            raise state.issue
        check_bindings(state)
        return print_spec(state)

    def build(self) -> Spec:
        """Print the text and parse it. The spec is an ordinary one, whose
        errors while reading name the call site of their statement."""
        text, sites = self._printed()
        try:
            return Spec._parse_built(text, ORIGIN, sites)
        except Error as error:
            raise BuildError.from_error(error, text) from error


def _target_name(state: State, target: object) -> str | None:
    """Resolve a definition's target: ``None`` for the discard."""
    if isinstance(target, Discard):
        return None
    if isinstance(target, str):
        return state.define_new(target)
    if isinstance(target, Field):
        state.check_owner(target._owner, "the handle", target._name)
        return target._name
    raise TypeError(
        f"a target is a new name, a handle or ib.discard(), not {type(target).__name__}"
    )


def _store_definition(state: State, target: object) -> Definition:
    """The definition a terminator's ``store`` makes."""
    if isinstance(target, Discard):
        state.fail(
            "unprintable",
            "`store _` keeps an entry named `_`, so `ib.discard()` cannot spell it: "
            'pass a handle from `forward("_", ib.StrField[str])`',
        )
    return Definition(_target_name(state, target), "str", 0)


_KINDS: dict[object, ValueTag] = {
    IntField: "int",
    FloatField: "float",
    NumField: "num",
    StrField: "str",
}
_LEAVES: dict[frozenset[object], ValueTag] = {
    frozenset({int}): "int",
    frozenset({float}): "float",
    frozenset({int, float}): "num",
    frozenset({str}): "str",
}
_OPEN = frozenset(typing.get_args(Value))
_UNIONS = (typing.Union, types.UnionType)


def _members(view: object) -> set[object]:
    return set(typing.get_args(view)) if typing.get_origin(view) in _UNIONS else {view}


def handle_kind(kind: object, taker: str) -> tuple[type[Field[Any]], Shape]:
    """The handle class and shape that a handle type such as
    ``ib.IntField[list[int] | None]`` promises; ``taker`` names the function
    that takes it, for the error."""
    if kind is DynField:
        return DynField, Shape("dyn", None, False)
    origin = typing.get_origin(kind)
    arguments = typing.get_args(kind)
    if origin not in _KINDS or len(arguments) != 1:
        raise TypeError(
            f"{taker} takes a handle type with its view, such as ib.IntField[int] or "
            f"ib.StrField[list[str] | None], or ib.DynField, not {kind!r}"
        )
    tag = _KINDS[origin]
    members = _members(arguments[0])
    required = type(None) not in members
    members.discard(type(None))
    levels: int | None = 0
    while levels is not None and len(members) == 1:
        (member,) = members
        if typing.get_origin(member) is not list:
            break
        members = _members(typing.get_args(member)[0])
        levels += 1
    if members == _OPEN:
        levels = None
    elif _LEAVES.get(frozenset(members)) != tag:
        raise TypeError(f"the view of {kind!r} does not match its value type {describe(tag)}")
    return cast("type[Field[Any]]", origin), Shape(tag, levels, required)


def declared(name: str, kind: type[Any]) -> Any:
    """A handle for the entry ``name`` of a text spec, of the handle type
    ``kind``, such as ``ib.IntField[list[int]]`` or ``ib.DynField``.

    ``iris gen --lang python SPEC`` prints one per entry. The handle reads data
    as a builder's handle does, but no builder accepts it, since it comes from
    none.
    """
    handle_type, shape = handle_kind(kind, "declared")
    return handle_type(name, NO_BUILDER, shape.levels, shape.required)
