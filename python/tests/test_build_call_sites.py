"""Errors point at the host code: every statement records the line that called
it, and failures carry that line beside their location in the emitted text."""

import pickle
import sys
from collections.abc import Callable
from typing import Any, cast

import pytest

import gecode_iris.build as ib
from gecode_iris import CallSite, Error, Spec

FILE = __file__


def here() -> int:
    """The line of the caller."""
    return sys._getframe(1).f_lineno


def assert_here(site: CallSite | None, line: int) -> None:
    assert site is not None, "the failure has a call site"
    assert (site.filename, site.line) == (FILE, line)


def build_error(b: ib.SpecBuilder) -> ib.BuildError:
    with pytest.raises(ib.BuildError) as info:
        b.build()
    return info.value


def spec_error(error: ib.BuildError) -> Error:
    assert error.error is not None, "the emitted text is rejected"
    return error.error


def read_error(spec: Spec, data: bytes, origin: str = "input.txt") -> Error:
    with pytest.raises(Error) as info:
        spec.read(data, origin=origin)
    return info.value


def test_a_parse_error_names_the_statement_after_instances() -> None:
    b = ib.SpecBuilder()
    b.instances()
    line = here() + 1
    b.read("after", ib.int_)
    error = build_error(b)

    assert_here(error.call_site, line)
    assert_here(spec_error(error).call_site, line)
    assert spec_error(error).origin == "builder.iris"
    assert spec_error(error).line == 2
    assert error.emitted_line == "after: int"
    assert error.text == "instances {}\nafter: int\n"
    assert error.__cause__ is error.error


def test_a_resolution_error_names_the_statement_that_uses_the_name() -> None:
    b = ib.SpecBuilder()
    outside = b.read("outside", ib.int_)
    with b.repeat(2) as s:
        s.read("inside", ib.int_)
        record = s.record
    line = here() + 1
    b.let("bad", record[outside])
    error = build_error(b)

    assert spec_error(error).code == "E-SPEC-RECORD-SCOPE"
    assert_here(error.call_site, line)
    assert spec_error(error).line == 6
    assert error.emitted_line == "let bad = record_1.outside"


def test_the_display_shows_the_code_message_emitted_line_and_call_site() -> None:
    b = ib.SpecBuilder()
    b.instances()
    line = here() + 1
    b.read("after", ib.int_)
    error = build_error(b)

    message = spec_error(error).message
    code = spec_error(error).code
    assert str(error) == (
        f"{code}: builder.iris:2: {message}\n     2 | after: int\ncall site: {FILE}:{line}"
    )


def test_a_failed_check_names_the_check_in_its_block() -> None:
    b = ib.SpecBuilder("row")
    n = b.read("n", ib.int_)
    with b.repeat(n) as s:
        x = s.read("x", ib.int_)
        line = here() + 1
        s.check(x > 0, "not positive")
    error = read_error(b.build(), b"2 1 -1")

    assert error.code == "E-CHECK-FAILED"
    assert_here(error.call_site, line)
    assert "\nspec: builder.iris:5" in str(error)
    assert str(error).endswith(f"\ncall site: {FILE}:{line}")


def test_a_read_failure_names_the_declaration_that_read() -> None:
    b = ib.SpecBuilder()
    line = here() + 1
    b.read("n", ib.int_)
    assert_here(read_error(b.build(), b"word").call_site, line)


def test_a_catalog_failure_names_the_statement() -> None:
    b = ib.SpecBuilder()
    line = here() + 1
    b.read("n", ib.int_)
    b.instances()
    with pytest.raises(Error) as info:
        b.build().catalog(b"word", origin="input.txt")
    assert_here(info.value.call_site, line)


def test_a_deferred_rejection_keeps_its_call_site() -> None:
    b = ib.SpecBuilder()
    with b.instances() as s:
        n = s.read("n", ib.int_)
        line = here() + 1
        s.check(n > 0)
    error = read_error(b.build(), b"0")

    assert error.code == "E-CHECK-FAILED"
    assert_here(error.call_site, line)


def test_a_failure_without_a_statement_has_no_call_site() -> None:
    b = ib.SpecBuilder()
    b.read("n", ib.int_)
    error = read_error(b.build(), b"1 2")

    assert error.code == "E-TRAILING-INPUT"
    assert error.call_site is None
    assert "call site" not in str(error)


