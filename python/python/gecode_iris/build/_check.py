"""The binding check: one walk over the statements, which knows the depth and
presence of each definition, so ``optional`` counts wherever it is given."""

from __future__ import annotations

from dataclasses import dataclass, field, replace
from typing import NamedTuple, NoReturn

from gecode_iris._error import CallSite
from gecode_iris.build._error import BuildError, BuildErrorKind
from gecode_iris.build._ops import ValueTag, describe
from gecode_iris.build._pattern import BindPiece, CapturePiece, Piece
from gecode_iris.build._state import (
    Arm,
    Capture,
    Close,
    Declaration,
    Definition,
    Else,
    ElseDeclaration,
    ElseIf,
    Forward,
    If,
    Let,
    Line,
    ListStmt,
    Param,
    Repeat,
    Sections,
    Shape,
    State,
    Stmt,
    TypeDefinition,
    Undefined,
    Until,
    declared_mentions,
    mentions,
    opens_block,
)
from gecode_iris.build._types import NamedSpec


@dataclass(frozen=True, slots=True)
class Frame:
    block: int
    """Identifies this block. Open blocks are numbered in nesting order."""
    depth: int
    """The repeats around this block."""
    required: bool


@dataclass(slots=True)
class Bound:
    count: int = 0
    required: bool = False
    block: int | None = None
    """The block of a binding, which always precedes what follows it there."""


class PendingStore(NamedTuple):
    """A repeat's ``store``, which runs when the repeat ends."""

    stmt: int
    repeat_block: int
    store: Definition
    frame: Frame


class TypeDeclaration(NamedTuple):
    """A ``type`` statement, in the block whose end hides it."""

    stmt: int
    block: int


def check_bindings(state: State) -> None:
    """Check every capture, forward handle and named type against how it was
    declared."""
    walk = _Walk(state)
    for index, stmt in enumerate(state.stmts):
        walk.current = index
        frame = walk.frames[-1]
        for mention in mentions(stmt):
            if isinstance(mention, NamedSpec):
                walk.type_use(mention)
        walk.record_definitions(index, stmt, frame)
        walk.follow_blocks(stmt, frame)
    walk.finish()


def _fail_at(site: CallSite, message: str, kind: BuildErrorKind = "binding") -> NoReturn:
    raise BuildError(kind, message, site)


def _agrees(promised: Shape, value: ValueTag, levels: int | None) -> bool:
    """Whether a definition certainly has the shape a forward handle promises.
    A value type or depth that is not known agrees only with an open promise."""
    if promised.value == "dyn":
        value_agrees = True
    elif promised.value == "num":
        value_agrees = value in ("int", "float", "num")
    else:
        value_agrees = promised.value == value
    return value_agrees and (promised.levels is None or promised.levels == levels)


def _describe(value: ValueTag, levels: int | None, open_depth: str) -> str:
    if levels is None:
        return f"{describe(value)} at {open_depth} depth"
    return f"{describe(value)} at depth {levels}"


