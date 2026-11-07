"""Handles, expressions, records and the functions that combine them."""

from __future__ import annotations

from collections.abc import Callable
from typing import Any, ClassVar, Generic, NoReturn, TypeVar, cast

from gecode_iris.build._ops import (
    INDEX,
    BinaryOp,
    Binding,
    BindingOp,
    CallOp,
    ComprehensionOp,
    FloatOp,
    HostOp,
    IntOp,
    ListMemberOp,
    MemberOp,
    NameOp,
    Node,
    RecordListOp,
    RecordOp,
    StrOp,
    UnaryOp,
    ValueTag,
)

_T = TypeVar("_T")
_E = TypeVar("_E", bound="Expr")

INT_MIN = -(2**63)
INT_MAX = 2**63 - 1
_INTEGRAL = frozenset({"int", "dyn"})
_NUMERIC = frozenset({"int", "float", "num", "dyn"})

_BOOL_HINT = (
    "an Iris expression has no truth value in Python: use `&`, `|` and `~` for "
    "`&&`, `||` and `!`, write each comparison in parentheses instead of chaining "
    "them, and use `if_` for a conditional block"
)
_FORMAT_HINT = (
    "an Iris expression cannot be formatted into a string: write a pattern as a "
    "t-string on Python 3.14 or later, or as a tuple of text and parts"
)


def _hint(operator: str, function: str) -> NoReturn:
    raise TypeError(f"`{operator}` is not Iris arithmetic here: use `ib.{function}(a, b)`")


class Expr:
    """An Iris expression. Operators build larger expressions; ``bool()`` is an
    error, so that a comparison is never taken for a Python condition."""

    __slots__ = ("_node",)
    _tag: ClassVar[ValueTag] = "dyn"
    _open: ClassVar[bool] = False
    """Whether the expression looks up a stored value, which may be an array."""

    def __init__(self, node: Node) -> None:
        self._node = node

    def dynamic(self) -> DynExpr:
        """This expression with no static type, accepted anywhere."""
        return cast(DynExpr, _SCALARS["dyn"](self._node))

    __hash__ = object.__hash__

    def __bool__(self) -> NoReturn:
        raise TypeError(_BOOL_HINT)

    def __format__(self, spec: str) -> NoReturn:
        raise TypeError(_FORMAT_HINT)

    def __iter__(self) -> NoReturn:
        raise TypeError("an Iris expression is not iterable: use a comprehension such as ib.sum")

    def __repr__(self) -> str:
        return f"<{type(self).__name__}>"

    def __add__(self, other: object) -> Any:
        return _binary(self, other, "+", _INTEGRAL)

    def __radd__(self, other: object) -> Any:
        return _binary(other, self, "+", _INTEGRAL)

    def __sub__(self, other: object) -> Any:
        return _binary(self, other, "-", _INTEGRAL)

    def __rsub__(self, other: object) -> Any:
        return _binary(other, self, "-", _INTEGRAL)

    def __mul__(self, other: object) -> Any:
        return _binary(self, other, "*", _INTEGRAL)

    def __rmul__(self, other: object) -> Any:
        return _binary(other, self, "*", _INTEGRAL)

    def __and__(self, other: object) -> Any:
        return _binary(self, other, "&&", _INTEGRAL)

    def __rand__(self, other: object) -> Any:
        return _binary(other, self, "&&", _INTEGRAL)

    def __or__(self, other: object) -> Any:
        return _binary(self, other, "||", _INTEGRAL)

    def __ror__(self, other: object) -> Any:
        return _binary(other, self, "||", _INTEGRAL)

    def __lt__(self, other: object) -> Any:
        return _binary(self, other, "<", _NUMERIC)

    def __le__(self, other: object) -> Any:
        return _binary(self, other, "<=", _NUMERIC)

    def __gt__(self, other: object) -> Any:
        return _binary(self, other, ">", _NUMERIC)

    def __ge__(self, other: object) -> Any:
        return _binary(self, other, ">=", _NUMERIC)

    def __eq__(self, other: object) -> Any:
        return _equality(self, other, "==")

    def __ne__(self, other: object) -> Any:
        return _equality(self, other, "!=")

    def __neg__(self) -> Any:
        if self._tag not in _NUMERIC:
            raise TypeError(f"unary `-` takes a number, not a {_describe(self)}")
        return _SCALARS[self._tag](Node(UnaryOp("-"), (self._node,)))

    def __invert__(self) -> IntExpr:
        if self._tag not in _INTEGRAL:
            raise TypeError(f"`~` (Iris `!`) takes an int, not a {_describe(self)}")
        return _int(Node(UnaryOp("!"), (self._node,)))

    def __floordiv__(self, other: object) -> NoReturn:
        _hint("//", "div")

    def __rfloordiv__(self, other: object) -> NoReturn:
        _hint("//", "div")

    def __truediv__(self, other: object) -> NoReturn:
        _hint("/", "div")

    def __rtruediv__(self, other: object) -> NoReturn:
        _hint("/", "div")

    def __mod__(self, other: object) -> NoReturn:
        _hint("%", "rem")

    def __rmod__(self, other: object) -> NoReturn:
        _hint("%", "rem")

    def __pow__(self, other: object) -> NoReturn:
        _hint("**", "pow")

    def __rpow__(self, other: object) -> NoReturn:
        _hint("**", "pow")

    def _iris_item(self, node: Node, /) -> Any:
        """What a comprehension over this expression gives its body."""
        return _LOOKUPS[self._tag](node)