def test_a_spec_parsed_from_text_has_no_call_sites() -> None:
    """The same text, parsed and built, differs only in the call site."""
    b = ib.SpecBuilder()
    n = b.read("n", ib.int_)
    b.check(n > 0, "not positive")
    built = b.build()
    parsed = Spec.parse(b.source(), "builder.iris")

    from_text, from_builder = read_error(parsed, b"0"), read_error(built, b"0")
    assert from_text.call_site is None
    assert (from_text.code, from_text.message, from_text.line, from_text.column) == (
        from_builder.code,
        from_builder.message,
        from_builder.line,
        from_builder.column,
    )
    assert from_text.to_json() == from_builder.to_json()
    assert from_builder.call_site is not None
    assert f"{from_text}\ncall site: {from_builder.call_site}" == str(from_builder)

    with pytest.raises(Error) as info:
        Spec.parse("instances {}\nafter: int\n", "builder.iris")
    assert info.value.call_site is None
    assert "call site" not in str(info.value)


def test_a_call_site_survives_pickling() -> None:
    b = ib.SpecBuilder()
    line = here() + 1
    b.read("n", ib.int_)
    error = read_error(b.build(), b"word")
    copy = pickle.loads(pickle.dumps(error))
    assert copy.call_site == error.call_site
    assert_here(copy.call_site, line)
    assert str(copy) == str(error)
    assert isinstance(copy.call_site, CallSite)


def test_a_builder_check_names_the_statement_that_failed_it() -> None:
    b = ib.SpecBuilder()
    b.read("x", ib.int_)
    with pytest.raises(ib.BuildError) as info:
        line = here() + 1
        b.read("x", ib.int_)
    error = info.value
    assert error.kind == "redefinition"
    assert_here(error.call_site, line)
    assert str(error).endswith(f":{line}")
    assert error.call_site is not None and error.call_site.function == (
        "test_a_builder_check_names_the_statement_that_failed_it"
    )


def test_a_handle_from_another_builder_names_the_statement_that_used_it() -> None:
    other = ib.SpecBuilder()
    foreign = other.read("x", ib.int_)
    b = ib.SpecBuilder()
    with pytest.raises(ib.BuildError) as info:
        line = here() + 1
        b.let("y", foreign)
    assert info.value.kind == "provenance"
    assert_here(info.value.call_site, line)


def test_a_handle_never_bound_names_the_call_that_made_it() -> None:
    b = ib.SpecBuilder()
    line = here() + 1
    b.forward("later", ib.IntField[int])
    error = build_error(b)
    assert error.kind == "binding"
    assert_here(error.call_site, line)

    b = ib.SpecBuilder()
    line = here() + 1
    b.capture("n", ib.int_)
    assert_here(build_error(b).call_site, line)

    b = ib.SpecBuilder()
    line = here() + 1
    b.forward_record("each", "list")
    assert_here(build_error(b).call_site, line)


def test_a_binding_failure_names_the_definition() -> None:
    b = ib.SpecBuilder()
    name = b.undefined("name")
    line = here() + 1
    b.read(name, ib.int_)
    error = build_error(b)
    assert error.kind == "binding"
    assert_here(error.call_site, line)


def test_an_unprintable_value_names_the_statement_that_holds_it() -> None:
    b = ib.SpecBuilder()
    b.read("x", ib.int_)
    line = here() + 1
    b.let("y", float("nan"))
    error = build_error(b)
    assert error.kind == "unprintable"
    assert_here(error.call_site, line)


def test_a_guard_used_late_names_the_guard_call() -> None:
    b = ib.SpecBuilder()
    guard = b.sections()
    b.read("m", ib.int_)
    with pytest.raises(ib.BuildError) as info:
        line = here() + 1
        guard.until("late")
    assert info.value.kind == "scope"
    assert_here(info.value.call_site, line)

    b = ib.SpecBuilder()
    repeat = b.repeat(1)
    b.read("m", ib.int_)
    line = here() + 1
    with pytest.raises(ib.BuildError) as info, repeat:
        pass
    assert_here(info.value.call_site, line)


def without_tracking(s: ib.Scope0[object]) -> int:
    line = here() + 1
    s.read("x", ib.int_)
    return line


