"""The printed text: every grammar production, the parenthesis and lookahead
tables, and specs far larger than a program would write by hand."""

import math
import tempfile
from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest

import gecode_iris.build as ib
from gecode_iris import _native

Build = Callable[[ib.SpecBuilder], object]


def assert_formatted(text: str) -> None:
    """Check that ``text`` is what ``iris fmt`` writes with no line limit."""
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        (root / ".editorconfig").write_text("root = true\n[*]\nmax_line_length = 1000000\n")
        path = root / "builder.iris"
        path.write_text(text, encoding="utf-8")
        assert _native.run_cli(["iris", "fmt", str(path)]) == 0
        assert path.read_text(encoding="utf-8") == text


def assert_text(build: Build, expected: str) -> ib.SpecBuilder:
    """Check the text of a spec that the language's static checks need not
    accept."""
    b = ib.SpecBuilder()
    build(b)
    text = b.source()
    assert text == expected
    assert_formatted(text)
    return b


def assert_prints(build: Build, expected: str) -> None:
    assert_text(build, expected).build()


def assert_expression(
    build: Callable[[ib.IntField[int], ib.IntField[int], ib.IntField[int]], Any],
    expected: str,
) -> None:
    """``let v = expression``, with ``x``, ``y`` and ``z`` defined first."""

    def define(b: ib.SpecBuilder) -> None:
        x = b.read("x", ib.int_)
        y = b.read("y", ib.int_)
        z = b.read("z", ib.int_)
        b.let("v", build(x, y, z))

    assert_prints(define, f"x: int\ny: int\nz: int\nlet v = {expected}\n")


def test_settings() -> None:
    b = ib.SpecBuilder("alpha-beta.1")
    b.format("two words")
    b.comment("#")
    b.stop("a", "b")
    b.tokens(",;")
    b.separators(":")
    b.ignore_trailing()
    text = b.source()
    assert text == (
        'format alpha-beta.1\nformat "two words"\ncomment "#"\nstop "a" "b"\n'
        'tokens ",;"\nseparators ":"\nignore trailing\n'
    )
    assert_formatted(text)
    b.build()


@pytest.mark.parametrize(
    ("name", "expected"),
    [
        ("a", "format a\n"),
        ("a-b", "format a-b\n"),
        ("a.b-2", "format a.b-2\n"),
        ("a-1.2", 'format "a-1.2"\n'),
        ("a-1e5", 'format "a-1e5"\n'),
        ("1a", 'format "1a"\n'),
        ("a-", 'format "a-"\n'),
        ("", 'format ""\n'),
        ("a b", 'format "a b"\n'),
    ],
)
def test_format_names_are_bare_only_when_they_have_that_spelling(name: str, expected: str) -> None:
    b = ib.SpecBuilder(name)
    assert b.source() == expected
    assert_formatted(expected)
    b.build()


def test_declarations() -> None:
    def build(b: ib.SpecBuilder) -> None:
        n = b.read("n", ib.int_)
        b.read("a", ib.float_)
        b.read("b", ib.num)
        b.read("c", ib.word)
        b.read("d", ib.rest)
        b.read("e", ib.int_.dim(2).dim(n))
        b.read("f", ib.int_.dim(2).dim(2).layout(1).offset(n))
        b.read("g", ib.alphabet("ab", blank=" ", strip="\t", base=-1).dim(2).dim(2))
        b.read("h", ib.alphabet("ab").dims(2, n))
        b.read(ib.discard(), ib.int_)

    assert_prints(
        build,
        "n: int\na: float\nb: num\nc: word\nd: rest\ne[2][n]: int\n"
        "f[2][2]: int layout 1 offset n\n"
        'g[2][2]: alphabet "ab" blank " " strip "\\t" base -1\n'
        'h[2][n]: alphabet "ab"\n_: int\n',
    )


