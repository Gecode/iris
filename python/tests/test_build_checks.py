"""The builder's own checks: each raises ``BuildError`` at the call that failed
it, or from ``build()``, which raises the first one again."""

import sys
from collections.abc import Callable
from typing import Any, cast

import pytest

import gecode_iris.build as ib
from gecode_iris import Spec

Build = Callable[[ib.SpecBuilder], object]


def failure_of(build: Build, kind: str) -> ib.BuildError:
    """Run ``build`` on a new builder and expect an error of ``kind``, from a
    statement or from ``build()``. A statement's error comes again from
    ``build()``."""
    b = ib.SpecBuilder()
    try:
        build(b)
    except ib.BuildError as raised:
        with pytest.raises(ib.BuildError) as again:
            b.build()
        assert again.value is raised
        error = raised
    else:
        with pytest.raises(ib.BuildError) as info:
            b.build()
        error = info.value
    assert error.kind == kind, str(error)
    return error


def builds(build: Build) -> Spec:
    b = ib.SpecBuilder()
    build(b)
    return b.build()


def test_a_statement_through_an_outer_scope_is_rejected() -> None:
    def build(b: ib.SpecBuilder) -> None:
        with b.instances() as s:
            n = s.capture("n", ib.int_)
            with s.sections() as arms, arms.arm((n,)):
                s.read("x", ib.int_)

    failure_of(build, "scope")


def test_an_arm_through_an_outer_block_is_rejected() -> None:
    def build(b: ib.SpecBuilder) -> None:
        with b.sections() as arms, arms.arm("a"):
            arms.arm("b")

    failure_of(build, "scope")


def test_a_guard_after_later_statements_is_rejected() -> None:
    def branch(b: ib.SpecBuilder) -> None:
        n = b.read("n", ib.int_)
        with b.if_(n):
            pass
        b.read("y", ib.int_)
        with b.else_():
            pass

    failure_of(branch, "scope")

    def repeat(b: ib.SpecBuilder) -> None:
        guard = b.repeat_until("end")
        b.read("y", ib.int_)
        with guard:
            pass

    failure_of(repeat, "scope")

    def clause(b: ib.SpecBuilder) -> None:
        guard = b.sections()
        b.read("y", ib.int_)
        guard.skip_other()

    failure_of(clause, "scope")


def test_a_handle_from_another_builder_is_rejected() -> None:
    other = ib.SpecBuilder()
    foreign = other.read("x", ib.int_)
    with other.repeat(1) as r:
        record = r.record
    named = other.define_type("t", ib.int_)

    failure_of(lambda b: b.let("y", foreign + 1), "provenance")
    failure_of(lambda b: b.read(foreign, ib.int_), "provenance")
    failure_of(lambda b: b.line(("", ib.ref(foreign))), "provenance")
    failure_of(lambda b: b.check(record.list[0][foreign]), "provenance")
    failure_of(lambda b: b.read("x", named), "provenance")
    failure_of(lambda b: b.capture("x", named), "provenance")


def test_a_declared_handle_reads_but_does_not_build() -> None:
    x = ib.declared("x", ib.IntField[int])
    failure_of(lambda b: b.let("y", x), "provenance")
    with pytest.raises(TypeError, match="declared takes a handle type"):
        ib.declared("x", cast(Any, int))
    with pytest.raises(TypeError, match="does not match its value type"):
        ib.declared("x", ib.IntField[str])


def test_defining_a_name_twice_needs_the_handle() -> None:
    def twice(b: ib.SpecBuilder) -> None:
        b.read("x", ib.int_)
        b.let("x", 1)

    error = failure_of(twice, "redefinition")
    assert "`x`" in str(error)

    def through_the_handle(b: ib.SpecBuilder) -> None:
        x = b.read("x", ib.int_)
        b.set(x, 2)
        b.let(x, 1.5)

    builds(through_the_handle)


