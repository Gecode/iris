"""The statement stream: one flat list in text order, with blocks marked by
openers and ``Close``, so that every pass is a loop over it."""

from __future__ import annotations

from collections.abc import Iterator
from dataclasses import dataclass, field
from itertools import count
from typing import Literal, NamedTuple, NoReturn, TypeAlias

from gecode_iris._error import CallSite
from gecode_iris.build._error import BuildError, BuildErrorKind
from gecode_iris.build._ops import (
    ListMemberOp,
    MemberOp,
    NameOp,
    Ops,
    RecordListOp,
    RecordOp,
    ValueTag,
)
from gecode_iris.build._pattern import (
    BindPiece,
    CapturePiece,
    DiscardAsPiece,
    Piece,
    ReferencePiece,
    ValidatePiece,
)
from gecode_iris.build._site import caller
from gecode_iris.build._types import Declared, NamedSpec, RangeSpec

NO_BUILDER = 0
"""The owner of a handle that no builder made, such as a declared one."""

_owners = count(NO_BUILDER + 1)


def next_owner() -> int:
    """A new builder id. It only compares handles; it is never printed."""
    return next(_owners)


class Definition(NamedTuple):
    """What a definition defines: ``name`` is ``None`` for the discard ``_``.
    ``levels`` counts the definition's own array levels, or is ``None`` when
    they are open."""

    name: str | None
    value: ValueTag
    levels: int | None


@dataclass(slots=True)
class Times:
    count: Ops


@dataclass(slots=True)
class UntilEof:
    pass


@dataclass(slots=True)
class Until:
    terminators: tuple[str, ...]
    store: Definition | None = None


RepeatCount: TypeAlias = "Times | UntilEof | Until"


@dataclass(slots=True)
class SkipOther:
    pass


SectionClause: TypeAlias = "Until | SkipOther"


@dataclass(slots=True)
class Format:
    name: str


@dataclass(slots=True)
class Strings:
    """``comment``, ``stop``, ``tokens``, ``separators`` or ``expect`` with
    its strings."""

    keyword: Literal["comment", "stop", "tokens", "separators", "expect"]
    strings: tuple[str, ...]


@dataclass(slots=True)
class IgnoreTrailing:
    pass


@dataclass(slots=True)
class Param:
    definition: Definition
    default: Ops | None


@dataclass(slots=True)
class Declaration:
    definition: Definition
    declared: Declared


@dataclass(slots=True)
class ListStmt:
    definition: Definition
    declared: Declared
    end: Ops | None
    """The sentinel, or ``None`` for ``until eol``."""


@dataclass(slots=True)
class TypeDefinition:
    name: str
    declared: Declared


@dataclass(slots=True)
class Line:
    keyword: Literal["line", "find"]
    pieces: list[Piece]


@dataclass(slots=True)
class SkipLine:
    pass


@dataclass(slots=True)
class SkipLines:
    count: Ops


@dataclass(slots=True)
class Index:
    position: Ops


@dataclass(slots=True)
class Let:
    definition: Definition
    value: Ops


@dataclass(slots=True)
class Check:
    condition: Ops
    message: str | None


@dataclass(slots=True)
class Repeat:
    count: RepeatCount
    record: int


@dataclass(slots=True)
class Sections:
    clauses: list[SectionClause]


@dataclass(slots=True)
class Arm:
    pieces: list[Piece]
    optional: bool = False
    last: bool = False
    body: bool = False


@dataclass(slots=True)
class If:
    condition: Ops


@dataclass(slots=True)
class ElseIf:
    condition: Ops


@dataclass(slots=True)
class Else:
    pass


@dataclass(slots=True)
class ElseDeclaration:
    """``else if:type``, a declaration named ``if`` as the else branch."""

    definition: Definition
    declared: Declared


@dataclass(slots=True)
class Instances:
    pass


@dataclass(slots=True)
class Close:
    pass


Stmt: TypeAlias = (
    Format
    | Strings
    | IgnoreTrailing
    | Param
    | Declaration
    | ListStmt
    | TypeDefinition
    | Line
    | SkipLine
    | SkipLines
    | Index
    | Let
    | Check
    | Repeat
    | Sections
    | Arm
    | If
    | ElseIf
    | Else
    | ElseDeclaration
    | Instances
    | Close
)


class NameMention(NamedTuple):
    text: str
    owner: int


Mention: TypeAlias = "Ops | NameMention | NamedSpec"
"""Something a statement mentions: an expression, a handle in a pattern, or
a named type."""


def declared_mentions(declared: Declared) -> Iterator[Mention]:
    yield from declared.dimensions
    ty = declared.ty
    if isinstance(ty, NamedSpec):
        yield ty
    elif isinstance(ty, RangeSpec):
        yield from (bound for bound in (ty.lower, ty.upper) if bound is not None)
    yield from (option for option in (declared.layout, declared.offset) if option is not None)