def test_ranges() -> None:
    def build(b: ib.SpecBuilder) -> None:
        n = b.read("n", ib.int_)
        b.read("a", ib.int_range(0, below=10))
        b.read("b", ib.int_range(0, 10))
        b.read("c", ib.at_least(0))
        b.read("d", ib.int_range(below=10))
        b.read("e", ib.int_range(hi=10))
        b.read("f", ib.int_range(0, below=n))
        b.read("g", ib.int_range(0, below=10).dim(n))
        b.read("h", ib.at_least((n + 1).dynamic()))
        b.read("i", ib.int_range(ib.gecode.INT_MIN, ib.gecode.INT_MAX))

    assert_prints(
        build,
        "n: int\na: 0..<10\nb: 0..10\nc: 0..\nd: ..<10\ne: ..10\nf: 0..<n\ng[n]: 0..<10\n"
        "h: n + 1..\ni: gecode.int_min..gecode.int_max\n",
    )


def test_lists_types_and_params() -> None:
    def build(b: ib.SpecBuilder) -> None:
        b.list("a", ib.int_.until(0))
        b.list("b", ib.word.until("end"))
        b.list("c", ib.int_.until(ib.eol).offset(1))
        t = b.define_type("t", ib.at_least(0))
        b.read("x", t)
        u = b.define_type("u", t)
        b.list("d", u.until(1))
        b.param("p")
        b.param("q", 3)

    assert_prints(
        build,
        'list a: int until 0\nlist b: word until "end"\nlist c: int until eol offset 1\n'
        "type t = 0..\nx: t\ntype u = t\nlist d: u until 1\nparam p\nparam q = 3\n",
    )


def test_line_statements() -> None:
    def build(b: ib.SpecBuilder) -> None:
        n = b.read("n", ib.int_)
        b.line("plain")
        b.find('a "quoted" \\ line\n')
        b.skip_line()
        b.skip_lines(n)
        b.index(n + 1)
        b.expect("x")
        b.expect("y", "z")

    assert_prints(
        build,
        'n: int\nline "plain"\nfind "a \\"quoted\\" \\\\ line\\n"\nskip line\nskip n lines\n'
        'index n + 1\nexpect "x"\nexpect "y" "z"\n',
    )


def test_patterns() -> None:
    def build(b: ib.SpecBuilder) -> None:
        x = b.capture("x", ib.int_)
        y = b.capture("y")
        z = b.capture("z", ib.at_least(1))
        w = b.forward("w", ib.StrField[str])
        b.line(("a ", x, " b ", y))
        b.line((z,))
        b.line((ib.ref(x),))
        b.line((ib.validate(ib.float_),))
        b.line((ib.blank(), " ", ib.discard()))
        b.line((ib.discard_as(ib.word),))
        b.line((ib.bind(w, ib.word),))
        b.line(("tail ", ib.rest_of_line))
        b.line(("a", ib.rest_of_line, "  "))
        b.line("lone } brace")

    assert_prints(
        build,
        'line "a {x:int} b {y}"\nline "{z:1..}"\nline "{$x}"\nline "{:float}"\n'
        'line "{} {_}"\nline "{_:word}"\nline "{w:word}"\nline "tail ..."\n'
        'line "a...  "\nline "lone } brace"\n',
    )


def test_repeats() -> None:
    def build(b: ib.SpecBuilder) -> None:
        n = b.read("n", ib.int_)
        with b.repeat(n):
            pass
        with b.repeat(n) as r:
            r.read("a", ib.int_)
        with b.repeat_until_eof() as r:
            r.read("b", ib.int_)
        b.repeat_until_eof()
        b.repeat_until("end")
        stored = b.forward("stored", ib.StrField[str])
        with b.repeat_until("x", "y", store=stored) as r:
            r.read("c", ib.int_)
        b.repeat_until("z", store=b.forward("_", ib.StrField[str]))

    assert_prints(
        build,
        "n: int\nrepeat n {}\nrepeat n {\n  a: int\n}\nrepeat until eof {\n  b: int\n}\n"
        'repeat until eof {}\nrepeat until "end" {}\nrepeat until "x" "y" store stored {\n'
        '  c: int\n}\nrepeat until "z" store _ {}\n',
    )