def test_values_with_no_spelling_are_rejected() -> None:
    def a_lone_alphabet(b: ib.SpecBuilder) -> None:
        a = b.read("a", ib.int_.dim(1))
        b.let("v", ib.all(ib.over(a, "not a name"), lambda x: x))

    def a_range_before_format(b: ib.SpecBuilder) -> None:
        b.read("x", ib.at_least(0))
        b.format("second")

    def a_type_before_format(b: ib.SpecBuilder) -> None:
        b.define_type("t", ib.at_least(0))
        b.format("second")

    def a_brace_in_a_validated_type(b: ib.SpecBuilder) -> None:
        b.line(("", ib.validate(ib.at_least(ib.lit("}").dynamic()))))

    unprintable: list[Build] = [
        lambda b: b.line("a { b"),
        lambda b: b.line("a ... b"),
        lambda b: b.read("format", ib.int_),
        lambda b: b.read("_", ib.int_),
        lambda b: b.list("_", ib.int_.until(0)),
        lambda b: b.let("_", 1),
        lambda b: b.param("_"),
        lambda b: b.define_type("_", ib.int_),
        lambda b: b.capture("_", ib.int_),
        lambda b: b.optional_capture("_", ib.int_),
        lambda b: b.repeat_until("x", store="_"),
        lambda b: b.repeat_until("x", store=ib.discard()),
        lambda b: b.sections(until="x", store=ib.discard()),
        lambda b: b.read("gecode.x", ib.int_),
        lambda b: b.read("not a name", ib.int_),
        lambda b: b.read("", ib.int_),
        a_lone_alphabet,
        lambda b: b.let("x", float("nan")),
        lambda b: b.let("x", float("inf")),
        a_range_before_format,
        a_type_before_format,
        lambda b: b.read("x", ib.alphabet("ab", base=-(2**63))),
        lambda b: b.list("x", ib.int_.layout(1).until(0)),
        a_brace_in_a_validated_type,
        lambda b: b.sections(skip_other=True, store="end"),
    ]
    for build in unprintable:
        failure_of(build, "unprintable")


def test_a_bound_after_an_open_range_is_fine_before_other_statements() -> None:
    def build(b: ib.SpecBuilder) -> None:
        b.read("x", ib.at_least(0))
        b.read("y", ib.at_least(0))
        b.instances()
        b.format("second")

    builds(build)


def test_a_capture_binds_exactly_once_in_its_scope() -> None:
    def twice(b: ib.SpecBuilder) -> None:
        n = b.capture("n", ib.int_)
        b.line((n,))
        b.line((n,))

    assert "twice" in str(failure_of(twice, "binding"))

    def never(b: ib.SpecBuilder) -> None:
        b.capture("n", ib.int_)

    assert "never" in str(failure_of(never, "binding"))

    def elsewhere(b: ib.SpecBuilder) -> None:
        n = b.capture("n", ib.int_)
        with b.repeat(1) as r:
            r.line((n,))

    assert "different scope" in str(failure_of(elsewhere, "binding"))


def test_the_presence_of_a_capture_follows_its_arm() -> None:
    def required_in_optional(b: ib.SpecBuilder) -> None:
        n = b.capture("n", ib.int_)
        with b.sections() as arms:
            arms.arm((n,), optional=True)

    assert "optional_capture" in str(failure_of(required_in_optional, "binding"))

    def optional_in_line(b: ib.SpecBuilder) -> None:
        n = b.optional_capture("n", ib.int_)
        b.line((n,))

    assert "`capture`" in str(failure_of(optional_in_line, "binding"))

    def optional_in_optional(b: ib.SpecBuilder) -> None:
        n = b.optional_capture("n", ib.int_)
        with b.sections() as arms:
            arms.arm((n,), optional=True)

    builds(optional_in_optional)


def test_a_handle_that_is_not_a_capture_is_not_guessed() -> None:
    def bound(b: ib.SpecBuilder) -> None:
        x = b.read("x", ib.int_)
        b.line((x,))

    assert "`ib.ref`" in str(failure_of(bound, "binding"))

    def referenced(b: ib.SpecBuilder) -> None:
        x = b.read("x", ib.int_)
        b.line(("", ib.ref(x)))

    builds(referenced)


