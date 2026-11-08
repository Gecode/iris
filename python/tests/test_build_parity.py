"""Every test of the Rust spec builder has a Python counterpart, so that the
two builders keep the same checks."""

import re
from pathlib import Path

TESTS = Path(__file__).parent
RUST_TESTS = TESTS.parent.parent / "rust" / "iris-build" / "tests"

# A Rust test whose Python counterpart has another name, and why.
RENAMED = {
    "a_method_used_as_a_function_value_records_its_definition": (
        "a_method_used_as_a_function_value_records_its_caller",
        "Python finds a call site on the stack, so a method value records its caller",
    ),
    "every_public_method_of_a_scope_tracks_its_caller": (
        "every_setting_and_plain_statement_records_its_call",
        "Python finds a call site on the stack and needs no attribute per method",
    ),
    "a_million_term_chain_prints_and_drops": (
        "a_long_chain_prints",
        "Python has nothing to drop, and a shorter chain keeps the suite fast",
    ),
    "three_hundred_dimensions_read_through_a_value_ref": (
        "three_hundred_dimensions_read",
        "Python reads nested lists, not value references",
    ),
    "get_panics_with_a_pointer_to_try_get": (
        "a_handle_of_another_spec_reports_what_went_wrong",
        "Python raises KeyError or TypeError instead of panicking",
    ),
}


def rust_tests() -> list[str]:
    names: list[str] = []
    for path in sorted(RUST_TESTS.glob("*.rs")):
        text = path.read_text(encoding="utf-8")
        names += re.findall(r"#\[test\]\s*(?:#\[[^\]]*\]\s*)*fn (\w+)", text)
    return names


def python_tests() -> set[str]:
    names: set[str] = set()
    for path in TESTS.glob("test_build_*.py"):
        names.update(re.findall(r"^def test_(\w+)", path.read_text(encoding="utf-8"), re.MULTILINE))
    return names


def test_every_rust_builder_test_has_a_python_counterpart() -> None:
    rust = rust_tests()
    assert len(rust) >= 90, f"only {len(rust)} Rust tests were found"
    python = python_tests()
    missing = [name for name in rust if RENAMED.get(name, (name,))[0] not in python]
    assert not missing, f"Rust builder tests without a Python counterpart: {missing}"
    stale = [name for name in RENAMED if name not in rust]
    assert not stale, f"renamed tests that Rust no longer has: {stale}"