@dataclass
class _Walk:
    state: State
    current: int = 0
    """The index of the statement being checked."""
    bound: dict[str, Bound] = field(default_factory=dict)
    frames: list[Frame] = field(default_factory=lambda: [Frame(0, 0, True)])
    next_block: int = 0
    stores: list[PendingStore] = field(default_factory=list)
    """The stores of the open repeats, innermost last."""
    types: dict[str, list[TypeDeclaration]] = field(default_factory=dict)
    """The declarations of each type name so far, latest last. Those of closed
    blocks are dropped when a use finds them."""

    def fail(self, message: str) -> NoReturn:
        """Fail the statement being checked."""
        _fail_at(self.state.sites[self.current], message)

    def is_open(self, block: int) -> bool:
        return any(frame.block == block for frame in reversed(self.frames))

    def own_block(self, frame: Frame) -> Frame:
        """``frame`` with the number of a block of its own."""
        self.next_block += 1
        return replace(frame, block=self.next_block)

    def enter(self, frame: Frame) -> int:
        frame = self.own_block(frame)
        self.frames.append(frame)
        return frame.block

    def record_binding(self, name: str, frame: Frame) -> int:
        precedes = self.binding_precedes(name)
        bound = self.bound.setdefault(name, Bound())
        bound.count += 1
        bound.required |= frame.required
        if not precedes:
            bound.block = frame.block
        return bound.count

    def binding_precedes(self, name: str) -> bool:
        """Whether a binding of ``name`` always executes before the current
        statement."""
        bound = self.bound.get(name)
        return bound is not None and bound.block is not None and self.is_open(bound.block)

    def bind(self, name: str, value: ValueTag, levels: int | None, frame: Frame) -> None:
        """A statement defines ``name`` in ``frame``, with ``levels`` in all."""
        state = self.state.names.get(name)
        if isinstance(state, Undefined):
            self.fail(
                f"`{name}` comes from `undefined`, so nothing may define it; "
                "use `forward` for a name that is defined"
            )
        if not isinstance(state, Forward):
            return
        if _agrees(state.shape, value, levels):
            self.record_binding(name, frame)
        elif not self.binding_precedes(name):
            # After a binding, this is an explicit redefinition, which leaves
            # the handle stale.
            shape = state.shape
            self.fail(
                f"the forward handle `{name}` is {_describe(shape.value, shape.levels, 'any')}, "
                f"but this definition is {_describe(value, levels, 'an unknown')}; "
                "declare it with the definition's type, or as `ib.DynField`"
            )

    def definition(self, definition: Definition, frame: Frame) -> None:
        """A statement other than ``store`` defines ``definition`` in ``frame``."""
        name = definition.name
        if name is None:
            return
        if name == "_":
            self.fail(
                "only `store` defines the entry named `_`: a declaration, a list or a "
                "capture discards it, and `let` and `param` reject it"
            )
        levels = None if definition.levels is None else frame.depth + definition.levels
        self.bind(name, definition.value, levels, frame)

    def store(self, store: Definition, frame: Frame) -> None:
        """A ``store`` defines ``store`` in ``frame``. No repeat collects ``_``
        into a column, so inside one it lasts for the iteration and leaves no
        entry."""
        name = store.name
        if name is None:
            return
        if name == "_" and frame.depth > 0:
            self.bind(name, store.value, store.levels, replace(frame, required=False))
            return
        levels = None if store.levels is None else frame.depth + store.levels
        self.bind(name, store.value, levels, frame)

    def pattern(self, pieces: list[Piece], frame: Frame) -> None:
        for piece in pieces:
            if isinstance(piece, CapturePiece):
                self.capture(piece.name, frame)
            elif isinstance(piece, BindPiece):
                self.definition(Definition(piece.name, piece.value, 0), frame)

    def capture(self, name: str, frame: Frame) -> None:
        """A pattern binds the capture ``name`` in ``frame``."""
        state = self.state.names.get(name)
        if not isinstance(state, Capture):
            self.fail(
                f"`{name}` is not a capture; use `ib.bind` to define it in a pattern, "
                "or `ib.ref` to match its value"
            )
        if self.record_binding(name, frame) > 1:
            self.fail(
                f"the capture `{name}` is bound twice; a capture belongs to one pattern, "
                "so use `ib.ref` to match its value again"
            )
        shape = state.shape
        if shape.levels is not None and shape.levels != frame.depth:
            self.fail(
                f"the capture `{name}` is bound in a different scope: create it with "
                "`capture` in the scope of the pattern"
            )
        if shape.required != frame.required:
            form = "capture" if frame.required else "optional_capture"
            self.fail(f"the capture `{name}` is bound where its presence differs: use `{form}`")
        if state.spec is not None:
            for mention in declared_mentions(state.spec):
                if isinstance(mention, NamedSpec):
                    self.type_use(mention)

    def type_use(self, named: NamedSpec) -> None:
        """A statement uses the named type ``named``, which must be the one
        visible."""
        declarations = self.types.setdefault(named.name, [])
        while declarations and not self.is_open(declarations[-1].block):
            declarations.pop()
        if not declarations or declarations[-1].stmt != named.declaration:
            _fail_at(
                self.state.sites[self.current],
                f"the declaration of the type `{named.name}` that this handle came from "
                "is not the one visible here",
                "scope",
            )

    def finish(self) -> None:
        for name, site in self.state.declared:
            bound = self.bound.get(name, Bound())
            state = self.state.names[name]
            if isinstance(state, Capture) and bound.count == 0:
                _fail_at(site, f"the capture `{name}` is never bound; add it to a pattern")
            if isinstance(state, Forward) and bound.count == 0:
                _fail_at(
                    site,
                    f"the forward handle `{name}` is never bound; define it, "
                    "or use `undefined` for a name left undefined",
                )
            if isinstance(state, Forward) and state.shape.required and not bound.required:
                _fail_at(
                    site,
                    f"the forward handle `{name}` is required, but every definition "
                    "may not execute; declare it with `| None`, as in "
                    "`ib.IntField[int | None]`",
                )
        for record in self.state.records:
            if record.forward is not None and not record.bound:
                _fail_at(
                    record.forward,
                    f"the forward record `{record.each or ''}` is never bound; "
                    "use `record_as` in its repeat",
                )

    def record_definitions(self, index: int, stmt: Stmt, frame: Frame) -> None:
        """Record what ``stmt``, at ``index``, defines in ``frame``."""
        if isinstance(stmt, (Param, Declaration, ListStmt, Let)):
            self.definition(stmt.definition, frame)
        elif isinstance(stmt, ElseDeclaration):
            self.definition(stmt.definition, self.own_block(replace(frame, required=False)))
        elif isinstance(stmt, TypeDefinition):
            self.types.setdefault(stmt.name, []).append(TypeDeclaration(index, frame.block))
        elif isinstance(stmt, Line):
            self.pattern(stmt.pieces, frame)
        elif isinstance(stmt, Sections):
            # End of file and `last` end sections without storing.
            optional = self.own_block(replace(frame, required=False))
            for clause in stmt.clauses:
                if isinstance(clause, Until) and clause.store is not None:
                    self.store(clause.store, optional)

    def follow_blocks(self, stmt: Stmt, frame: Frame) -> None:
        """Open or close the block ``stmt`` starts or ends, and check an arm's
        pattern in its own block."""
        if isinstance(stmt, Close):
            closed = self.frames.pop()
            if self.stores and self.stores[-1].repeat_block == closed.block:
                pending = self.stores.pop()
                self.current = pending.stmt
                self.store(pending.store, pending.frame)
        elif isinstance(stmt, Repeat):
            repeat_block = self.enter(replace(frame, depth=frame.depth + 1))
            count = stmt.count
            if isinstance(count, Until) and count.store is not None:
                self.stores.append(PendingStore(self.current, repeat_block, count.store, frame))
        elif isinstance(stmt, (If, ElseIf, Else)):
            self.enter(replace(frame, required=False))
        elif isinstance(stmt, Arm):
            arm = self.own_block(replace(frame, required=frame.required and not stmt.optional))
            self.pattern(stmt.pieces, arm)
            if stmt.body:
                self.frames.append(arm)
        elif opens_block(stmt):
            self.enter(frame)