def test_a_forward_handle_must_be_bound() -> None:
    error = failure_of(lambda b: b.forward("x", ib.IntField[int]), "binding")
    assert "never" in str(error)
    failure_of(lambda b: b.forward_record("each_one", "all_of_them"), "binding")


def test_a_forward_handle_must_be_bound_as_declared() -> None:
    def other_type(b: ib.SpecBuilder) -> None:
        x = b.forward("x", ib.IntField[int])
        b.read(x, ib.float_)

    failure_of(other_type, "binding")

    def other_depth(b: ib.SpecBuilder) -> None:
        x = b.forward("x", ib.IntField[int])
        with b.repeat(2) as r:
            r.read(x, ib.int_)

    failure_of(other_depth, "binding")

    def maybe_absent(b: ib.SpecBuilder) -> None:
        x = b.forward("x", ib.IntField[int])
        with b.if_(1) as t:
            t.read(x, ib.int_)

    failure_of(maybe_absent, "binding")

    def as_declared(b: ib.SpecBuilder) -> None:
        x = b.forward("x", ib.IntField[list[int]])
        with b.repeat(2) as r:
            r.read(x, ib.int_)

    builds(as_declared)


def test_a_later_definition_of_another_type_is_a_redefinition() -> None:
    b = ib.SpecBuilder()
    x = b.forward("x", ib.IntField[int])
    b.read(x, ib.int_)
    again = b.read(x, ib.float_)
    assert isinstance(again, ib.DynField)
    data = b.build().read(b"1 2.5")
    assert data[again] == 2.5
    with pytest.raises(TypeError, match="does not match its handle"):
        data[x]

    def twice_in_a_branch(b: ib.SpecBuilder) -> None:
        x = b.forward("x", ib.IntField[int])
        with b.if_(1) as t:
            t.read(x, ib.int_)
        with b.if_(1) as t:
            t.read(x, ib.int_)
            t.read(x, ib.float_)

    failure_of(twice_in_a_branch, "binding")


def test_a_redefinition_needs_a_binding_that_always_precedes_it() -> None:
    def in_else(b: ib.SpecBuilder) -> None:
        x = b.forward("x", ib.IntField[int | None])
        with b.if_(1) as t:
            t.read(x, ib.int_)
        with b.else_() as e:
            e.read(x, ib.float_)

    assert "float at depth 0" in str(failure_of(in_else, "binding"))

    def after_a_branch(b: ib.SpecBuilder) -> None:
        x = b.forward("x", ib.IntField[int | None])
        with b.if_(1) as t:
            t.read(x, ib.int_)
        b.read(x, ib.float_)

    failure_of(after_a_branch, "binding")

    def in_two_arms(b: ib.SpecBuilder) -> None:
        x = b.forward("x", ib.IntField[int | None])
        with b.sections() as arms:
            arms.arm(("", ib.bind(x, ib.int_)), optional=True)
            arms.arm(("", ib.bind(x, ib.float_)), optional=True)

    failure_of(in_two_arms, "binding")

    def inside_a_block(b: ib.SpecBuilder) -> None:
        x = b.forward("x", ib.IntField[int | None])
        with b.if_(1) as t:
            t.read(x, ib.int_)
            with t.if_(1) as nested:
                nested.read(x, ib.float_)

    builds(inside_a_block)