class _Indexable(Expr):
    """An expression that can be indexed and have its fields looked up."""

    __slots__ = ()

    def __getitem__(self, index: object) -> Any:
        return _LOOKUPS[self._tag](Node(INDEX, (self._node, _index(index)._node)))

    def field(self, handle: Field[Any]) -> Any:
        """``self.name``, the field ``handle`` of the record this selects."""
        return _member(self._node, handle)


class IntExpr(Expr):
    """An integer expression."""

    __slots__ = ()
    _tag = "int"


class IntLookup(_Indexable, IntExpr):
    """An integer lookup, such as an index or a field: it may be an array."""

    __slots__ = ()
    _open = True


class FloatExpr(Expr):
    """A float expression."""

    __slots__ = ()
    _tag = "float"


class FloatLookup(_Indexable, FloatExpr):
    """A float lookup, which may be an array."""

    __slots__ = ()
    _open = True


class NumExpr(Expr):
    """A ``num`` expression: an int or a float, decided by the data."""

    __slots__ = ()
    _tag = "num"


class NumLookup(_Indexable, NumExpr):
    """A ``num`` lookup, which may be an array."""

    __slots__ = ()
    _open = True


class StrExpr(Expr):
    """A string expression."""

    __slots__ = ()
    _tag = "str"


class StrLookup(_Indexable, StrExpr):
    """A string lookup, which may be an array."""

    __slots__ = ()
    _open = True


class DynExpr(_Indexable):
    """An expression with no static type, which every operator accepts."""

    __slots__ = ()
    _tag = "dyn"
    _open = True


class Field(_Indexable, Generic[_T]):
    """A typed handle: the name of an entry. ``data[handle]`` reads its value.

    In the builder a handle is an expression; after reading it is a key.
    """

    __slots__ = ("_levels", "_name", "_owner", "_required")

    def __init__(self, name: str, owner: int, levels: int | None, required: bool) -> None:
        super().__init__(Node(NameOp(name, owner)))
        self._name = name
        self._owner = owner
        self._levels = levels
        self._required = required

    @property
    def name(self) -> str:
        """The entry name."""
        return self._name

    def __repr__(self) -> str:
        return f"<{type(self).__name__} {self._name}>"

    def _iris_key(self) -> tuple[str, str | None, int | None, bool]:
        """What the handle promises about its entry, for ``Data`` access."""
        value_type = None if self._tag == "dyn" else _VALUE_TYPES[self._tag]
        return (self._name, value_type, self._levels, self._required)


