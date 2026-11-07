"""The printer. It writes what ``iris fmt`` writes with no line limit, so the
text a builder emits is already formatted."""

from __future__ import annotations

import builtins
import math
import re
from collections.abc import Iterable, Iterator
from dataclasses import dataclass, field
from typing import NamedTuple

from gecode_iris._error import CallSite
from gecode_iris.build._error import BuildError
from gecode_iris.build._ops import (
    BinaryOp,
    Binding,
    BindingOp,
    CallOp,
    ComprehensionOp,
    FloatOp,
    HostOp,
    IndexOp,
    IntOp,
    ListMemberOp,
    MemberOp,
    NameOp,
    Op,
    Ops,
    RecordListOp,
    RecordOp,
    StrOp,
    UnaryOp,
    operand_count,
)
from gecode_iris.build._pattern import (
    BindPiece,
    BlankPiece,
    CapturePiece,
    DiscardAsPiece,
    DiscardPiece,
    Piece,
    ReferencePiece,
    RestOfLinePiece,
    TextPiece,
    ValidatePiece,
)
from gecode_iris.build._site import CallSites
from gecode_iris.build._state import (
    Arm,
    Capture,
    Check,
    Close,
    Declaration,
    Definition,
    Else,
    ElseDeclaration,
    ElseIf,
    Format,
    If,
    IgnoreTrailing,
    Index,
    Instances,
    Let,
    Line,
    ListStmt,
    Param,
    Repeat,
    Sections,
    SkipLine,
    SkipLines,
    SkipOther,
    State,
    Stmt,
    Strings,
    Times,
    TypeDefinition,
    UntilEof,
    continues_if,
    declared_mentions,
    expressions,
    is_identifier,
    opens_block,
)
from gecode_iris.build._types import (
    AlphabetBase,
    AlphabetBlank,
    AlphabetOption,
    AlphabetSpec,
    AlphabetStrip,
    BaseSpec,
    Declared,
    NamedSpec,
    RangeSpec,
)

STOP_SET = frozenset(
    {
        "comment",
        "stop",
        "expect",
        "tokens",
        "separators",
        "ignore",
        "param",
        "list",
        "line",
        "find",
        "skip",
        "index",
        "repeat",
        "each",
        "instances",
        "sections",
        "if",
        "let",
        "check",
        "type",
        "layout",
        "offset",
        "until",
    }
)
"""The words that end a range with no upper bound, from the Specification's
``upper bound ahead`` predicate."""

TYPE_WORDS = frozenset({"int", "float", "num", "word", "rest", "alphabet"})
"""The words the type dispatch takes before a range."""

# The Specification's ten precedence levels, loosest first.
DISJUNCTION = 1
CONJUNCTION = 2
EQUALITY = 3
RELATION = 4
ADDITIVE = 5
MULTIPLICATIVE = 6
UNARY = 7
POWER = 8
POSTFIX = 9
PRIMARY = 10

_BINARY_LEVELS = {
    "||": DISJUNCTION,
    "&&": CONJUNCTION,
    "==": EQUALITY,
    "!=": EQUALITY,
    "<": RELATION,
    "<=": RELATION,
    ">": RELATION,
    ">=": RELATION,
    "+": ADDITIVE,
    "-": ADDITIVE,
    "*": MULTIPLICATIVE,
    "/": MULTIPLICATIVE,
    "%": MULTIPLICATIVE,
    "^": POWER,
}
_INT_MIN = -(2**63)
_FIRST_WORD = re.compile(r"[A-Za-z0-9_]*")
_EXPONENT = re.compile(r"e([+-])0*(\d)")


class Printed(NamedTuple):
    """The emitted text, and where each of its statements came from."""

    text: str
    sites: CallSites


def print_spec(state: State) -> Printed:
    """Print the statements as text."""
    printer = _Printer(state, _Names.assign(state))
    printer.statements()
    if printer.issue is not None:
        raise printer.issue
    return printer.printed()


_FIXED_LEVELS: dict[type, int] = {
    UnaryOp: UNARY,
    IndexOp: POSTFIX,
    MemberOp: POSTFIX,
    ListMemberOp: POSTFIX,
}


def _level(op: Op) -> int:
    if type(op) is BinaryOp:
        return _BINARY_LEVELS[op.symbol]
    if type(op) is IntOp:
        return UNARY if op.value < 0 and op.value != _INT_MIN else PRIMARY
    if type(op) is FloatOp:
        return UNARY if math.copysign(1.0, op.value) < 0 else PRIMARY
    return _FIXED_LEVELS.get(type(op), PRIMARY)