def test_an_unknown_shape_does_not_bind_a_static_forward_handle() -> None:
    def array_into(let: Callable[[ib.SpecBuilder, ib.IntField[list[int]]], object]) -> Build:
        def build(b: ib.SpecBuilder) -> None:
            a = b.read("a", ib.int_.dim(2))
            let(b, a)

        return build

    def whole(b: ib.SpecBuilder, a: ib.IntField[list[int]]) -> None:
        b.let(b.forward("x", ib.IntField[int]), a)

    assert "int at depth 1" in str(failure_of(array_into(whole), "binding"))

    def element(b: ib.SpecBuilder, a: ib.IntField[list[int]]) -> None:
        b.let(b.forward("x", ib.IntField[int]), a[0])

    def dynamic(b: ib.SpecBuilder, a: ib.IntField[list[int]]) -> None:
        b.let(b.forward("x", ib.IntField[int]), a.dynamic())

    for let in (element, dynamic):
        failure_of(array_into(let), "binding")

    def dynamic_text(b: ib.SpecBuilder) -> None:
        b.let(b.forward("x", ib.IntField[int]), ib.lit("text").dynamic())

    failure_of(dynamic_text, "binding")
    failure_of(lambda b: b.param(b.forward("x", ib.IntField[int])), "binding")

    def bare_in_a_repeat(b: ib.SpecBuilder) -> None:
        y = b.forward("y", ib.IntField[list[int]])
        with b.repeat(1) as r:
            v = r.read("v", ib.int_)
            r.let(y, v)

    assert "unknown depth" in str(failure_of(bare_in_a_repeat, "binding"))


def test_a_known_shape_binds_a_static_forward_handle() -> None:
    b = ib.SpecBuilder()
    a = b.read("a", ib.int_.dim(2))
    n = b.read("n", ib.int_)
    whole = b.forward("whole", ib.IntField[list[int]])
    b.let(whole, a)
    scalar = b.forward("scalar", ib.IntField[int])
    b.let(scalar, n)
    computed = b.forward("computed", ib.IntField[int])
    b.let(computed, n + 1)
    rows = b.forward("rows", ib.IntField[list[int]])
    with b.repeat(2) as r:
        r.let(rows, n + 2)
    data = b.build().read(b"1 2 3")
    assert data[whole] == [1, 2]
    assert data[scalar] == 3
    assert data[computed] == 4
    assert data[rows] == [5, 5]

    b = ib.SpecBuilder()
    a = b.read("a", ib.int_.dim(2))
    x = b.forward("x", ib.DynField)
    b.let(x, a[0])
    y = b.forward("y", ib.DynField)
    with b.repeat(1) as r:
        r.let(y, a)
    data = b.build().read(b"7 8")
    assert data[x] == 7
    assert data[y] == [[7, 8]]


def test_set_keeps_a_shape_the_types_cannot_compare_until_the_walk() -> None:
    def build(b: ib.SpecBuilder) -> None:
        a = b.read("a", ib.int_.dim(2))
        y = b.forward("y", ib.IntField[list[int]])
        with b.repeat(1) as r:
            r.set(y, a)

    failure_of(build, "binding")


def test_set_rejects_another_value_type_or_a_known_other_depth() -> None:
    b = ib.SpecBuilder()
    x = b.read("x", ib.int_)
    with pytest.raises(TypeError, match="value type int"):
        b.set(x, 1.5)
    a = b.read("a", ib.int_.dim(2))
    with pytest.raises(TypeError, match="redefine the name with `let"):
        b.set(x, a)
    loose = b.let(x, 1.5)
    b.set(loose, "anything")
    b.build()


def test_a_named_type_handle_belongs_to_its_declaration() -> None:
    def old_type(b: ib.SpecBuilder) -> ib.IntType[int]:
        with b.if_(1) as s:
            return s.define_type("t", ib.int_)

    def read(b: ib.SpecBuilder) -> None:
        old = old_type(b)
        with b.if_(1) as s:
            s.define_type("t", ib.float_)
            s.read("x", old)

    assert "`t`" in str(failure_of(read, "scope"))

    def capture(b: ib.SpecBuilder) -> None:
        old = old_type(b)
        with b.if_(1) as s:
            s.define_type("t", ib.float_)
            x = s.optional_capture("x", old)
            s.line((x,))

    failure_of(capture, "scope")

    def validate(b: ib.SpecBuilder) -> None:
        old = old_type(b)
        with b.if_(1) as s:
            s.define_type("t", ib.float_)
            s.line(("", ib.validate(old)))

    failure_of(validate, "scope")

    def after(b: ib.SpecBuilder) -> None:
        b.read("x", old_type(b))

    failure_of(after, "scope")

    b = ib.SpecBuilder()
    with b.if_(1) as s:
        t = s.define_type("t", ib.int_)
        first = s.read("first", t)
        with s.if_(1) as nested:
            nested.read("nested", t)
    with b.if_(1) as s:
        u = s.define_type("t", ib.float_)
        second = s.read("second", u)
    data = b.build().read(b"1 2 2.5")
    assert data[first] == 1
    assert data[second] == 2.5