def test_user_names_for_records_always_print() -> None:
    def build(b: ib.SpecBuilder) -> None:
        with b.repeat(2) as r:
            r.each_name("job")
            r.as_name("jobs")

    assert_prints(build, "repeat 2 as jobs {\n  each job\n}\n")


def test_a_forward_record_serves_use_before_the_repeat() -> None:
    def build(b: ib.SpecBuilder) -> None:
        job = b.forward_record("job", "jobs")
        x = b.forward("x", ib.IntField[list[int]])
        b.let("n", ib.size(job))
        b.let("total", ib.sum(job, lambda j: j[x]))
        with b.repeat(2) as r:
            r.record_as(job)
            r.read(x, ib.int_)

    assert_prints(
        build,
        "let n = size(jobs)\nlet total = sum(element_1 in jobs: element_1.x)\n"
        "repeat 2 as jobs {\n  each job\n  x: int\n}\n",
    )


def test_sections() -> None:
    def build(b: ib.SpecBuilder) -> None:
        stored = b.forward("stored", ib.StrField[str | None])
        a = b.optional_capture("a", ib.int_)
        with b.sections():
            pass
        with b.sections().until("END", store=stored).skip_other().until("x", "y") as s:
            s.arm("plain")
            s.arm(("a ", a), optional=True)
            s.arm("b", last=True)
            s.arm("c", optional=True, last=True)
            with s.arm("d"):
                pass
            with s.arm("e", optional=True, last=True) as e:
                e.read("x", ib.int_)

    assert_prints(
        build,
        'sections {}\nsections until "END" store stored skip other until "x" "y" {\n'
        '  "plain"\n  "a {a:int}" optional\n  "b" last\n  "c" optional last\n  "d" {}\n'
        '  "e" optional last {\n    x: int\n  }\n}\n',
    )


def test_sections_take_their_clauses_as_keywords() -> None:
    def build(b: ib.SpecBuilder) -> None:
        stored = b.forward("stored", ib.StrField[str | None])
        b.sections(until=("a", "b"), skip_other=True, store=stored)
        b.sections(until="c")

    assert_prints(
        build,
        'sections until "a" "b" store stored skip other {}\nsections until "c" {}\n',
    )


def test_branches() -> None:
    def build(b: ib.SpecBuilder) -> None:
        n = b.read("n", ib.int_)
        with b.if_(n):
            pass
        with b.if_(n) as t:
            t.read("a", ib.int_)
        with b.if_(n) as t:
            t.read("b", ib.int_)
        with b.else_() as e:
            e.read("c", ib.int_)
        b.if_(n > 1)
        b.else_if(n > 0)
        b.else_()
        branch = b.forward("if", ib.DynField)
        b.if_(n)
        b.else_read_if(branch, ib.int_)
        with b.if_(n) as t:
            t.read("d", ib.int_)
        with b.else_if(n < 0) as t:
            t.read("e", ib.int_)

    assert_prints(
        build,
        "n: int\nif n {}\nif n {\n  a: int\n}\nif n {\n  b: int\n} else {\n  c: int\n}\n"
        "if n > 1 {} else if n > 0 {} else {}\nif n {} else if: int\n"
        "if n {\n  d: int\n} else if n < 0 {\n  e: int\n}\n",
    )


def test_let_and_check() -> None:
    def build(b: ib.SpecBuilder) -> None:
        x = b.let("x", 1)
        b.set(x, 2)
        b.check(x)
        b.check(x > 0, 'a "quoted" message')

    assert_prints(build, 'let x = 1\nlet x = 2\ncheck x\ncheck x > 0 "a \\"quoted\\" message"\n')