def _extents(ops: Ops) -> list[int]:
    """Where each subexpression starts, indexed by the operation that ends it."""
    starts: list[int] = []
    stack: list[int] = []
    for index, op in enumerate(ops):
        first = len(stack) - operand_count(op)
        start = stack[first] if first < len(stack) else index
        del stack[first:]
        stack.append(start)
        starts.append(start)
    return starts


def _operands(starts: builtins.list[int], end: int, count: int) -> list[int]:
    """The last operation of each of the ``count`` operands before ``end``."""
    ends: list[int] = []
    following = end
    for _ in range(count):
        following -= 1
        ends.append(following)
        following = starts[following]
    ends.reverse()
    return ends


def float_text(value: float) -> str:
    """A finite float as Rust's ``{:?}`` writes it: the shortest digits that
    read back, in decimal from 1e-4 up to 1e16 and in exponent form outside."""
    return _EXPONENT.sub(lambda match: "e" + match[1].replace("+", "") + match[2], repr(value))


def _string(text: str) -> str:
    escaped = text.replace("\\", "\\\\").replace('"', '\\"')
    return '"' + escaped.replace("\n", "\\n").replace("\t", "\\t") + '"'


def _first_word(text: str) -> str:
    match = _FIRST_WORD.match(text)
    return match[0] if match else ""


def is_bare_format(name: str) -> bool:
    """Whether a format name can be written without quotes: names and integers
    glued with ``-`` or ``.``, starting with a name."""
    rest = name
    first = True
    while True:
        end = min((i for i in (rest.find("-"), rest.find(".")) if i >= 0), default=len(rest))
        part, tail = rest[:end], rest[end:]
        integer = part.isascii() and part.isdigit() and int(part) <= 2**63 - 1
        if not (is_identifier(part) or (not first and integer and not tail.startswith("."))):
            return False
        if not tail:
            return True
        rest = tail[1:]
        first = False


def _printed_expressions(state: State, stmt: Stmt) -> Iterator[Ops]:
    """Every expression the text of ``stmt`` writes: those it mentions, and the
    types of the captures its pattern names, which print where they bind."""
    yield from expressions(stmt)
    if not isinstance(stmt, (Line, Arm)):
        return
    for piece in stmt.pieces:
        if isinstance(piece, CapturePiece):
            capture = state.names.get(piece.name)
            if isinstance(capture, Capture) and capture.spec is not None:
                for mention in declared_mentions(capture.spec):
                    if isinstance(mention, list):
                        yield mention


@dataclass
class _Names:
    """Internal names, chosen after the whole stream is known."""

    each: dict[int, str] = field(default_factory=dict)
    list: dict[int, str] = field(default_factory=dict)
    bindings: dict[Binding, str] = field(default_factory=dict)

    @classmethod
    def assign(cls, state: State) -> _Names:
        """Choose names in stream order, skipping every name in the spec, so the
        same calls always give the same text."""
        needs_each: set[int] = set()
        needs_list: set[int] = set()
        taken = set(state.names) | state.types
        for record in state.records:
            taken.update(name for name in (record.each, record.list) if name is not None)
        for stmt in state.stmts:
            for ops in _printed_expressions(state, stmt):
                for op in ops:
                    if isinstance(op, RecordOp):
                        needs_each.add(state.record_id(op.record))
                    elif isinstance(op, (RecordListOp, ListMemberOp)):
                        needs_list.add(state.record_id(op.record))
                    elif isinstance(op, ComprehensionOp) and op.name is not None:
                        taken.add(op.name)
        fresh = _Fresh(taken)
        names = cls()
        for stmt in state.stmts:
            if isinstance(stmt, Repeat):
                record_id = state.record_id(stmt.record)
                info = state.records[record_id]
                if info.each is None and record_id in needs_each:
                    names.each[record_id] = fresh.name("record")
                if info.list is None and record_id in needs_list:
                    names.list[record_id] = fresh.name("records")
            for ops in _printed_expressions(state, stmt):
                names.name_bindings(ops, fresh)
        return names

    def name_bindings(self, ops: Ops, fresh: _Fresh) -> None:
        for op in ops:
            if isinstance(op, ComprehensionOp) and op.name is not None:
                self.bindings[op.binding] = op.name
        for op in ops:
            if isinstance(op, (ComprehensionOp, BindingOp)) and op.binding not in self.bindings:
                self.bindings[op.binding] = fresh.name("element")