def test_only_a_store_defines_the_entry_named_underscore() -> None:
    def binding(define: Callable[[ib.SpecBuilder, ib.DynField], object]) -> None:
        def build(b: ib.SpecBuilder) -> None:
            stored = b.forward("_", ib.DynField)
            b.repeat_until("END", store=stored)
            define(b, stored)

        assert "`store`" in str(failure_of(build, "binding"))

    binding(lambda b, stored: b.read(stored, ib.int_))
    binding(lambda b, stored: b.list(stored, ib.int_.until(0)))
    binding(lambda b, stored: b.let(stored, 1))
    binding(lambda b, stored: b.param(stored))
    binding(lambda b, stored: b.line(("", ib.bind(stored, ib.int_))))

    def never(b: ib.SpecBuilder) -> None:
        b.check(ib.defined(b.forward("_", ib.DynField)))

    failure_of(never, "binding")


def test_a_store_of_underscore_reads_as_an_entry_at_the_top_level() -> None:
    b = ib.SpecBuilder()
    stored = b.forward("_", ib.StrField[str])
    b.repeat_until("END", store=stored)
    copy = b.let("s", stored)
    b.line(("", ib.ref(stored)))
    spec = b.build()
    assert b.source() == 'repeat until "END" store _ {}\nlet s = _\nline "{$_}"\n'
    data = spec.read(b"END\nEND")
    assert data[stored] == "END"
    assert data[copy] == "END"

    def an_int(b: ib.SpecBuilder) -> None:
        b.repeat_until("END", store=b.forward("_", ib.IntField[int]))

    failure_of(an_int, "binding")

    b = ib.SpecBuilder()
    maybe = b.forward("_", ib.StrField[str | None])
    b.sections(until="END", store=maybe)
    assert b.build().read(b"END")[maybe] == "END"


def test_a_store_of_underscore_in_a_repeat_leaves_no_entry() -> None:
    def an_array(b: ib.SpecBuilder) -> None:
        stored = b.forward("_", ib.StrField[list[str]])
        with b.repeat(1) as r:
            r.repeat_until("END", store=stored)

    failure_of(an_array, "binding")

    def required(b: ib.SpecBuilder) -> None:
        stored = b.forward("_", ib.StrField[str])
        with b.repeat(1) as r:
            r.repeat_until("END", store=stored)

    assert "| None" in str(failure_of(required, "binding"))

    b = ib.SpecBuilder()
    stored = b.forward("_", ib.DynField)
    with b.repeat(2) as r:
        r.repeat_until("END", store=stored)
        copy = r.let("copy", stored)
    data = b.build().read(b"END END")
    assert "_" not in data
    assert data[copy] == ["END", "END"]

    b = ib.SpecBuilder()
    maybe = b.forward("_", ib.StrField[str | None])
    with b.repeat(1) as r:
        r.repeat_until("END", store=maybe)
    assert b.build().read(b"END")[maybe] is None


def test_underscore_names_records() -> None:
    b = ib.SpecBuilder()
    with b.repeat(1) as r:
        r.as_name("_")
        r.each_name("row")
        r.read("x", ib.int_)
    assert "repeat 1 as _ {" in b.source()
    b.build()

    b = ib.SpecBuilder()
    with b.repeat(1) as r:
        r.each_name("_")
        record, x = r.record, r.read("x", ib.int_)
    b.check(record[x] == 1)
    assert "check _.x == 1" in b.source()
    b.build().read(b"1")

    b = ib.SpecBuilder()
    rows = b.forward_record("_", "rows")
    with b.repeat(1) as r:
        r.record_as(rows)
        r.read("x", ib.int_)
    n = b.let("n", ib.size(rows))
    assert b.build().read(b"1")[n] == 1