def test_instances_and_a_later_format() -> None:
    b = ib.SpecBuilder("one")
    with b.instances() as i:
        i.read("x", ib.int_)
    b.format("two")
    text = b.source()
    assert text == "format one\ninstances {\n  x: int\n}\nformat two\n"
    assert_formatted(text)
    b.build()


@pytest.mark.parametrize(
    ("value", "expected"),
    [
        (1, "1"),
        (-1, "-1"),
        (1.0, "1.0"),
        (-1.5, "-1.5"),
        (1e300, "1e300"),
        (9007199254740992.0, "9007199254740992.0"),
        (1e16, "1e16"),
        (1e-5, "1e-5"),
        (0.0001, "0.0001"),
        (-0.0, "-0.0"),
        ("a\tb", '"a\\tb"'),
        ("s", '"s"'),
        (4294967295, "4294967295"),
        (-(2**63), "(-9223372036854775807 - 1)"),
        (2**63 - 1, "9223372036854775807"),
    ],
)
def test_literals(value: float | str, expected: str) -> None:
    assert_expression(lambda x, y, z: ib.lit(value), expected)


def test_host_constants() -> None:
    assert_expression(lambda x, y, z: ib.gecode.INT_MAX + 0, "gecode.int_max + 0")
    assert_expression(lambda x, y, z: -ib.gecode.INT_MIN, "-gecode.int_min")


@pytest.mark.parametrize(
    ("build", "expected"),
    [
        (lambda x, y, z: x + y + z, "x + y + z"),
        (lambda x, y, z: x + (y + z), "x + (y + z)"),
        (lambda x, y, z: x - (y - z), "x - (y - z)"),
        (lambda x, y, z: x * y + z, "x * y + z"),
        (lambda x, y, z: x * (y + z), "x * (y + z)"),
        (lambda x, y, z: ib.div(x + y, z), "(x + y) / z"),
        (lambda x, y, z: ib.rem(x, y * z), "x % (y * z)"),
        (lambda x, y, z: x < (y < z), "x < (y < z)"),
        (lambda x, y, z: (x < y) < z, "x < y < z"),
        (lambda x, y, z: (x == y) != z, "x == y != z"),
        (lambda x, y, z: x == (y != z), "x == (y != z)"),
        (lambda x, y, z: ((x <= y) >= z) > 1, "x <= y >= z > 1"),
        (lambda x, y, z: (x < y) == z, "x < y == z"),
        (lambda x, y, z: x & y | z, "x && y || z"),
        (lambda x, y, z: x & (y | z), "x && (y || z)"),
        (lambda x, y, z: (x & y) & z, "x && y && z"),
        (lambda x, y, z: x & (y & z), "x && (y && z)"),
        (lambda x, y, z: ~(x & y), "!(x && y)"),
        (lambda x, y, z: ~x & y, "!x && y"),
        (lambda x, y, z: -(x + y), "-(x + y)"),
        (lambda x, y, z: -(-x), "--x"),  # noqa: B002
        (lambda x, y, z: ~(~x), "!!x"),
        (lambda x, y, z: -(~x), "-!x"),
        (lambda x, y, z: x - (-y), "x - -y"),
        (lambda x, y, z: z > x, "z > x"),
        (lambda x, y, z: ib.size(x) == y, "size(x) == y"),
    ],
)
def test_parentheses_follow_the_precedence_levels(
    build: Callable[[ib.IntField[int], ib.IntField[int], ib.IntField[int]], object],
    expected: str,
) -> None:
    assert_expression(build, expected)