def _capture_declared(piece: Piece) -> Declared | None:
    if isinstance(piece, (ValidatePiece, DiscardAsPiece)):
        return piece.capture
    if isinstance(piece, BindPiece):
        return piece.capture
    return None


def pattern_mentions(pieces: list[Piece]) -> Iterator[Mention]:
    for piece in pieces:
        if isinstance(piece, (CapturePiece, BindPiece, ReferencePiece)):
            yield NameMention(piece.name, piece.owner)
        declared = _capture_declared(piece)
        if declared is not None:
            yield from declared_mentions(declared)


def mentions(stmt: Stmt) -> Iterator[Mention]:
    """Every expression, handle and named type a statement mentions."""
    if isinstance(stmt, Param):
        if stmt.default is not None:
            yield stmt.default
    elif isinstance(stmt, (Declaration, TypeDefinition, ElseDeclaration)):
        yield from declared_mentions(stmt.declared)
    elif isinstance(stmt, ListStmt):
        yield from declared_mentions(stmt.declared)
        if stmt.end is not None:
            yield stmt.end
    elif isinstance(stmt, (Line, Arm)):
        yield from pattern_mentions(stmt.pieces)
    elif isinstance(stmt, (If, ElseIf, Check)):
        yield stmt.condition
    elif isinstance(stmt, Let):
        yield stmt.value
    elif isinstance(stmt, SkipLines):
        yield stmt.count
    elif isinstance(stmt, Index):
        yield stmt.position
    elif isinstance(stmt, Repeat) and isinstance(stmt.count, Times):
        yield stmt.count.count


def expressions(stmt: Stmt) -> Iterator[Ops]:
    """The expressions a statement mentions."""
    return (mention for mention in mentions(stmt) if isinstance(mention, list))


def opens_block(stmt: Stmt) -> bool:
    """Whether a statement starts a block that ``Close`` ends."""
    if isinstance(stmt, Arm):
        return stmt.body
    return isinstance(stmt, (Repeat, Sections, If, ElseIf, Else, Instances))


def continues_if(stmt: Stmt) -> bool:
    """Whether a statement continues the line of the preceding ``}``."""
    return isinstance(stmt, (ElseIf, Else, ElseDeclaration))


class Shape(NamedTuple):
    """What a forward handle or capture promises about its definitions."""

    value: ValueTag
    levels: int | None
    required: bool


class Defined(NamedTuple):
    pass


class Forward(NamedTuple):
    shape: Shape


class Undefined(NamedTuple):
    """From ``undefined``; nothing may define it."""


class Capture(NamedTuple):
    spec: Declared | None
    shape: Shape


NameState: TypeAlias = Defined | Forward | Undefined | Capture


@dataclass(slots=True)
class RecordInfo:
    each: str | None = None
    list: str | None = None
    forward: CallSite | None = None
    """Where ``forward_record`` made it, awaiting ``record_as``."""
    bound: bool = False
    link: int | None = None
    """The forward record this repeat was bound to."""


class DeclaredName(NamedTuple):
    """A forward handle or capture, with the call that made it."""

    name: str
    site: CallSite


class ClosedBlock(NamedTuple):
    """The block that the last ``}`` closed, for ``else`` to continue."""

    parent: int
    opener: int
    close: int


ROOT_BLOCK = 0


def is_identifier(name: str) -> bool:
    return (
        bool(name)
        and name.isascii()
        and (name[0].isalpha() or name[0] == "_")
        and all(c.isalnum() or c == "_" for c in name)
    )