@ib.track_caller
def with_tracking(s: ib.Scope0[object]) -> None:
    s.read("x", ib.int_)


def test_a_helper_records_its_own_line_unless_it_tracks_its_caller() -> None:
    b = ib.SpecBuilder()
    line = without_tracking(b)
    assert_here(read_error(b.build(), b"word", "i").call_site, line)

    b = ib.SpecBuilder()
    line = here() + 1
    with_tracking(b)
    assert_here(read_error(b.build(), b"word", "i").call_site, line)


def test_track_caller_takes_only_functions() -> None:
    with pytest.raises(TypeError):
        ib.track_caller(cast(Any, len))


def test_a_method_used_as_a_function_value_records_its_caller() -> None:
    """The call site comes from the stack, so a method value records the line
    that called it."""
    b = ib.SpecBuilder()
    skip: Callable[[ib.Scope[object]], None] = ib.Scope.skip_line
    line = here() + 1
    skip(b)
    b.read("n", ib.int_)
    error = read_error(b.build(), b"")
    assert_here(error.call_site, line)


class Expected:
    """What each statement's text starts with and the line of its call."""

    def __init__(self) -> None:
        self.starts: list[tuple[str, int]] = []

    def at(self, start: str, below: int = 0) -> None:
        """Expect a statement starting with ``start`` from the caller's line,
        or from the line ``below`` lines under it."""
        self.starts.append((start, sys._getframe(1).f_lineno + below))


def assert_sites(b: ib.SpecBuilder, expected: Expected) -> None:
    """Check each statement's call site, in text order, through a failure of
    a spec whose text is cut off after it."""
    text = b.source()
    sites = b.build()._call_sites
    cursor = 0
    data = text.encode()
    for start, line in expected.starts:
        offset = data.find(start.encode(), cursor)
        assert offset >= 0, f"no `{start}` after byte {cursor} of:\n{text}"
        site = sites.at(offset)
        assert (start, site.filename, site.line) == (start, FILE, line), text
        cursor = offset + len(start.encode())


# fmt: off
def test_every_setting_and_plain_statement_records_its_call() -> None:
    e = Expected()
    e.at("format first"); b = ib.SpecBuilder("first")
    e.at("format second"); b.format("second")
    e.at("comment"); b.comment("#")
    e.at("stop"); b.stop("END")
    e.at("tokens"); b.tokens(":")
    e.at("separators"); b.separators(",")
    e.at("ignore trailing"); b.ignore_trailing()
    e.at("param p"); b.param("p")
    e.at("param q"); b.param("q", 1)
    e.at("type t"); b.define_type("t", ib.int_)
    e.at("n: int"); n = b.read("n", ib.int_)
    e.at("list l"); b.list("l", ib.int_.until(0))
    e.at("let v"); v = b.let("v", 1)
    e.at("let v"); b.set(v, 2)
    e.at("check"); b.check(n >= 0)
    e.at("expect"); b.expect("word")
    e.at("skip line"); b.skip_line()
    e.at("skip 1 lines"); b.skip_lines(1)
    m = b.capture("m", ib.word)
    e.at("line"); b.line(("m ", m))
    e.at("find"); b.find("x")
    e.at("index"); b.index(0)
    assert_sites(b, e)


def test_every_block_statement_records_its_call() -> None:
    e = Expected()
    b = ib.SpecBuilder()
    e.at("n: int"); n = b.read("n", ib.int_)
    e.at("if n", below=1)
    with b.if_(n > 0) as s:
        e.at("a: int"); s.read("a", ib.int_)
    e.at("else if n", below=1)
    with b.else_if(n < 0) as s:
        e.at("b: int"); s.read("b", ib.int_)
    e.at("else", below=1)
    with b.else_() as s:
        e.at("c: int"); s.read("c", ib.int_)
    e.at("repeat 2", below=1)
    with b.repeat(2) as s:
        e.at("d: int"); s.read("d", ib.int_)
    e.at('repeat until "END"', below=1)
    with b.repeat_until("END") as s:
        e.at("f: int"); s.read("f", ib.int_)
    e.at("sections", below=1)
    with b.sections() as arms:
        e.at('"one"', below=1)
        with arms.arm("one") as s:
            e.at("h: int"); s.read("h", ib.int_)
    e.at("repeat until eof", below=1)
    with b.repeat_until_eof() as s:
        e.at("i: float"); s.read("i", ib.float_)
    e.at("instances", below=1)
    with b.instances() as s:
        e.at("j: int"); s.read("j", ib.int_)
    assert_sites(b, e)


