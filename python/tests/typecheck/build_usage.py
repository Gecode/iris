"""Builds specs with precise handle types; pyright and mypy accept it.

The type checkers read this file and never run it. Each ``assert_type`` pins
the type that ``data[handle]`` gives for a value type, a depth or a presence.
"""

from __future__ import annotations

from typing import Any

from typing_extensions import assert_type

import gecode_iris as iris
import gecode_iris.build as ib


def value_types(data: iris.Data) -> None:
    b = ib.SpecBuilder("values")
    i = b.read("i", ib.int_)
    f = b.read("f", ib.float_)
    n = b.read("n", ib.num)
    w = b.read("w", ib.word)
    r = b.read("r", ib.rest)
    a = b.read("a", ib.alphabet("ab", blank=" "))
    bounded = b.read("bounded", ib.int_range(0, i))
    assert_type(i, ib.IntField[int])
    assert_type(data[i], int)
    assert_type(data[f], float)
    assert_type(data[n], int | float)
    assert_type(data[w], str)
    assert_type(data[r], str)
    assert_type(data[a], int)
    assert_type(data[bounded], int)


def arrays(data: iris.Data) -> None:
    b = ib.SpecBuilder()
    n = b.read("n", ib.int_)
    row = b.read("row", ib.int_.dim(n))
    grid = b.read("grid", ib.float_.dim(n).dim(2))
    cube = b.read("cube", ib.word.dims(2, 2, n))
    deep = b.read("deep", ib.int_.dims(*[1] * 5))
    values = b.list("values", ib.int_.until(0))
    words = b.list("words", ib.word.until(ib.eol))
    assert_type(data[row], list[int])
    assert_type(data[grid], list[list[float]])
    assert_type(data[cube], list[list[list[str]]])
    assert_type(deep, ib.IntField[Any])
    assert_type(data[values], list[int])
    assert_type(data[words], list[str])
    assert_type(data.ints(row), list[int])


def depths(data: iris.Data) -> None:
    b = ib.SpecBuilder()
    with b.repeat(2) as one:
        assert_type(one, ib.Scope1[ib.Required])
        x1 = one.read("x1", ib.int_)
        row1 = one.read("row1", ib.float_.dim(3))
        with one.repeat(2) as two:
            x2 = two.read("x2", ib.word)
            with two.repeat(2) as three:
                x3 = three.read("x3", ib.num)
                with three.repeat(2) as deeper:
                    assert_type(deeper, ib.ScopeN[ib.Required])
                    x4 = deeper.read("x4", ib.int_)
    assert_type(data[x1], list[int])
    assert_type(data[row1], list[list[float]])
    assert_type(data[x2], list[list[str]])
    assert_type(data[x3], list[list[list[int | float]]])
    assert_type(x4, ib.DynField)
    assert_type(data[x4], iris.Value | None)


def presence(data: iris.Data) -> None:
    b = ib.SpecBuilder()
    n = b.read("n", ib.int_)
    with b.if_(n > 0) as then:
        assert_type(then, ib.Scope0[None])
        x = then.read("x", ib.int_)
        with then.repeat(n) as rows:
            y = rows.read("y", ib.word)
    with b.else_() as otherwise:
        z = otherwise.read("z", ib.float_)
    assert_type(data[x], int | None)
    assert_type(data[y], list[str] | None)
    assert_type(data[z], float | None)

    with b.repeat(n) as r, r.if_(n) as maybe:
        w = maybe.read("w", ib.int_)
    assert_type(data[w], list[int] | None)

    with b.sections() as arms:
        with arms.arm("always") as always:
            a = always.read("a", ib.int_)
        with arms.arm("sometimes", optional=True, last=True) as sometimes:
            s = sometimes.read("s", ib.int_)
    assert_type(data[a], int)
    assert_type(data[s], int | None)


def captures_and_forwards(data: iris.Data) -> None:
    b = ib.SpecBuilder()
    n = b.capture("n", ib.at_least(1))
    name = b.capture("name")
    maybe = b.optional_capture("maybe", ib.float_)
    b.line(("n = ", n, " name = ", name))
    with b.sections() as arms:
        arms.arm(("maybe = ", maybe), optional=True)
    assert_type(data[n], int)
    assert_type(data[name], str)
    assert_type(data[maybe], float | None)

    total = b.forward("total", ib.IntField[int])
    stored = b.forward("stored", ib.StrField[str | None])
    rows = b.forward("rows", ib.IntField[list[int]])
    anything = b.forward("anything", ib.DynField)
    b.let(total, n + 1)
    b.repeat_until("END", store=stored)
    with b.repeat(n) as r:
        r.read(rows, ib.int_)
    b.let(anything, 1)
    assert_type(data[total], int)
    assert_type(data[stored], str | None)
    assert_type(data[rows], list[int])
    assert_type(data[anything], iris.Value | None)