def test_a_conditional_and_a_later_definition_are_not_optional() -> None:
    def build(b: ib.SpecBuilder) -> None:
        x = b.forward("x", ib.IntField[int])
        with b.if_(1) as t:
            t.read(x, ib.int_)
        b.let(x, 1)

    builds(build)


def test_a_pattern_binds_a_forward_handle() -> None:
    def word(b: ib.SpecBuilder) -> None:
        x = b.forward("x", ib.StrField[str])
        b.line(("x = ", ib.bind(x, ib.word)))

    builds(word)

    def int_word(b: ib.SpecBuilder) -> None:
        x = b.forward("x", ib.IntField[int])
        b.line(("x = ", ib.bind(x, ib.word)))

    failure_of(int_word, "binding")


def test_an_undefined_name_cannot_be_defined() -> None:
    b = ib.SpecBuilder()
    zz = b.undefined("zz")
    b.check(~ib.defined(zz))
    b.source()
    b.let(zz, 1)
    with pytest.raises(ib.BuildError, match="undefined") as info:
        b.build()
    assert info.value.kind == "binding"

    def stored(b: ib.SpecBuilder) -> None:
        b.repeat_until("END", store=b.undefined("_"))

    assert "undefined" in str(failure_of(stored, "binding"))


def test_a_store_defines_a_forward_handle() -> None:
    def repeat(b: ib.SpecBuilder) -> None:
        end = b.forward("end", ib.StrField[str])
        with b.repeat_until("END", store=end) as r:
            r.read("x", ib.int_)

    builds(repeat)

    def required(b: ib.SpecBuilder) -> None:
        b.sections(until="END", store=b.forward("end", ib.StrField[str]))

    failure_of(required, "binding")

    def maybe(b: ib.SpecBuilder) -> None:
        b.sections(until="END", store=b.forward("end", ib.StrField[str | None]))

    builds(maybe)


def test_a_store_does_not_precede_the_body_of_its_repeat() -> None:
    def in_the_body(b: ib.SpecBuilder) -> None:
        x = b.forward("x", ib.StrField[str])
        with b.repeat_until("END", store=x) as r:
            r.let(x, 1)
            r.read(ib.discard(), ib.int_)

    failure_of(in_the_body, "binding")

    def after(b: ib.SpecBuilder) -> None:
        x = b.forward("x", ib.StrField[str])
        with b.repeat_until("END", store=x) as r:
            r.read(ib.discard(), ib.int_)
        b.read(x, ib.int_)

    builds(after)


def test_a_num_forward_handle_takes_an_int_or_a_float() -> None:
    b = ib.SpecBuilder()
    n = b.forward("n", ib.NumField[int | float])
    b.read(n, ib.int_)
    assert b.build().read(b"7")[n] == 7

    def a_float(b: ib.SpecBuilder) -> None:
        b.let(b.forward("n", ib.NumField[int | float]), 1.5)

    builds(a_float)

    def a_word(b: ib.SpecBuilder) -> None:
        b.read(b.forward("n", ib.NumField[int | float]), ib.word)

    failure_of(a_word, "binding")

    def an_array(b: ib.SpecBuilder) -> None:
        b.read(b.forward("n", ib.NumField[int | float]), ib.int_.dim(2))

    failure_of(an_array, "binding")