@pytest.mark.parametrize(
    ("build", "expected"),
    [
        (lambda x, y, z: -ib.pow(2, 2), "-2 ^ 2"),
        (lambda x, y, z: ib.pow(-ib.lit(2), 2), "(-2) ^ 2"),
        (lambda x, y, z: ib.pow(-2, 2), "(-2) ^ 2"),
        (lambda x, y, z: ib.pow(2, ib.pow(3, 4)), "2 ^ 3 ^ 4"),
        (lambda x, y, z: ib.pow(ib.pow(2, 3), 4), "(2 ^ 3) ^ 4"),
        (lambda x, y, z: ib.pow(2, -x), "2 ^ -x"),
        (lambda x, y, z: ib.pow(x + y, 2), "(x + y) ^ 2"),
        (lambda x, y, z: ib.pow(2, x + y), "2 ^ (x + y)"),
        (lambda x, y, z: x * ib.pow(y, z), "x * y ^ z"),
    ],
)
def test_powers_bind_tighter_than_unary_on_the_left_only(
    build: Callable[[ib.IntField[int], ib.IntField[int], ib.IntField[int]], object],
    expected: str,
) -> None:
    assert_expression(build, expected)


def test_operators_take_literals_on_either_side() -> None:
    assert_expression(lambda x, y, z: 1 + x, "1 + x")
    assert_expression(lambda x, y, z: 2 * (x - 1), "2 * (x - 1)")
    assert_expression(lambda x, y, z: ib.div(7, x), "7 / x")
    assert_expression(lambda x, y, z: ib.rem(7, x), "7 % x")
    assert_expression(lambda x, y, z: 1 + ib.gecode.INT_MAX - x, "1 + gecode.int_max - x")
    assert_expression(lambda x, y, z: 1 < x, "x > 1")


def test_calls_and_comprehensions() -> None:
    def build(b: ib.SpecBuilder) -> None:
        a = b.read("a", ib.int_.dim(2))
        b.let("v1", ib.size(a))
        b.let("v2", ib.count(a))
        b.let("v3", ib.min(a))
        b.let("v4", ib.max(a, 1))
        b.let("v5", ib.sum(1, 2, 3))
        b.let("v6", ib.distinct(1, 2))
        b.let("v7", ib.defined(a))
        b.let("v8", ib.all(a, lambda x: x > 0))
        b.let("v9", ib.any(a, lambda x: x > 0))
        b.let("v10", ib.sum(a, lambda x: x))
        b.let("v11", ib.count(ib.over(a, "item"), lambda x: x > 1))
        b.let("v12", ib.sum(a, lambda x: ib.sum(a, lambda y: x.dynamic() * y)))
        b.let("v13", a[0] + a[1])

    assert_prints(
        build,
        "a[2]: int\nlet v1 = size(a)\nlet v2 = count(a)\nlet v3 = min(a)\nlet v4 = max(a, 1)\n"
        "let v5 = sum(1, 2, 3)\nlet v6 = distinct(1, 2)\nlet v7 = defined(a)\n"
        "let v8 = all(element_1 in a: element_1 > 0)\n"
        "let v9 = any(element_2 in a: element_2 > 0)\n"
        "let v10 = sum(element_3 in a: element_3)\nlet v11 = count(item in a: item > 1)\n"
        "let v12 = sum(element_4 in a: sum(element_5 in a: element_4 * element_5))\n"
        "let v13 = a[0] + a[1]\n",
    )


def test_internal_names_skip_every_name_in_the_spec() -> None:
    def build(b: ib.SpecBuilder) -> None:
        b.read("record_1", ib.int_)
        b.read("records_1", ib.int_)
        a = b.read("element_1", ib.int_.dim(1))
        with b.repeat(1) as r:
            job, x = r.record, r.read("x", ib.int_)
        b.let("v", ib.sum(job, lambda j: j[x]))
        b.check(job[x])
        b.let("u", ib.all(a, lambda e: e))

    assert_prints(
        build,
        "record_1: int\nrecords_1: int\nelement_1[1]: int\nrepeat 1 as records_2 {\n"
        "  each record_2\n  x: int\n}\nlet v = sum(element_2 in records_2: element_2.x)\n"
        "check record_2.x\nlet u = all(element_3 in element_1: element_3)\n",
    )