def declared_handles(data: iris.Data) -> None:
    assert_type(data[ib.declared("n", ib.IntField[int])], int)
    assert_type(
        data[ib.declared("x", ib.NumField[list[int | float] | None])], list[int | float] | None
    )
    assert_type(data[ib.declared("d", ib.DynField)], iris.Value | None)


def definitions(data: iris.Data) -> None:
    b = ib.SpecBuilder()
    n = b.read("n", ib.int_)
    a = b.read("a", ib.int_.dim(n))
    copy = b.let("copy", a)
    element = b.let("element", a[0])
    total = b.let("total", ib.sum(a, lambda x: x * 2))
    ratio = b.let("ratio", 1.5)
    label = b.let("label", "text")
    p = b.param("p")
    q = b.param("q", 3)
    from_handle = b.param("from_handle", n)
    from_array = b.param("from_array", a)
    loose = b.let(n, "now a string")
    b.set(total, ib.div(total, 2) + ib.rem(n, 3) - ib.pow(2, n))
    b.read(ib.discard(), ib.int_)
    assert_type(data[copy], list[int])
    assert_type(data[element], iris.Value)
    assert_type(data[total], int)
    assert_type(data[ratio], float)
    assert_type(data[label], str)
    assert_type(data[p], iris.Value | None)
    assert_type(data[q], int)
    assert_type(data[from_handle], int)
    assert_type(data[from_array], iris.Value)
    assert_type(loose, ib.DynField)

    with b.repeat(n) as r:
        doubled = r.let("doubled", n * 2)
        same = r.let("same", doubled)
        outer = r.param("outer", n)
    assert_type(data[outer], iris.Value)
    assert_type(data[doubled], list[int])
    assert_type(data[same], iris.Value)


def records_and_expressions() -> None:
    b = ib.SpecBuilder()
    jobs = b.forward_record("job", "jobs")
    n = b.read("n", ib.int_)
    with b.repeat(n) as r:
        r.record_as(jobs)
        duration = r.read("duration", ib.int_)
        name = r.read("name", ib.word)
        with r.repeat(2) as inner:
            step = inner.read("step", ib.float_)
            steps = inner.record
    assert_type(jobs[duration], ib.IntLookup)
    assert_type(jobs.list[0][name], ib.StrLookup)
    assert_type(jobs.list[0].records(steps)[1][step], ib.FloatLookup)
    b.check(ib.all(jobs, lambda job: job[duration] >= 0), "durations are positive")
    b.check(ib.count(ib.over(jobs, "j"), lambda j: j[name] == "x") <= n)
    b.check(ib.size(jobs) == n)
    b.check(~ib.defined(jobs) | (n > 0) & (n < ib.gecode.INT_MAX))
    b.check(ib.max(n, 1) == ib.min(n, ib.lit(2)))
    b.check(ib.distinct(n, 1, 2))
    dynamic = n.dynamic()
    assert_type(dynamic + 1, ib.IntExpr)
    assert_type(dynamic[0].field(duration), ib.IntLookup)
    assert_type(-ib.lit(1.5), ib.FloatExpr)
    assert_type(ib.lit("s") == "t", ib.IntExpr)


def types_and_patterns() -> None:
    b = ib.SpecBuilder()
    small = b.define_type("small", ib.int_range(0, below=10))
    assert_type(small, ib.IntType[int])
    x = b.read("x", small.dim(2).layout(1).offset(0))
    assert_type(x, ib.IntField[list[int]])
    m = b.read("m", ib.float_.dim(2).dim(2).layout("LOWER_COL"))
    assert_type(m, ib.FloatField[list[list[float]]])
    w = b.forward("w", ib.StrField[str])
    y = b.capture("y", ib.word)
    b.line(("x ", ib.ref(x), " ", ib.bind(w, ib.word), " ", y, ib.rest_of_line))
    b.find((ib.blank(), ib.discard(), ib.discard_as(ib.int_), ib.validate(small)))
    b.expect("a", "b")
    b.skip_lines(2)
    b.index(x[0])
    spec: iris.Spec = b.build()
    text: str = b.source()
    print(spec, text)


@ib.track_caller
def helper(scope: ib.Scope0[object]) -> ib.IntField[object]:
    return scope.read("helper", ib.int_)


def errors() -> None:
    b = ib.SpecBuilder()
    helper(b)
    try:
        b.build()
    except ib.BuildError as error:
        kind: ib.BuildErrorKind = error.kind
        site: iris.CallSite | None = error.call_site
        line: str | None = error.emitted_line
        cause: iris.Error | None = error.error
        print(kind, site, line, cause)
    except iris.Error as error:
        print(error.call_site)
