"""Expression operations and the tree that holds them.

An expression is a tree of nodes, so that composing two expressions takes
constant time. A statement flattens each of its expressions once into
postfix operations, in which every operation pops its operands' results and
pushes one, so that a chain of any length checks and prints without
recursion.
"""

from __future__ import annotations

from typing import Literal, NamedTuple, TypeAlias

ValueTag: TypeAlias = Literal["int", "float", "str", "num", "dyn"]
"""A value type as data: ``dyn`` is a value with no static type."""

_DESCRIPTIONS: dict[ValueTag, str] = {
    "int": "int",
    "float": "float",
    "str": "string",
    "num": "num",
    "dyn": "dynamic",
}


def describe(tag: ValueTag) -> str:
    return _DESCRIPTIONS[tag]


class Binding:
    """A comprehension variable. Printed names never depend on its identity;
    they are numbered by first appearance in the text."""

    __slots__ = ()


class IntOp(NamedTuple):
    value: int


class FloatOp(NamedTuple):
    value: float


class StrOp(NamedTuple):
    text: str


class HostOp(NamedTuple):
    """A host constant such as ``gecode.int_max``."""

    name: str


class NameOp(NamedTuple):
    text: str
    owner: int


class BindingOp(NamedTuple):
    binding: Binding


class RecordOp(NamedTuple):
    """The ``each`` name of a repeat."""

    record: int
    owner: int


class RecordListOp(NamedTuple):
    """The ``as`` name of a repeat."""

    record: int
    owner: int


class UnaryOp(NamedTuple):
    symbol: Literal["-", "!"]


class BinaryOp(NamedTuple):
    symbol: str


class IndexOp(NamedTuple):
    pass


class MemberOp(NamedTuple):
    """``.name`` on the preceding operand."""

    text: str
    owner: int


class ListMemberOp(NamedTuple):
    """``.records`` on the preceding operand, with a repeat's ``as`` name."""

    record: int
    owner: int


class CallOp(NamedTuple):
    function: str
    arguments: int


class ComprehensionOp(NamedTuple):
    function: str
    binding: Binding
    name: str | None


Op: TypeAlias = (
    IntOp
    | FloatOp
    | StrOp
    | HostOp
    | NameOp
    | BindingOp
    | RecordOp
    | RecordListOp
    | UnaryOp
    | BinaryOp
    | IndexOp
    | MemberOp
    | ListMemberOp
    | CallOp
    | ComprehensionOp
)
Ops: TypeAlias = "list[Op]"
"""One expression as postfix operations."""

INDEX = IndexOp()


_OPERAND_COUNTS: dict[type, int] = {
    UnaryOp: 1,
    MemberOp: 1,
    ListMemberOp: 1,
    BinaryOp: 2,
    IndexOp: 2,
    ComprehensionOp: 2,
}


def operand_count(op: Op) -> int:
    """How many operands an operation pops."""
    if type(op) is CallOp:
        return op.arguments
    return _OPERAND_COUNTS.get(type(op), 0)


class Node(NamedTuple):
    op: Op
    children: tuple[Node, ...] = ()


def postfix(node: Node) -> list[Op]:
    """The operations of a tree in postfix order."""
    ops: list[Op] = []
    pending: list[tuple[Node, bool]] = [(node, False)]
    while pending:
        current, expanded = pending.pop()
        if expanded or not current.children:
            ops.append(current.op)
            continue
        pending.append((current, True))
        pending.extend((child, False) for child in reversed(current.children))
    return ops