def test_a_forward_record_binds_once() -> None:
    def twice(b: ib.SpecBuilder) -> None:
        job = b.forward_record("job", "jobs")
        with b.repeat(1) as r:
            r.record_as(job)
        with b.repeat(1) as r:
            r.record_as(job)

    failure_of(twice, "binding")

    def own(b: ib.SpecBuilder) -> None:
        with b.repeat(1) as r:
            record = r.record
        with b.repeat(1) as r:
            r.record_as(record)

    failure_of(own, "binding")

    line = 0

    def a_second_record(b: ib.SpecBuilder) -> None:
        nonlocal line
        first = b.forward_record("first", "firsts")
        second = b.forward_record("second", "seconds")
        with b.repeat(1) as r:
            r.record_as(first)
            line = sys._getframe().f_lineno + 1
            r.record_as(second)
        b.let("n", ib.size(first))

    error = failure_of(a_second_record, "binding")
    assert "already bound" in str(error)
    assert error.call_site is not None
    assert (error.call_site.filename, error.call_site.line) == (__file__, line)


def test_the_language_decides_the_rest() -> None:
    b = ib.SpecBuilder()
    x = b.undefined("x")
    b.let("y", x)
    b.read(ib.discard(), ib.int_)
    b.build()
    assert "let y = x" in b.source()

    b = ib.SpecBuilder()
    b.instances()
    b.read("after", ib.int_)
    with pytest.raises(ib.BuildError) as info:
        b.build()
    error = info.value
    assert error.kind == "spec"
    assert error.error is not None
    assert error.text is not None and "after: int" in error.text


def test_forward_rejects_a_kind_without_a_view() -> None:
    b = ib.SpecBuilder()
    with pytest.raises(TypeError, match="handle type with its view"):
        b.forward("x", ib.IntField)
    with pytest.raises(TypeError, match="does not match"):
        b.forward("x", ib.IntField[str])
    with pytest.raises(TypeError, match="does not match"):
        b.forward("x", ib.NumField[int])
    with pytest.raises(TypeError, match="handle type with its view"):
        b.forward("x", cast(Any, int))


def test_a_bool_is_not_a_condition() -> None:
    b = ib.SpecBuilder()
    x = b.read("x", ib.int_)
    with pytest.raises(TypeError, match="not a bool"):
        b.check(x == "text")
    with pytest.raises(TypeError, match="not a bool"):
        b.if_(True)
    with pytest.raises(TypeError, match="no truth value"):
        if x > 0:
            pass
    with pytest.raises(TypeError, match="no truth value"):
        0 < x < 3  # noqa: B015


def test_python_division_points_to_the_iris_functions() -> None:
    b = ib.SpecBuilder()
    x: Any = b.read("x", ib.int_)
    with pytest.raises(TypeError, match=r"ib\.div"):
        x / 2
    with pytest.raises(TypeError, match=r"ib\.div"):
        2 // x
    with pytest.raises(TypeError, match=r"ib\.rem"):
        x % 2
    with pytest.raises(TypeError, match=r"ib\.pow"):
        x**2


def test_operands_have_the_types_their_operators_take() -> None:
    b = ib.SpecBuilder()
    # Any lets each misuse reach the runtime check that rejects it.
    x: Any = b.read("x", ib.int_)
    s: Any = b.read("s", ib.word)
    f: Any = b.read("f", ib.float_)
    with pytest.raises(TypeError):
        x + s
    with pytest.raises(TypeError):
        f + 1
    with pytest.raises(TypeError):
        _ = s < "a"
    with pytest.raises(TypeError):
        ib.lit(True)
    with pytest.raises(OverflowError):
        ib.lit(2**63)
    with pytest.raises(TypeError, match="not iterable"):
        list(x)
    with pytest.raises(TypeError, match="formatted"):
        f"{x}"
    assert isinstance(f < 1.5, ib.IntExpr)
    assert isinstance(s == "a", ib.IntExpr)
    assert isinstance(s.dynamic() + 1, ib.IntExpr)


def test_a_check_raises_where_it_fails_and_build_raises_it_again() -> None:
    b = ib.SpecBuilder()
    b.read("x", ib.int_)
    with pytest.raises(ib.BuildError) as first:
        b.read("x", ib.int_)
    with pytest.raises(ib.BuildError, match="already declared"):
        b.forward("x", ib.IntField[int])
    with pytest.raises(ib.BuildError) as again:
        b.source()
    assert again.value is first.value