_VALUE_TYPES = {"int": "int", "float": "float", "num": "num", "str": "string"}


class IntField(IntLookup, Field[_T]):
    """A handle of an integer entry."""

    __slots__ = ()


class FloatField(FloatLookup, Field[_T]):
    """A handle of a float entry."""

    __slots__ = ()


class NumField(NumLookup, Field[_T]):
    """A handle of a ``num`` entry."""

    __slots__ = ()


class StrField(StrLookup, Field[_T]):
    """A handle of a string entry."""

    __slots__ = ()


class DynField(DynExpr, Field[Any]):
    """A handle whose value type or depth is open."""

    __slots__ = ()


def _leaf(base: type[_E]) -> type[_E]:
    """A subclass of ``base`` that no handle class inherits.

    Python tries the reflected comparison first when the right operand's type
    is a subclass of the left one's, so ``size(r) == machines`` would print as
    ``machines == size(r)`` if expressions were instances of the classes that
    handles extend. Built expressions are instances of these leaves instead.
    """
    namespace = {"__slots__": (), "__module__": base.__module__, "__qualname__": base.__qualname__}
    return cast("type[_E]", type(base.__name__, (base,), namespace))


_SCALARS: dict[ValueTag, type[Expr]] = {
    "int": _leaf(IntExpr),
    "float": _leaf(FloatExpr),
    "num": _leaf(NumExpr),
    "str": _leaf(StrExpr),
    "dyn": _leaf(DynExpr),
}
_LOOKUPS: dict[ValueTag, type[Expr]] = {
    "int": _leaf(IntLookup),
    "float": _leaf(FloatLookup),
    "num": _leaf(NumLookup),
    "str": _leaf(StrLookup),
    "dyn": _SCALARS["dyn"],
}


def _int(node: Node) -> IntExpr:
    return cast(IntExpr, _SCALARS["int"](node))


FIELDS: dict[ValueTag, type[Field[Any]]] = {
    "int": IntField,
    "float": FloatField,
    "num": NumField,
    "str": StrField,
    "dyn": DynField,
}


def field_tag(kind: type[Field[Any]]) -> ValueTag:
    return kind._tag


def _describe(expr: Expr) -> str:
    return {"str": "string", "dyn": "dynamic value"}.get(expr._tag, expr._tag)


def lit(value: float | str) -> Any:
    """An Iris literal: an ``int`` in the 64-bit range, a ``float`` or a ``str``."""
    if isinstance(value, bool):
        raise TypeError("Iris has no bool literal: use 1 and 0")
    if isinstance(value, int):
        if not INT_MIN <= value <= INT_MAX:
            raise OverflowError(f"the integer {value} is outside the 64-bit range of Iris")
        return _int(Node(IntOp(value)))
    if isinstance(value, float):
        return _SCALARS["float"](Node(FloatOp(value)))
    if isinstance(value, str):
        return _SCALARS["str"](Node(StrOp(value)))
    raise TypeError(f"an Iris literal is an int, float or str, not {type(value).__name__}")


def operand(value: object) -> Expr:
    """An expression, or a Python literal as one."""
    if isinstance(value, Expr):
        return value
    if isinstance(value, (int, float, str)):
        return lit(value)
    raise TypeError(
        f"an Iris operand is an expression, handle, int, float or str, not {type(value).__name__}"
    )


def _optional_operand(value: object) -> Expr | None:
    if isinstance(value, bool) or not isinstance(value, (Expr, int, float, str)):
        return None
    return operand(value)