def test_the_guards_of_if_record_their_calls() -> None:
    e = Expected()
    b = ib.SpecBuilder()
    n = b.read("n", ib.int_)
    b.if_(n > 0)
    e.at("else if n"); b.else_if(n < 0)
    e.at("else if:"); b.else_read_if("if", ib.int_)
    assert_sites(b, e)
# fmt: on


def test_a_closing_brace_belongs_to_the_call_that_wrote_the_block() -> None:
    b = ib.SpecBuilder()
    line = here() + 1
    with b.repeat(2) as s:
        s.read("x", ib.int_)
    b.read("y", ib.int_)
    sites = b.build()._call_sites
    assert_here(sites.at(b.source().find("}")), line)


def test_an_empty_block_gives_its_brace_to_the_call_that_wrote_it() -> None:
    b = ib.SpecBuilder()
    guard = b.repeat_until("END")
    line = here() + 1
    with guard:
        pass
    text = b.source()
    assert text == 'repeat until "END" {}\n'
    assert_here(b.build()._call_sites.at(text.find("}")), line)

    b = ib.SpecBuilder()
    line = here() + 1
    b.repeat_until("END")
    assert_here(b.build()._call_sites.at(b.source().find("}")), line)


def test_call_sites_count_bytes_of_the_text() -> None:
    b = ib.SpecBuilder()
    b.line("ünïcödé ✓")
    line = here() + 1
    b.read("n", ib.int_)
    assert_here(read_error(b.build(), "ünïcödé ✓\nword".encode()).call_site, line)


def assert_owner(error: ib.BuildError, line: int, emitted_line: int, emitted: str | None) -> None:
    """A declaration's form is checked at the token after it, which may start
    another statement. The location stays there; the call site does not."""
    spec = spec_error(error)
    assert spec.code == "E-SPEC-TYPE"
    assert spec.message == "'rest' values cannot be arrays"
    assert_here(error.call_site, line)
    assert_here(spec.call_site, line)
    assert spec.line == emitted_line
    assert error.emitted_line == emitted
    assert error.text is not None
    with pytest.raises(Error) as info:
        Spec.parse(error.text, "builder.iris")
    parsed = info.value
    assert parsed.call_site is None
    assert (parsed.code, parsed.message, parsed.line) == (spec.code, spec.message, spec.line)


def test_an_error_on_the_next_statement_names_the_statement_it_belongs_to() -> None:
    b = ib.SpecBuilder()
    line = here() + 1
    b.read("bad", ib.rest.dim(2))
    b.read("next", ib.int_)
    assert_owner(build_error(b), line, 2, "next: int")


def test_an_error_at_the_end_of_a_nested_block_names_the_statement_in_it() -> None:
    b = ib.SpecBuilder()
    with b.repeat(1) as s, s.repeat(1) as t:
        line = here() + 1
        t.read("bad", ib.rest.dim(2))
    assert_owner(build_error(b), line, 4, "  }")


def test_an_error_at_the_end_of_an_else_block_names_the_statement_in_it() -> None:
    b = ib.SpecBuilder()
    b.if_(ib.lit(1) == 1)
    with b.else_() as s:
        line = here() + 1
        s.read("bad", ib.rest.dim(2))
    assert_owner(build_error(b), line, 3, "}")


def test_an_error_at_the_end_of_the_spec_names_the_last_statement() -> None:
    b = ib.SpecBuilder()
    b.read("n", ib.int_)
    line = here() + 1
    b.read("bad", ib.rest.dim(2))
    assert_owner(build_error(b), line, 3, None)


def test_an_error_after_a_brace_names_the_block_that_opened_it() -> None:
    lines: dict[str, int] = {}

    def nest(s: Any, depth: int) -> None:
        if depth == 0:
            lines["read"] = here() + 1
            s.read("x", ib.int_)
        else:
            lines["block"] = here() + 1
            with s.if_(ib.lit(1) == 1) as inner:
                nest(inner, depth - 1)

    b = ib.SpecBuilder()
    with b.if_(ib.lit(1) == 1) as s:
        nest(s, 256)
    error = build_error(b)

    assert spec_error(error).code == "E-SPEC-NESTING"
    assert error.emitted_line is not None and error.emitted_line.strip() == "x: int"
    assert_here(error.call_site, lines["block"])


