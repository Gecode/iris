"""Build reader specs from Python code, with typed handles.

A :class:`SpecBuilder` is the root scope of a spec. Each statement is a method
call, a block is a ``with`` statement, and every definition returns a handle
that later statements use in expressions and that ``data[handle]`` reads from
the data with the type it promises::

    import gecode_iris.build as ib

    b = ib.SpecBuilder("jobshop")
    jobs = b.read("jobs", ib.int_range(1))
    machines = b.read("machines", ib.int_range(1))
    with b.repeat(jobs) as job:
        with job.repeat(machines) as task:
            machine = task.read("machine", ib.int_range(0, machines - 1))
            duration = task.read("duration", ib.at_least(0))
    spec = b.build()

    data = spec.read(b"1 2\\n0 3 1 4\\n")
    data[duration]  # [[3, 4]], typed list[list[int]]

Python operators build Iris expressions, except for division, remainder and
power, which are :func:`div`, :func:`rem` and :func:`pow`, and the logical
operators, which are ``&``, ``|`` and ``~``. Line patterns are t-strings on
Python 3.14 and later, and tuples of text and parts on every version.

A mistake that a builder can find raises :class:`BuildError` at the call that
made it, or from ``build()``, and names the caller's file and line. Errors
while reading a built spec name the call site of their statement in
:attr:`gecode_iris.Error.call_site`.
"""

from typing import NoReturn

from gecode_iris._error import CallSite
from gecode_iris.build import gecode
from gecode_iris.build._error import BuildError, BuildErrorKind
from gecode_iris.build._expr import (
    DynExpr,
    DynField,
    Expr,
    Field,
    FloatExpr,
    FloatField,
    FloatLookup,
    IntExpr,
    IntField,
    IntLookup,
    NumExpr,
    NumField,
    NumLookup,
    Record,
    RecordExpr,
    RecordList,
    StrExpr,
    StrField,
    StrLookup,
    all,
    any,
    count,
    defined,
    distinct,
    div,
    lit,
    max,
    min,
    over,
    pow,
    rem,
    size,
    sum,
)
from gecode_iris.build._pattern import (
    Discard,
    Part,
    bind,
    blank,
    discard,
    discard_as,
    ref,
    rest_of_line,
    validate,
)
from gecode_iris.build._scope import (
    ArmGuard,
    Arms,
    Block,
    Scope,
    Scope0,
    Scope1,
    Scope2,
    Scope3,
    ScopeN,
    SectionsGuard,
    SpecBuilder,
)
from gecode_iris.build._site import track_caller
from gecode_iris.build._types import (
    Alphabet,
    Eol,
    FloatType,
    IntType,
    ListReading,
    NumType,
    StrType,
    Type,
    alphabet,
    at_least,
    eol,
    float_,
    int_,
    int_range,
    num,
    rest,
    word,
)

Required = NoReturn
"""The presence of a scope whose statements always execute."""

Maybe = None
"""The presence of a scope whose statements may not execute."""

__all__ = [
    "Alphabet",
    "ArmGuard",
    "Arms",
    "Block",
    "BuildError",
    "BuildErrorKind",
    "CallSite",
    "Discard",
    "DynExpr",
    "DynField",
    "Eol",
    "Expr",
    "Field",
    "FloatExpr",
    "FloatField",
    "FloatLookup",
    "FloatType",
    "IntExpr",
    "IntField",
    "IntLookup",
    "IntType",
    "ListReading",
    "Maybe",
    "NumExpr",
    "NumField",
    "NumLookup",
    "NumType",
    "Part",
    "Record",
    "RecordExpr",
    "RecordList",
    "Required",
    "Scope",
    "Scope0",
    "Scope1",
    "Scope2",
    "Scope3",
    "ScopeN",
    "SectionsGuard",
    "SpecBuilder",
    "StrExpr",
    "StrField",
    "StrLookup",
    "StrType",
    "Type",
    "all",
    "alphabet",
    "any",
    "at_least",
    "bind",
    "blank",
    "count",
    "defined",
    "discard",
    "discard_as",
    "distinct",
    "div",
    "eol",
    "float_",
    "gecode",
    "int_",
    "int_range",
    "lit",
    "max",
    "min",
    "num",
    "over",
    "pow",
    "ref",
    "rem",
    "rest",
    "rest_of_line",
    "size",
    "sum",
    "track_caller",
    "validate",
    "word",
]