def test_capture_types_name_the_records_and_elements_they_use() -> None:
    def build(b: ib.SpecBuilder) -> tuple[Any, Any, Any]:
        a = b.read("a", ib.int_.dim(2))
        x = b.capture("x", ib.int_range(0, ib.sum(a, lambda v: v)))
        b.line((x,))
        with b.repeat(2) as r:
            record, y = r.record, r.read("y", ib.int_)
            z = r.capture("z", ib.int_range(0, record[y]))
            r.line((z,))
        w = b.capture("w", ib.int_range(0, ib.sum(record, lambda e: e[y])))
        b.line((w,))
        return x, z, w

    handles: list[tuple[Any, Any, Any]] = []
    b = assert_text(
        lambda b: handles.append(build(b)),
        'a[2]: int\nline "{x:0..sum(element_1 in a: element_1)}"\n'
        'repeat 2 as records_1 {\n  each record_1\n  y: int\n  line "{z:0..record_1.y}"\n}\n'
        'line "{w:0..sum(element_2 in records_1: element_2.y)}"\n',
    )
    x, z, w = handles[0]
    data = b.build().read(b"1 2\n3\n1\n1\n2\n2\n3\n")
    assert (data[x], data[z], data[w]) == (3, [1, 2], 3)


def test_records_project_and_index() -> None:
    def build(b: ib.SpecBuilder) -> None:
        with b.repeat(2) as r:
            with r.repeat(2) as s:
                inner, x = s.record, s.read("x", ib.int_)
            outer = r.record
        b.let("a", outer.list[0][x][1])
        b.check(ib.size(outer.records(inner)))
        b.let("c", outer.list[1].records(inner)[0][x])
        b.let("d", ib.size(outer.list))
        b.check(ib.defined(outer))
        b.let("f", ib.sum(outer, lambda o: ib.sum(o.records(inner), lambda i: i[x])))

    assert_prints(
        build,
        "repeat 2 as records_1 {\n  each record_1\n  repeat 2 as records_2 {\n    x: int\n  }\n}\n"
        "let a = records_1[0].x[1]\ncheck size(record_1.records_2)\n"
        "let c = records_1[1].records_2[0].x\nlet d = size(records_1)\n"
        "check defined(records_1)\n"
        "let f = sum(element_1 in records_1: sum(element_2 in element_1.records_2: element_2.x))\n",
    )


def test_a_member_never_glues_to_a_literal_or_a_name_gecode() -> None:
    def build(b: ib.SpecBuilder) -> None:
        x = b.read("x", ib.int_)
        g = b.undefined("gecode")
        b.let("a", ib.lit(1).dynamic().field(x))
        b.let("b", g[0].field(x))
        b.let("c", g.dynamic().field(x))
        b.let("d", g.dynamic().field(g).field(x))
        b.let("e", (ib.lit(1).dynamic() + 2).dynamic().field(x))

    assert_text(
        build,
        "x: int\nlet a = (1).x\nlet b = gecode[0].x\nlet c = gecode . x\n"
        "let d = gecode . gecode . x\nlet e = (1 + 2).x\n",
    )


def test_keywords_at_the_start_of_an_expression_take_parentheses() -> None:
    def build(b: ib.SpecBuilder) -> None:
        until, line, eol, int_, layout, alpha, t = (
            b.read(name, ib.int_)
            for name in ("until", "line", "eol", "int", "layout", "alphabet", "t")
        )
        with b.repeat(until):
            pass
        with b.repeat(until + 1):
            pass
        b.skip_lines(line)
        b.skip_lines(line * 2)
        b.list("a", ib.int_.until(eol))
        b.list("b", ib.int_.until(eol + 1))
        b.read("c", ib.int_range(0, layout))
        b.read("d", ib.int_range(0, below=layout))
        b.read("e", ib.int_range(hi=layout + 1))
        b.read("f", ib.int_range(int_, t))
        b.read("g", ib.at_least(int_ + 1))
        b.read("h", ib.at_least(alpha))
        b.read("j", ib.at_least(t))
        b.line((ib.validate(ib.int_range(0, layout)),))

    assert_prints(
        build,
        "until: int\nline: int\neol: int\nint: int\nlayout: int\nalphabet: int\nt: int\n"
        "repeat (until) {}\nrepeat (until + 1) {}\nskip (line) lines\n"
        "skip (line * 2) lines\nlist a: int until (eol)\nlist b: int until (eol + 1)\n"
        "c: 0..(layout)\nd: 0..<(layout)\ne: ..(layout + 1)\nf: (int)..t\ng: (int + 1)..\n"
        'h: (alphabet)..\nj: t..\nline "{:0..(layout)}"\n',
    )