def _binary(left: object, right: object, symbol: str, accepted: frozenset[str]) -> Any:
    left_expr, right_expr = _optional_operand(left), _optional_operand(right)
    if left_expr is None or right_expr is None:
        return NotImplemented
    if left_expr._tag not in accepted or right_expr._tag not in accepted:
        return NotImplemented
    return _int(Node(BinaryOp(symbol), (left_expr._node, right_expr._node)))


def _equatable(left: ValueTag, right: ValueTag) -> bool:
    if "dyn" in (left, right):
        return True
    return (left == "str") == (right == "str")


def _equality(left: Expr, right: object, symbol: str) -> Any:
    right_expr = _optional_operand(right)
    if right_expr is None or not _equatable(left._tag, right_expr._tag):
        return NotImplemented
    return _int(Node(BinaryOp(symbol), (left._node, right_expr._node)))


def _integral(value: object, role: str) -> Expr:
    expr = operand(value)
    if expr._tag not in _INTEGRAL:
        raise TypeError(f"{role} must be an int, not a {_describe(expr)}")
    return expr


def _index(value: object) -> Expr:
    return _integral(value, "an index")


def _arithmetic(symbol: str, left: object, right: object) -> IntExpr:
    left_expr = _integral(left, f"the left operand of `{symbol}`")
    right_expr = _integral(right, f"the right operand of `{symbol}`")
    return _int(Node(BinaryOp(symbol), (left_expr._node, right_expr._node)))


def div(left: object, right: object) -> IntExpr:
    """Iris ``left / right``, integer division rounding toward zero."""
    return _arithmetic("/", left, right)


def rem(left: object, right: object) -> IntExpr:
    """Iris ``left % right``, the remainder with the sign of ``left``."""
    return _arithmetic("%", left, right)


def pow(base: object, exponent: object) -> IntExpr:
    """Iris ``base ^ exponent``."""
    return _arithmetic("^", base, exponent)


def _member(base: Node, handle: object) -> Any:
    if not isinstance(handle, Field):
        raise TypeError(f"a field is a handle, not {type(handle).__name__}")
    return _LOOKUPS[handle._tag](Node(MemberOp(handle._name, handle._owner), (base,)))


class Record:
    """The records of a repeat. ``record[field]`` is the field of the current
    record, ``record.list`` all the records, and a comprehension over a record
    gives its body one record at a time."""

    __slots__ = ("_id", "_owner")

    def __init__(self, record: int, owner: int) -> None:
        self._id = record
        self._owner = owner

    def __getitem__(self, handle: Field[Any]) -> Any:
        return _member(Node(RecordOp(self._id, self._owner)), handle)

    @property
    def list(self) -> RecordList:
        """All the records of this repeat, printed as its ``as`` name."""
        return RecordList(Node(RecordListOp(self._id, self._owner)))

    def records(self, inner: Record) -> RecordList:
        """The records of the nested repeat ``inner`` in the current record."""
        return RecordList(
            Node(ListMemberOp(inner._id, inner._owner), (Node(RecordOp(self._id, self._owner)),))
        )

    def _iris_item(self, node: Node, /) -> RecordExpr:
        return RecordExpr(node)

    def __repr__(self) -> str:
        return "<Record>"


class RecordList:
    """A list of records: all the records of a repeat, or a nested list.
    ``records[i]`` selects one."""

    __slots__ = ("_node",)

    def __init__(self, node: Node) -> None:
        self._node = node

    def __getitem__(self, index: object) -> RecordExpr:
        return RecordExpr(Node(INDEX, (self._node, _index(index)._node)))

    def _iris_item(self, node: Node, /) -> RecordExpr:
        return RecordExpr(node)


class RecordExpr:
    """One record: selected by index, or a comprehension's variable.
    ``record[field]`` looks up one of its fields."""

    __slots__ = ("_node",)

    def __init__(self, node: Node) -> None:
        self._node = node

    def __getitem__(self, handle: Field[Any]) -> Any:
        return _member(self._node, handle)

    def at(self, index: object) -> RecordExpr:
        """``self[index]``, for a record that is itself a list."""
        return RecordExpr(Node(INDEX, (self._node, _index(index)._node)))

    def records(self, inner: Record) -> RecordList:
        """The records of the nested repeat ``inner`` in this record."""
        return RecordList(Node(ListMemberOp(inner._id, inner._owner), (self._node,)))