@dataclass
class _Fresh:
    taken: set[str]
    next: dict[str, int] = field(default_factory=dict)

    def name(self, prefix: str) -> str:
        number = self.next.get(prefix, 1)
        while True:
            name = f"{prefix}_{number}"
            number += 1
            if name not in self.taken:
                self.taken.add(name)
                self.next[prefix] = number
                return name


class _Text(NamedTuple):
    text: str


class _Expression(NamedTuple):
    end: int
    minimum: int


class _Member(NamedTuple):
    """``.name`` after the operand just written."""

    name: str


class _ListMember(NamedTuple):
    """``.records`` after the operand just written, with a repeat's ``as``
    name."""

    record: int


class _Printer:
    def __init__(self, state: State, names: _Names) -> None:
        self.state = state
        self.names = names
        self.out: list[str] = []
        self.length = 0
        """The characters written so far."""
        self.starts: list[int] = []
        """The character offset of each statement, for its call site."""
        self.sites: list[CallSite] = []
        self.current = 0
        """The index of the statement being printed."""
        self.indent = 0
        self.issue: BuildError | None = None

    def write(self, text: str) -> None:
        self.out.append(text)
        self.length += len(text)

    def capture_output(self) -> int:
        return len(self.out)

    def take_output(self, mark: int) -> str:
        """Remove and return what was written since ``mark``."""
        text = "".join(self.out[mark:])
        del self.out[mark:]
        self.length -= len(text)
        return text

    def tail(self, count: int) -> str:
        """The last ``count`` characters written, or fewer at the start."""
        parts: list[str] = []
        gathered = 0
        for chunk in reversed(self.out):
            parts.append(chunk)
            gathered += len(chunk)
            if gathered >= count:
                break
        return "".join(reversed(parts))[-count:]

    def mark_site(self, index: int) -> None:
        self.starts.append(self.length)
        self.sites.append(self.state.sites[index])

    def printed(self) -> Printed:
        text = "".join(self.out)
        return Printed(text, CallSites(_byte_offsets(text, self.starts), self.sites))

    def unprintable(self, message: str) -> None:
        if self.issue is None:
            self.issue = BuildError("unprintable", message, self.state.sites[self.current])

    def start_line(self) -> None:
        self.write("  " * self.indent)

    def statements(self) -> None:
        stmts = self.state.stmts
        index = 0
        continuing = False
        while index < len(stmts):
            stmt = stmts[index]
            self.current = index
            self.mark_site(index)
            index += 1
            if isinstance(stmt, Close):
                self.indent -= 1
                self.start_line()
                self.write("}")
                continuing = self.after_close(stmts, index)
                continue
            if not continuing:
                self.start_line()
            continuing = False
            if isinstance(stmt, Format) and index >= 2 and _ends_open(stmts[index - 2]):
                self.unprintable(
                    "a range with no upper bound cannot come directly before `format`: "
                    "`format` would be its upper bound"
                )
            self.statement(stmt)
            if not opens_block(stmt):
                self.write("\n")
                continue
            each = self.each_line(stmt)
            if each is None and index < len(stmts) and isinstance(stmts[index], Close):
                self.write(" {")
                self.mark_site(index)
                self.write("}")
                index += 1
                continuing = self.after_close(stmts, index)
                continue
            self.write(" {\n")
            self.indent += 1
            if each is not None:
                self.start_line()
                self.write(f"each {each}\n")

    def after_close(self, stmts: builtins.list[Stmt], following: int) -> bool:
        """End the line after ``}``, unless an ``else`` continues it."""
        continues = following < len(stmts) and continues_if(stmts[following])
        self.write(" " if continues else "\n")
        return continues

    def record_each(self, record: int) -> str | None:
        record = self.state.record_id(record)
        name = self.state.records[record].each
        return name if name is not None else self.names.each.get(record)

    def record_list(self, record: int) -> str | None:
        record = self.state.record_id(record)
        name = self.state.records[record].list
        return name if name is not None else self.names.list.get(record)

    def each_line(self, stmt: Stmt) -> str | None:
        return self.record_each(stmt.record) if isinstance(stmt, Repeat) else None

    def statement(self, stmt: Stmt) -> None:
        if isinstance(stmt, Format):
            self.write("format " + (stmt.name if is_bare_format(stmt.name) else _string(stmt.name)))
        elif isinstance(stmt, Strings):
            self.write(stmt.keyword)
            self.strings(stmt.strings)
        elif isinstance(stmt, IgnoreTrailing):
            self.write("ignore trailing")
        elif isinstance(stmt, Param):
            self.write("param ")
            self.target(stmt.definition)
            if stmt.default is not None:
                self.write(" = ")
                self.expression(stmt.default)
        elif isinstance(stmt, Declaration):
            if stmt.definition.name == "format" and self.indent == 0:
                self.unprintable(
                    "an entry named `format` has no spelling at the top level: "
                    "use a `let` or a `list`, or another name"
                )
            self.declaration(stmt.definition, stmt.declared)
        elif isinstance(stmt, ElseDeclaration):
            self.write("else ")
            self.declaration(stmt.definition, stmt.declared)
        elif isinstance(stmt, ListStmt):
            self.list(stmt)
        elif isinstance(stmt, TypeDefinition):
            self.write(f"type {stmt.name} = ")
            self.type_with_options(stmt.declared)
        elif isinstance(stmt, Line):
            self.write(stmt.keyword + " ")
            self.pattern(stmt.pieces)
        elif isinstance(stmt, SkipLine):
            self.write("skip line")
        elif isinstance(stmt, SkipLines):
            self.write("skip ")
            self.guarded(stmt.count, frozenset({"line"}))
            self.write(" lines")
        elif isinstance(stmt, Index):
            self.write("index ")
            self.expression(stmt.position)
        elif isinstance(stmt, Let):
            self.write("let ")
            self.target(stmt.definition)
            self.write(" = ")
            self.expression(stmt.value)
        elif isinstance(stmt, Check):
            self.write("check ")
            self.expression(stmt.condition)
            if stmt.message is not None:
                self.write(" " + _string(stmt.message))
        elif isinstance(stmt, Repeat):
            self.repeat(stmt)
        elif isinstance(stmt, Sections):
            self.sections(stmt)
        elif isinstance(stmt, Arm):
            self.pattern(stmt.pieces)
            self.write(" optional" if stmt.optional else "")
            self.write(" last" if stmt.last else "")
        elif isinstance(stmt, If):
            self.write("if ")
            self.expression(stmt.condition)
        elif isinstance(stmt, ElseIf):
            self.write("else if ")
            self.expression(stmt.condition)
        elif isinstance(stmt, Else):
            self.write("else")
        elif isinstance(stmt, Instances):
            self.write("instances")

    def strings(self, strings: Iterable[str]) -> None:
        for text in strings:
            self.write(" " + _string(text))

    def target(self, definition: Definition) -> None:
        self.write(definition.name if definition.name is not None else "_")

    def declaration(self, definition: Definition, declared: Declared) -> None:
        self.target(definition)
        for dimension in declared.dimensions:
            self.write("[")
            self.expression(dimension)
            self.write("]")
        self.write(": ")
        self.type_with_options(declared)

    def list(self, stmt: ListStmt) -> None:
        if stmt.declared.layout is not None:
            self.unprintable("a `list` has no `layout`")
        self.write("list ")
        self.target(stmt.definition)
        self.write(": ")
        self.type_name(stmt.declared)
        self.write(" until ")
        if stmt.end is None:
            self.write("eol")
        else:
            self.guarded(stmt.end, frozenset({"eol"}))
        self.options(stmt.declared)

    def type_with_options(self, declared: Declared) -> None:
        self.type_name(declared)
        self.options(declared)

    def options(self, declared: Declared) -> None:
        for keyword, option in ((" layout ", declared.layout), (" offset ", declared.offset)):
            if option is not None:
                self.write(keyword)
                self.expression(option)

    def type_name(self, declared: Declared) -> None:
        ty = declared.ty
        if isinstance(ty, (BaseSpec, NamedSpec)):
            self.write(ty.name)
        elif isinstance(ty, AlphabetSpec):
            self.write("alphabet " + _string(ty.symbols))
            for option in ty.options:
                self.alphabet_option(option)
        elif isinstance(ty, RangeSpec):
            if ty.lower is not None:
                self.lower_bound(ty.lower)
            self.write(".." if ty.inclusive else "..<")
            if ty.upper is not None:
                self.guarded(ty.upper, STOP_SET)

    def alphabet_option(self, option: AlphabetOption) -> None:
        if isinstance(option, AlphabetBlank):
            self.write(" blank " + _string(option.characters))
        elif isinstance(option, AlphabetStrip):
            self.write(" strip " + _string(option.characters))
        elif isinstance(option, AlphabetBase):
            if option.value == _INT_MIN:
                self.unprintable("the alphabet base -9223372036854775808 has no spelling")
            else:
                self.write(f" base {option.value}")

    def lower_bound(self, ops: Ops) -> None:
        text = self.expression_text(ops)
        word = _first_word(text)
        if word in TYPE_WORDS or word in self.state.types:
            text = f"({text})"
        self.write(text)

    def guarded(self, ops: Ops, words: frozenset[str]) -> None:
        """Write an expression in parentheses if its first word is one of
        ``words``, which would otherwise be read as a keyword."""
        text = self.expression_text(ops)
        self.write(f"({text})" if _first_word(text) in words else text)

    def expression_text(self, ops: Ops) -> str:
        mark = self.capture_output()
        self.expression(ops)
        return self.take_output(mark)

    def repeat(self, stmt: Repeat) -> None:
        self.write("repeat ")
        count = stmt.count
        if isinstance(count, Times):
            self.guarded(count.count, frozenset({"until"}))
        elif isinstance(count, UntilEof):
            self.write("until eof")
        else:
            self.write("until")
            self.strings(count.terminators)
            self.store(count.store)
        records = self.record_list(stmt.record)
        if records is not None:
            self.write(f" as {records}")

    def store(self, store: Definition | None) -> None:
        if store is not None:
            self.write(" store ")
            self.target(store)

    def sections(self, stmt: Sections) -> None:
        self.write("sections")
        for clause in stmt.clauses:
            if isinstance(clause, SkipOther):
                self.write(" skip other")
            else:
                self.write(" until")
                self.strings(clause.terminators)
                self.store(clause.store)

    def capture_type(self, capture: Declared | None) -> str | None:
        if capture is None:
            return None
        mark = self.capture_output()
        self.type_with_options(capture)
        text = self.take_output(mark)
        if "}" in text:
            self.unprintable(f"the capture type `{text}` contains `}}`")
        return text

    def pattern(self, pieces: builtins.list[Piece]) -> None:
        text: list[str] = []
        for piece in pieces:
            if isinstance(piece, TextPiece):
                if "{" in piece.text or "..." in piece.text:
                    self.unprintable(
                        f"the pattern text `{piece.text}` contains `{{` or `...`, "
                        "which have no literal spelling"
                    )
                text.append(piece.text)
            elif isinstance(piece, CapturePiece):
                state = self.state.names.get(piece.name)
                spec = state.spec if isinstance(state, Capture) else None
                text.append(self.named_capture(piece.name, spec))
            elif isinstance(piece, BindPiece):
                text.append(self.named_capture(piece.name, piece.capture))
            elif isinstance(piece, ReferencePiece):
                text.append(f"{{${piece.name}}}")
            elif isinstance(piece, ValidatePiece):
                text.append(self.named_capture("", piece.capture))
            elif isinstance(piece, BlankPiece):
                text.append("{}")
            elif isinstance(piece, DiscardPiece):
                text.append("{_}")
            elif isinstance(piece, DiscardAsPiece):
                text.append(self.named_capture("_", piece.capture))
            elif isinstance(piece, RestOfLinePiece):
                text.append("...")
        self.write(_string("".join(text)))

    def named_capture(self, name: str, capture: Declared | None) -> str:
        ty = self.capture_type(capture)
        return f"{{{name}}}" if ty is None else f"{{{name}:{ty}}}"

    def literal(self, op: IntOp | FloatOp | StrOp) -> None:
        if isinstance(op, IntOp):
            self.write("(-9223372036854775807 - 1)" if op.value == _INT_MIN else str(op.value))
        elif isinstance(op, FloatOp):
            if math.isfinite(op.value):
                self.write(float_text(op.value))
            else:
                self.unprintable(f"the float {_rust_float(op.value)} has no spelling")
        else:
            self.write(_string(op.text))

    def member(self, name: str) -> None:
        """Write ``.name`` after the operand just written. ``gecode.x`` is one
        host constant token, so after a whole name ``gecode`` the dot keeps its
        spaces."""
        before = self.tail(7)
        whole = before.endswith("gecode") and not (
            len(before) == 7 and (before[0].isascii() and (before[0].isalnum() or before[0] == "_"))
        )
        self.write((" . " if whole else ".") + name)

    def expression(self, ops: Ops) -> None:
        starts = _extents(ops)
        tasks: builtins.list[_Text | _Expression | _Member | _ListMember] = [
            _Expression(len(ops) - 1, DISJUNCTION)
        ]
        while tasks:
            task = tasks.pop()
            if type(task) is _Text:
                self.write(task.text)
            elif type(task) is _Expression:
                if _level(ops[task.end]) < task.minimum:
                    self.write("(")
                    tasks.append(_Text(")"))
                self.operation(ops, starts, task.end, tasks)
            elif type(task) is _Member:
                self.member(task.name)
            elif type(task) is _ListMember:
                self.member(self.record_list(task.record) or "")

    def operation(
        self,
        ops: Ops,
        starts: builtins.list[int],
        end: int,
        tasks: builtins.list[_Text | _Expression | _Member | _ListMember],
    ) -> None:
        op = ops[end]
        if type(op) is BinaryOp:
            left, right = _operands(starts, end, 2)
            level = _BINARY_LEVELS[op.symbol]
            left_minimum, right_minimum = (
                (POSTFIX, UNARY) if op.symbol == "^" else (level, level + 1)
            )
            tasks.append(_Expression(right, right_minimum))
            tasks.append(_Text(f" {op.symbol} "))
            tasks.append(_Expression(left, left_minimum))
        elif type(op) is NameOp:
            self.write(op.text)
        elif isinstance(op, (IntOp, FloatOp, StrOp)):
            self.literal(op)
        elif isinstance(op, HostOp):
            self.write(op.name)
        elif isinstance(op, BindingOp):
            self.write(self.names.bindings[op.binding])
        elif isinstance(op, RecordOp):
            self.write(self.record_each(op.record) or "")
        elif isinstance(op, RecordListOp):
            self.write(self.record_list(op.record) or "")
        elif isinstance(op, UnaryOp):
            self.write(op.symbol)
            tasks.append(_Expression(end - 1, UNARY))
        elif isinstance(op, IndexOp):
            base, index = _operands(starts, end, 2)
            tasks.append(_Text("]"))
            tasks.append(_Expression(index, DISJUNCTION))
            tasks.append(_Text("["))
            tasks.append(_Expression(base, POSTFIX))
        elif isinstance(op, MemberOp):
            tasks.append(_Member(op.text))
            self.member_base(ops, end - 1, tasks)
        elif isinstance(op, ListMemberOp):
            tasks.append(_ListMember(op.record))
            self.member_base(ops, end - 1, tasks)
        elif isinstance(op, CallOp):
            self.write(op.function + "(")
            tasks.append(_Text(")"))
            arguments = _operands(starts, end, op.arguments)
            for position in range(len(arguments) - 1, -1, -1):
                tasks.append(_Expression(arguments[position], DISJUNCTION))
                if position > 0:
                    tasks.append(_Text(", "))
        elif isinstance(op, ComprehensionOp):
            domain, body = _operands(starts, end, 2)
            name = self.names.bindings[op.binding]
            if not is_identifier(name):
                self.unprintable(f"`{name}` is not an Iris name")
            self.write(f"{op.function}({name} in ")
            tasks.append(_Text(")"))
            tasks.append(_Expression(body, DISJUNCTION))
            tasks.append(_Text(": "))
            tasks.append(_Expression(domain, DISJUNCTION))

    def member_base(
        self, ops: Ops, base: int, tasks: builtins.list[_Text | _Expression | _Member | _ListMember]
    ) -> None:
        """A number before ``.`` would lex as a malformed number, so it takes
        parentheses at any level."""
        minimum = PRIMARY + 1 if isinstance(ops[base], (IntOp, FloatOp)) else POSTFIX
        tasks.append(_Expression(base, minimum))


def _rust_float(value: float) -> str:
    if math.isnan(value):
        return "NaN"
    return "inf" if value > 0 else "-inf"


def _ends_open(stmt: Stmt) -> bool:
    if isinstance(stmt, (Declaration, ElseDeclaration, TypeDefinition)):
        return stmt.declared.is_open_ended()
    return False


def _byte_offsets(text: str, offsets: list[int]) -> list[int]:
    """Character offsets in ``text``, which ascend, as UTF-8 byte offsets."""
    if text.isascii():
        return offsets
    result: list[int] = []
    previous = 0
    total = 0
    for offset in offsets:
        total += len(text[previous:offset].encode())
        previous = offset
        result.append(total)
    return result