@dataclass
class State:
    """The builder's state, shared by its scopes and guards."""

    id: int = field(default_factory=next_owner)
    stmts: list[Stmt] = field(default_factory=list)
    sites: list[CallSite] = field(default_factory=list)
    """The call site of each statement. A ``}`` has the call that entered its
    block."""
    open: list[int] = field(default_factory=lambda: [ROOT_BLOCK])
    """The ids of the open blocks, innermost last."""
    next_block: int = ROOT_BLOCK + 1
    names: dict[str, NameState] = field(default_factory=dict)
    declared: list[DeclaredName] = field(default_factory=list)
    """Forward handles and captures in creation order, for reporting."""
    types: set[str] = field(default_factory=set)
    records: list[RecordInfo] = field(default_factory=list)
    closed: ClosedBlock | None = None
    issue: BuildError | None = None

    def fail(self, kind: BuildErrorKind, message: str, site: CallSite | None = None) -> NoReturn:
        """Report a failed check. The first one is kept, since the checks after
        it may only be its consequences, and ``build()`` raises it again."""
        error = BuildError(kind, message, site or caller())
        if self.issue is None:
            self.issue = error
        raise error

    def open_block(self) -> int:
        block = self.next_block
        self.next_block += 1
        self.open.append(block)
        return block

    def close_block(self, parent: int, opener: int, site: CallSite) -> None:
        self.open.pop()
        self.push_close(parent, opener, site)

    def push_close(self, parent: int, opener: int, site: CallSite) -> None:
        self.stmts.append(Close())
        self.sites.append(site)
        self.closed = ClosedBlock(parent, opener, len(self.stmts) - 1)

    def check_innermost(self, block: int) -> None:
        if self.open[-1] != block:
            self.fail(
                "scope",
                "a statement went to a scope that is not the innermost open block; "
                "use the scope of the `with` block it is in",
            )

    def is_last(self, index: int) -> bool:
        return index + 1 == len(self.stmts)

    def check_last(self, index: int) -> None:
        """Check that a guard's statement at ``index`` is still the last one."""
        if not self.is_last(index):
            self.fail(
                "scope",
                "a guard was used after later statements; "
                "finish a statement before starting the next one",
            )

    def reopen(self, index: int) -> None:
        """Remove the ``}`` at ``index``, so that a block can take its body."""
        self.check_last(index)
        self.stmts.pop()
        self.sites.pop()

    def check_owner(self, owner: int, what: str, name: str) -> None:
        if owner != self.id:
            self.fail("provenance", f"{what} `{name}` does not come from this builder")

    def check_record(self, owner: int) -> None:
        if owner != self.id:
            self.fail("provenance", "a record handle does not come from this builder")

    def check_mention(self, mention: Mention) -> None:
        if isinstance(mention, NameMention):
            self.check_owner(mention.owner, "the handle", mention.text)
        elif isinstance(mention, NamedSpec):
            self.check_owner(mention.owner, "the type", mention.name)
        else:
            for op in mention:
                if isinstance(op, (NameOp, MemberOp)):
                    self.check_owner(op.owner, "the handle", op.text)
                elif isinstance(op, (RecordOp, RecordListOp, ListMemberOp)):
                    self.check_record(op.owner)

    def check_capture(self, spec: Declared | None) -> None:
        """Check the handles of a capture's type, which a pattern prints later."""
        if spec is not None:
            for mention in declared_mentions(spec):
                self.check_mention(mention)

    def push(self, block: int, stmt: Stmt, site: CallSite | None = None) -> int:
        """Check the handles of ``stmt`` and append it to the block ``block``."""
        self.check_innermost(block)
        for mention in mentions(stmt):
            self.check_mention(mention)
        self.stmts.append(stmt)
        self.sites.append(site or caller())
        return len(self.stmts) - 1

    def record_id(self, record: int) -> int:
        """Follow a repeat's record to the forward record it was bound to."""
        link = self.records[record].link if record < len(self.records) else None
        return record if link is None else link

    def new_record(self) -> int:
        self.records.append(RecordInfo())
        return len(self.records) - 1

    def definition_name(self, name: str) -> str:
        """Check the name of a definition. A declaration, list or capture named
        ``_`` stores nothing, and ``let``, ``param`` and ``type`` reject ``_``,
        so no definition gives a handle for it."""
        if name == "_":
            self.fail(
                "unprintable",
                "`_` names nothing here: use `ib.discard()` to read without storing, "
                'or `forward("_", ib.StrField[str])` with `store` for the entry named `_`',
            )
        return self.spelled_name(name)

    def spelled_name(self, name: str) -> str:
        """Check a name that may be ``_``: a repeat's ``each`` or ``as`` name, a
        forward handle for ``store``, or a name left undefined."""
        if not isinstance(name, str):
            raise TypeError(f"a name is a str, not {type(name).__name__}")
        if name.startswith("gecode."):
            self.fail(
                "unprintable",
                f"a definition cannot be named `{name}`: the prefix `gecode.` is reserved",
            )
        if not is_identifier(name):
            self.fail("unprintable", f"`{name}` is not an Iris name")
        return name

    def define_new(self, name: str) -> str:
        name = self.definition_name(name)
        if name in self.names:
            self.fail(
                "redefinition",
                f"`{name}` is already defined; to redefine it, pass its handle "
                "as the target, or use `set`",
            )
        self.names[name] = Defined()
        return name

    def declare(self, name: str, state: NameState) -> str:
        """Declare a checked name for a forward handle, a capture, or
        ``undefined``."""
        if name in self.names:
            self.fail("redefinition", f"`{name}` is already declared")
        self.names[name] = state
        self.declared.append(DeclaredName(name, caller()))
        return name