def test_an_error_in_an_arm_names_the_arm() -> None:
    b = ib.SpecBuilder()
    cap = b.capture("bad", ib.alphabet("ab"))
    with b.sections() as arms:
        line = here() + 1
        arms.arm((cap,))
    error = build_error(b)
    assert spec_error(error).code == "E-SPEC-PATTERN"
    assert_here(error.call_site, line)


def test_a_resolution_error_in_an_arm_names_the_arm() -> None:
    b = ib.SpecBuilder()
    outside = b.read("outside", ib.int_)
    with b.repeat(2) as s:
        s.each_name("row")
        s.read("inside", ib.int_)
        record = s.record
    cap = b.capture("cap", ib.int_range(0, below=record[outside]))
    with b.sections() as arms:
        line = here() + 1
        arms.arm((cap,))
    error = build_error(b)

    assert spec_error(error).code == "E-SPEC-RECORD-SCOPE"
    assert error.emitted_line == "sections {"
    assert_here(error.call_site, line)


def test_a_store_failure_names_the_statement_that_stores() -> None:
    b = ib.SpecBuilder()
    stored = b.forward("stored", ib.IntField[int])
    line = here() + 1
    with b.repeat_until("END", store=stored) as s:
        s.read("x", ib.int_)
    error = build_error(b)
    assert error.kind == "binding"
    assert_here(error.call_site, line)


def test_a_repeat_that_reads_nothing_names_the_repeat() -> None:
    b = ib.SpecBuilder()
    line = here() + 1
    with b.repeat_until_eof() as s:
        s.let("x", 1)
    error = read_error(b.build(), b"1")

    assert error.code == "E-NO-PROGRESS"
    assert "\nspec: builder.iris:2" in str(error)
    assert_here(error.call_site, line)


def test_a_column_failure_names_the_repeat() -> None:
    b = ib.SpecBuilder()
    line = here() + 1
    with b.repeat(2) as s:
        x = s.read("x", ib.int_)
        with s.if_(x > 0) as t:
            t.read("y", ib.int_)
        s.read("z", ib.int_)
    error = read_error(b.build(), b"1 2 3 0 4")

    assert error.code == "E-REPEAT-MISSING"
    assert "\nspec: builder.iris:6" in str(error)
    assert_here(error.call_site, line)


def test_a_read_failure_in_an_arm_names_the_arm() -> None:
    b = ib.SpecBuilder()
    n = b.capture("n", ib.int_range(1, 2))
    with b.sections() as arms:
        line = here() + 1
        arms.arm(("n = ", n))
    error = read_error(b.build(), b"n = 3\n")

    assert error.code == "E-EXPECTED-VALUE"
    assert "\nspec: builder.iris:1" in str(error)
    assert_here(error.call_site, line)


def test_a_line_a_last_arm_left_unread_names_its_arm() -> None:
    b = ib.SpecBuilder()
    with b.sections() as arms:
        line = here() + 1
        arms.arm("needed")
        with arms.arm("end", last=True) as s:
            s.read("x", ib.int_)
    error = read_error(b.build(), b"end\n1\n")

    assert error.code == "E-LINE-MATCH"
    assert "\nspec: builder.iris:4" in str(error)
    assert_here(error.call_site, line)


def test_a_line_no_arm_matches_names_the_sections() -> None:
    b = ib.SpecBuilder()
    line = here() + 1
    sections = b.sections()
    with sections as arms:
        arms.arm("known")
    error = read_error(b.build(), b"other\n")

    assert error.code == "E-SECTIONS-UNEXPECTED"
    assert_here(error.call_site, line)


def test_a_failure_after_sections_end_has_no_call_site() -> None:
    b = ib.SpecBuilder()
    with b.sections() as arms:
        arms.arm("end", last=True)
    error = read_error(b.build(), b"end\nmore")

    assert error.code == "E-TRAILING-INPUT"
    assert error.call_site is None