class _Over:
    """A domain whose comprehension variable has a chosen name."""

    __slots__ = ("_domain", "_name")

    def __init__(self, domain: Any, name: str | None) -> None:
        self._domain = domain
        self._name = name

    def _iris_item(self, node: Node, /) -> Any:
        return self._domain._iris_item(node)


def over(domain: Any, name: str | None = None) -> Any:
    """``domain``, with its comprehension variable printed as ``name``."""
    _domain_node(domain)
    return _Over(domain, name)


def _domain_node(domain: object) -> Node:
    if isinstance(domain, _Over):
        return _domain_node(domain._domain)
    if isinstance(domain, Record):
        return domain.list._node
    if isinstance(domain, (RecordList, Expr)):
        return domain._node
    raise TypeError(
        f"a domain is an expression, a handle, a record or a record list, "
        f"not {type(domain).__name__}"
    )


def _comprehension(function: str, domain: Any, body: Callable[[Any], object]) -> IntExpr:
    node = _domain_node(domain)
    name = domain._name if isinstance(domain, _Over) else None
    binding = Binding()
    result = operand(body(domain._iris_item(Node(BindingOp(binding)))))
    return _int(Node(ComprehensionOp(function, binding, name), (node, result._node)))


def _call(function: str, arguments: tuple[object, ...]) -> IntExpr:
    nodes = tuple(operand(argument)._node for argument in arguments)
    return _int(Node(CallOp(function, len(nodes)), nodes))


def _is_body(value: object) -> bool:
    return callable(value) and not isinstance(value, (Expr, Record, RecordList, RecordExpr))


def all(domain: Any, body: Callable[[Any], object]) -> IntExpr:
    """``all(x in domain: body)``."""
    return _comprehension("all", domain, body)


def any(domain: Any, body: Callable[[Any], object]) -> IntExpr:
    """``any(x in domain: body)``."""
    return _comprehension("any", domain, body)


def sum(*arguments: Any) -> IntExpr:
    """``sum(a, b, ...)``, or ``sum(x in domain: body)`` given a domain and a
    function of its variable."""
    if len(arguments) == 2 and _is_body(arguments[1]):
        return _comprehension("sum", arguments[0], arguments[1])
    return _call("sum", arguments)


def count(domain: Any, body: Callable[[Any], object] | None = None) -> IntExpr:
    """``count(domain)``, or ``count(x in domain: body)`` given a function."""
    if body is not None:
        return _comprehension("count", domain, body)
    return _int(Node(CallOp("count", 1), (_domain_node(domain),)))


def size(domain: Any) -> IntExpr:
    """``size(domain)``."""
    return _int(Node(CallOp("size", 1), (_domain_node(domain),)))


def min(*arguments: object) -> IntExpr:
    """``min(a, b, ...)``."""
    return _call("min", arguments)


def max(*arguments: object) -> IntExpr:
    """``max(a, b, ...)``."""
    return _call("max", arguments)


def distinct(*arguments: object) -> IntExpr:
    """``distinct(a, b, ...)``."""
    return _call("distinct", arguments)


def defined(name: object) -> IntExpr:
    """``defined(name)``: a handle, record, record list or comprehension
    variable. ``defined(record)`` tests the record list."""
    if isinstance(name, Record):
        node = name.list._node
    elif isinstance(name, (Expr, RecordList, RecordExpr)):
        node = name._node
    else:
        raise TypeError(f"`defined` takes a handle or a record, not {type(name).__name__}")
    return _int(Node(CallOp("defined", 1), (node,)))


def host(name: str) -> IntExpr:
    return _int(Node(HostOp(name)))