def test_a_named_type_takes_parentheses_as_a_lower_bound() -> None:
    def build(b: ib.SpecBuilder) -> None:
        named = b.undefined("named")
        b.define_type("named", ib.int_)
        b.read("x", ib.at_least(named.dynamic()))

    assert_text(build, "type named = int\nx: (named)..\n")


@pytest.mark.parametrize(
    "word",
    [
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
    ],
)
def test_every_stop_word_is_guarded_as_an_upper_bound(word: str) -> None:
    b = ib.SpecBuilder()
    bound = b.let(word, 5)
    b.read("x", ib.int_range(0, bound))
    text = b.source()
    assert text.endswith(f"x: 0..({word})\n"), text
    assert_formatted(text)
    assert b.build().read(b"3")["x"] == 3


def test_numbers_round_trip() -> None:
    b = ib.SpecBuilder()
    values = [0.1, 1e-7, 1.7976931348623157e308, 123456789012345680.0, -0.0]
    fields = [b.let(f"v{index}", value) for index, value in enumerate(values)]
    minimum = b.let("minimum", -(2**63))
    data = b.build().read(b"")
    for handle, value in zip(fields, values, strict=True):
        assert math.copysign(1, data[handle]) == math.copysign(1, value)
        assert data[handle] == value
    assert data[minimum] == -(2**63)


def test_a_long_chain_prints() -> None:
    """Composing keeps a tree, so a long chain takes linear time and no
    recursion."""
    b = ib.SpecBuilder()
    x = b.read("x", ib.int_)
    chain = x + 1
    for _ in range(200_000):
        chain = chain + x
    b.let("v", chain)
    assert b.source().count("+") == 200_001


def test_a_deep_right_chain_prints_without_recursion() -> None:
    b = ib.SpecBuilder()
    x = b.read("x", ib.int_)
    chain = -x
    for _ in range(20_000):
        chain = -chain
    b.let("v", chain)
    assert b.source().count("-") == 20_001


def test_three_hundred_dimensions_read() -> None:
    b = ib.SpecBuilder()
    deep = b.read("deep", ib.int_.dims(*[1] * 300))
    value: Any = b.build().read(b"7")[deep]
    for _ in range(299):
        value = value[0]
    assert value == [7]


def test_ten_nested_repeats_stay_dynamic_past_the_static_depths() -> None:
    def nest(scope: Any, depth: int) -> ib.Field[object]:
        if depth == 0:
            return scope.read("leaf", ib.int_)
        with scope.repeat(1) as inner:
            return nest(inner, depth - 1)

    b = ib.SpecBuilder()
    leaf = nest(b, 10)
    assert isinstance(leaf, ib.DynField)
    value: Any = b.build().read(b"9")[leaf]
    for _ in range(9):
        value = value[0]
    assert value == [9]


def test_num_entries_read_as_numbers() -> None:
    b = ib.SpecBuilder()
    x = b.read("x", ib.num)
    assert b.build().read(b"2.5")[x] == 2.5


def test_t_string_and_tuple_patterns_print_alike() -> None:
    b = ib.SpecBuilder()
    x = b.capture("x", ib.int_)
    b.line(("x = ", x, ", ", ib.blank(), ""))
    assert b.source() == 'line "x = {x:int}, {}"\n'
