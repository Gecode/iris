// Tests of the spec builder: its own checks and the call sites they name,
// the text it prints, and the data read through its handles. The call
// sites of language errors are in build_call_sites_test.cpp.

#include <iris/build.hpp>
#include <iris/iris.hpp>

#include <algorithm>
#include <bit>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <exception>
#include <expected>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <iterator>
#include <limits>
#include <optional>
#include <regex>
#include <source_location>
#include <span>
#include <string>
#include <string_view>
#include <thread>
#include <tuple>
#include <utility>
#include <variant>
#include <vector>

namespace {

using namespace iris::build;
using iris::DynField;
using iris::Field;
using iris::Number;
using iris::Presence;
using iris::Spec;
// <cmath> declares ::pow, which the using-directive would make ambiguous.
using iris::build::pow;
using Line = std::uint_least32_t;
using Int = std::int64_t;
using Str = std::string;

int failures = 0;

void check(bool condition, std::string_view what,
           std::source_location where = std::source_location::current()) {
  if (condition)
    return;
  failures++;
  std::cerr << where.file_name() << ':' << where.line()
            << ": check failed: " << what << '\n';
}

/// Return the column of the call site of `error`, or 0 if it has none
std::uint_least32_t column_of(const BuildError &error) {
  const std::optional<std::source_location> &site = error.call_site();
  return site ? site->column() : 0;
}

/// Return the line of the call
Line here(std::source_location where = std::source_location::current()) {
  return where.line();
}

/// Whether `site` is line `line` of this file
bool at_line(const std::optional<std::source_location> &site, Line line) {
  return site &&
         std::string_view(site->file_name()) ==
             std::source_location::current().file_name() &&
         site->line() == line;
}

std::string_view kind_name(BuildErrorKind kind) {
  switch (kind) {
  case BuildErrorKind::Scope:
    return "Scope";
  case BuildErrorKind::Provenance:
    return "Provenance";
  case BuildErrorKind::Redefinition:
    return "Redefinition";
  case BuildErrorKind::Unprintable:
    return "Unprintable";
  case BuildErrorKind::Binding:
    return "Binding";
  case BuildErrorKind::Spec:
    return "Spec";
  }
  return "?";
}

/// Check that building `b` fails with an error of kind `kind` whose call
/// site is line `line` of this file and whose text contains `fragment`
std::optional<BuildError>
expect_failure(const SpecBuilder &b, BuildErrorKind kind, Line line,
               std::string_view fragment = {},
               std::source_location where = std::source_location::current()) {
  std::expected<Spec, BuildError> built = b.try_build();
  if (built) {
    check(false, "the builder should fail", where);
    return std::nullopt;
  }
  const BuildError &error = built.error();
  const std::string text = error.what();
  check(error.kind() == kind,
        "kind " + std::string(kind_name(error.kind())) + ", not " +
            std::string(kind_name(kind)) + ": " + text,
        where);
  check(at_line(error.call_site(), line),
        "call site on line " + std::to_string(line) + ": " + text, where);
  check(text.contains(fragment),
        "the error mentions '" + std::string(fragment) + "': " + text, where);
  return error;
}

/// Check that building `b` succeeds, and return the spec
std::optional<Spec>
expect_build(const SpecBuilder &b,
             std::source_location where = std::source_location::current()) {
  std::expected<Spec, BuildError> built = b.try_build();
  if (!built) {
    check(false, std::string("the builder fails: ") + built.error().what(),
          where);
    return std::nullopt;
  }
  return std::move(*built);
}

/// Build `b` and read `input` with it
std::optional<iris::Data>
read(const SpecBuilder &b, std::string_view input,
     std::source_location where = std::source_location::current()) {
  const std::optional<Spec> spec = expect_build(b, where);
  if (!spec)
    return std::nullopt;
  std::expected<iris::Data, iris::Error> data =
      spec->try_read_text(input, "input");
  if (!data) {
    check(false, std::string("the read fails: ") + data.error().what(), where);
    return std::nullopt;
  }
  return std::move(*data);
}

/// Check that `b` prints `expected`, and that the text builds unless
/// `builds` is false
void expect_text(const SpecBuilder &b, std::string_view expected,
                 bool builds = true,
                 std::source_location where = std::source_location::current()) {
  std::expected<std::string, BuildError> text = b.try_source();
  if (!text) {
    check(false, std::string("no text: ") + text.error().what(), where);
    return;
  }
  check(*text == expected,
        "the text is\n" + *text + "\nnot\n" + std::string(expected), where);
  if (builds)
    expect_build(b, where);
}

// Scope discipline

void a_statement_through_an_outer_scope_is_rejected() {
  SpecBuilder b;
  Line line = 0;
  b.instances([&](auto &s) {
    auto n = s.capture("n", int_);
    s.sections([&](auto &arms) {
      arms.arm(pattern("{}", n), [&](auto & /*body*/) {
        line = here() + 1;
        s.read("x", int_);
      });
    });
  });
  expect_failure(b, BuildErrorKind::Scope, line, "innermost");
}

void an_arm_through_an_outer_block_is_rejected() {
  SpecBuilder b;
  Line line = 0;
  b.sections([&](auto &arms) {
    arms.arm("a", [&](auto & /*body*/) {
      line = here() + 1;
      arms.arm("b");
    });
  });
  expect_failure(b, BuildErrorKind::Scope, line);
}

void a_branch_continued_after_later_statements_is_rejected() {
  {
    SpecBuilder b;
    auto n = b.read("n", int_);
    auto branch = b.if_(n, [](auto & /*then*/) {});
    b.read("y", int_);
    const Line line = here() + 1;
    branch.else_([](auto & /*otherwise*/) {});
    expect_failure(b, BuildErrorKind::Scope, line, "continue an `if`");
  }
  {
    SpecBuilder b;
    auto n = b.read("n", int_);
    auto branch = b.if_(n, [](auto & /*then*/) {});
    b.read("y", int_);
    const Line line = here() + 1;
    branch.else_if(n > 0, [](auto & /*then*/) {});
    expect_failure(b, BuildErrorKind::Scope, line);
  }
  {
    SpecBuilder b;
    auto n = b.read("n", int_);
    auto branch = b.if_(n, [](auto & /*then*/) {});
    b.read("y", int_);
    const Line line = here() + 1;
    branch.else_read_if("if", int_);
    expect_failure(b, BuildErrorKind::Scope, line);
  }
  {
    // Each continuation follows the one before.
    SpecBuilder b;
    auto n = b.read("n", int_);
    auto branch = b.if_(n > 1, [](auto & /*then*/) {});
    branch.else_if(n > 0, [](auto & /*then*/) {});
    branch.else_([](auto & /*otherwise*/) {});
    expect_text(b, "n: int\nif n > 1 {} else if n > 0 {} else {}\n");
  }
}

void a_scope_used_after_its_block_is_rejected() {
  SpecBuilder b;
  std::optional<Scope<1>> escaped;
  b.repeat(1, [&](Scope<1> &s) { escaped = s; });
  if (!escaped) {
    check(false, "the repeat ran its body");
    return;
  }
  const Line line = here() + 1;
  escaped->read("x", int_);
  expect_failure(b, BuildErrorKind::Scope, line);
}

// Provenance

void a_handle_from_another_builder_is_rejected() {
  SpecBuilder other;
  auto foreign = other.read("x", int_);
  {
    SpecBuilder b;
    const Line line = here() + 1;
    b.let("y", foreign + 1);
    expect_failure(b, BuildErrorKind::Provenance, line,
                   "does not come from this builder");
  }
  {
    SpecBuilder b;
    const Line line = here() + 1;
    b.read(foreign, int_);
    expect_failure(b, BuildErrorKind::Provenance, line);
  }
  {
    SpecBuilder b;
    const Line line = here() + 1;
    b.line(pattern("{}", foreign));
    expect_failure(b, BuildErrorKind::Provenance, line);
  }
  Record record = other.repeat(1, [](auto &r) { return r.record(); });
  {
    SpecBuilder b;
    const Line line = here() + 1;
    b.check(record.list()[0].get(foreign));
    expect_failure(b, BuildErrorKind::Provenance, line);
  }
  auto named = other.define_type("t", int_);
  {
    SpecBuilder b;
    const Line line = here() + 1;
    b.read("x", named);
    expect_failure(b, BuildErrorKind::Provenance, line);
  }
  {
    // The capture's own statement holds the type.
    SpecBuilder b;
    const Line line = here() + 1;
    auto x = b.capture("x", named);
    b.line(pattern("{}", x));
    expect_failure(b, BuildErrorKind::Provenance, line, "`t`");
  }
}

void a_declared_handle_reads_but_does_not_build() {
  static constexpr Field<Int> declared("x");
  SpecBuilder b;
  const Line line = here() + 1;
  b.let("y", declared);
  expect_failure(b, BuildErrorKind::Provenance, line, "`x`");
}

// Redefinition

void defining_a_name_twice_needs_the_handle() {
  {
    SpecBuilder b;
    b.read("x", int_);
    const Line line = here() + 1;
    b.let("x", lit(1));
    expect_failure(b, BuildErrorKind::Redefinition, line, "`x`");
  }
  {
    SpecBuilder b;
    auto x = b.read("x", int_);
    b.set(x, lit(2));
    b.set(x, 3);
    DynField again = b.let(x, lit(1.5));
    const std::optional<iris::Data> data = read(b, "1");
    if (data) {
      const std::optional<iris::ValueRef> value = data->get(again);
      check(value && value->number() == 1.5, "the redefined value");
    }
  }
  {
    SpecBuilder b;
    b.read("x", int_);
    const Line line = here() + 1;
    b.read("x", int_);
    std::optional<BuildError> error =
        expect_failure(b, BuildErrorKind::Redefinition, line);
    if (error) {
      const std::string text = error->what();
      check(text.contains("\ncall site: "), "the display names the call");
      check(text.ends_with(":" + std::to_string(line) + ":" +
                           std::to_string(column_of(*error))),
            "the display ends with the line and column");
      check(!error->language_error() && !error->text() &&
                !error->emitted_line(),
            "a builder check has no language error or text");
    }
  }
}

// Unprintable values

/// Check that the statements of `build` have no spelling: `build` returns
/// the line of the statement at fault
template <class F>
void unprintable(F build, std::string_view fragment = {},
                 std::source_location where = std::source_location::current()) {
  SpecBuilder b;
  const Line line = build(b);
  expect_failure(b, BuildErrorKind::Unprintable, line, fragment, where);
}

void values_with_no_spelling_are_rejected() {
  unprintable([](SpecBuilder &b) {
    const Line line = here() + 1;
    b.line(Pattern().text("a { b"));
    return line;
  });
  unprintable([](SpecBuilder &b) {
    const Line line = here() + 1;
    b.line(Pattern().text("a ... b"));
    return line;
  });
  unprintable([](SpecBuilder &b) {
    const Line line = here() + 1;
    b.read("format", int_);
    return line;
  });
  unprintable([](SpecBuilder &b) {
    const Line line = here() + 1;
    b.read("_", int_);
    return line;
  });
  unprintable([](SpecBuilder &b) {
    const Line line = here() + 1;
    b.list("_", int_.until(0));
    return line;
  });
  unprintable([](SpecBuilder &b) {
    const Line line = here() + 1;
    b.let("_", lit(1));
    return line;
  });
  unprintable([](SpecBuilder &b) {
    const Line line = here() + 1;
    b.param("_");
    return line;
  });
  unprintable([](SpecBuilder &b) {
    const Line line = here() + 1;
    b.define_type("_", int_);
    return line;
  });
  unprintable([](SpecBuilder &b) {
    const Line line = here() + 1;
    auto x = b.capture("_", int_);
    b.line(pattern("{}", x));
    return line;
  });
  unprintable([](SpecBuilder &b) {
    const Line line = here() + 1;
    auto x = b.optional_capture("_", int_);
    b.line(pattern("{}", x));
    return line;
  });
  unprintable([](SpecBuilder &b) {
    const Line line = here() + 1;
    b.repeat_until("x", {.store = "_"}, [](auto & /*body*/) {});
    return line;
  });
  unprintable([](SpecBuilder &b) {
    const Line line = here() + 1;
    b.repeat_until("x", {.store = discard}, [](auto & /*body*/) {});
    return line;
  });
  unprintable([](SpecBuilder &b) {
    const Line line = here() + 1;
    b.sections({.until = "x", .store = discard}, [](auto & /*arms*/) {});
    return line;
  });
  unprintable([](SpecBuilder &b) {
    const Line line = here() + 1;
    b.sections({.store = "end", .skip_other = true}, [](auto & /*arms*/) {});
    return line;
  });
  unprintable([](SpecBuilder &b) {
    const Line line = here() + 1;
    b.read("gecode.x", int_);
    return line;
  });
  unprintable([](SpecBuilder &b) {
    const Line line = here() + 1;
    b.read("not a name", int_);
    return line;
  });
  unprintable([](SpecBuilder &b) {
    const Line line = here() + 1;
    b.read("", int_);
    return line;
  });
  unprintable([](SpecBuilder &b) {
    auto a = b.read("a", int_.dim(1));
    const Line line = here() + 1;
    b.let("v",
          all(over(a).named("not a name"), [](const auto &x) { return x; }));
    return line;
  });
  unprintable(
      [](SpecBuilder &b) {
        const Line line = here() + 1;
        b.let("x", lit(std::numeric_limits<double>::quiet_NaN()));
        return line;
      },
      "NaN");
  unprintable(
      [](SpecBuilder &b) {
        const Line line = here() + 1;
        b.let("x", lit(std::numeric_limits<double>::infinity()));
        return line;
      },
      "inf");
  unprintable([](SpecBuilder &b) {
    b.read("x", Range{.from = 0});
    const Line line = here() + 1;
    b.format("second");
    return line;
  });
  unprintable([](SpecBuilder &b) {
    b.define_type("t", Range{.from = 0});
    const Line line = here() + 1;
    b.format("second");
    return line;
  });
  unprintable([](SpecBuilder &b) {
    const Line line = here() + 1;
    b.read("x", alphabet("ab").base(std::numeric_limits<Int>::min()));
    return line;
  });
  unprintable([](SpecBuilder &b) {
    const Line line = here() + 1;
    b.list("x", int_.layout(1).until(0));
    return line;
  });
  unprintable([](SpecBuilder &b) {
    const Line line = here() + 1;
    b.line(Pattern().part(validate(Range{.from = lit("}").dynamic()})));
    return line;
  });
  unprintable(
      [](SpecBuilder &b) {
        const Line line = here() + 1;
        b.read("x", Range{.from = 0, .to = 1, .below = 2});
        return line;
      },
      "both `to` and `below`");
}

void a_bound_after_an_open_range_is_fine_before_other_statements() {
  SpecBuilder b;
  b.read("x", Range{.from = 0});
  b.read("y", Range{.from = 0});
  b.instances([](auto & /*body*/) {});
  b.format("second");
  expect_build(b);
}

// Capture binding

void a_capture_binds_exactly_once_in_its_scope() {
  {
    SpecBuilder b;
    auto n = b.capture("n", int_);
    b.line(pattern("{}", n));
    const Line line = here() + 1;
    b.line(pattern("{}", n));
    expect_failure(b, BuildErrorKind::Binding, line, "twice");
  }
  {
    SpecBuilder b;
    const Line line = here() + 1;
    b.capture("n", int_);
    expect_failure(b, BuildErrorKind::Binding, line, "never");
  }
  {
    SpecBuilder b;
    auto n = b.capture("n", int_);
    Line line = 0;
    b.repeat(1, [&](auto &r) {
      line = here() + 1;
      r.line(pattern("{}", n));
    });
    expect_failure(b, BuildErrorKind::Binding, line, "different scope");
  }
}

void the_presence_of_a_capture_follows_its_arm() {
  {
    SpecBuilder b;
    auto n = b.capture("n", int_);
    Line line = 0;
    b.sections([&](auto &arms) {
      line = here() + 1;
      arms.optional_arm(pattern("{}", n));
    });
    expect_failure(b, BuildErrorKind::Binding, line, "optional_capture");
  }
  {
    SpecBuilder b;
    auto n = b.optional_capture("n", int_);
    const Line line = here() + 1;
    b.line(pattern("{}", n));
    expect_failure(b, BuildErrorKind::Binding, line, "`capture`");
  }
  {
    SpecBuilder b;
    auto n = b.optional_capture("n", int_);
    b.sections([&](auto &arms) { arms.optional_arm(pattern("{}", n)); });
    expect_build(b);
  }
}

void a_handle_that_is_not_a_capture_is_not_guessed() {
  {
    SpecBuilder b;
    auto x = b.read("x", int_);
    const Line line = here() + 1;
    b.line(pattern("{}", x));
    expect_failure(b, BuildErrorKind::Binding, line, "`ref`");
  }
  {
    SpecBuilder b;
    auto x = b.read("x", int_);
    b.line(pattern("{}", ref(x)));
    expect_text(b, "x: int\nline \"{$x}\"\n");
  }
}

void a_forward_handle_must_be_bound() {
  {
    SpecBuilder b;
    const Line line = here() + 1;
    b.forward<Field<Int>>("x");
    expect_failure(b, BuildErrorKind::Binding, line, "never");
  }
  {
    SpecBuilder b;
    const Line line = here() + 1;
    b.forward_record("each_one", "all_of_them");
    expect_failure(b, BuildErrorKind::Binding, line);
  }
}

void a_forward_handle_must_be_bound_as_declared() {
  {
    SpecBuilder b;
    auto x = b.forward<Field<Int>>("x");
    const Line line = here() + 1;
    b.read(x, float_);
    expect_failure(b, BuildErrorKind::Binding, line);
  }
  {
    SpecBuilder b;
    auto x = b.forward<Field<Int>>("x");
    Line line = 0;
    b.repeat(2, [&](auto &r) {
      line = here() + 1;
      r.read(x, int_);
    });
    expect_failure(b, BuildErrorKind::Binding, line);
  }
  {
    // A required handle whose only definition may not run names the call
    // that made it, which needs Presence::Maybe.
    SpecBuilder b;
    const Line line = here() + 1;
    auto x = b.forward<Field<Int>>("x");
    b.if_(1, [&](auto &t) { t.read(x, int_); });
    expect_failure(b, BuildErrorKind::Binding, line, "Presence::Maybe");
  }
  {
    SpecBuilder b;
    auto x = b.forward<Field<Int, 1>>("x");
    b.repeat(2, [&](auto &r) { r.read(x, int_); });
    const std::optional<iris::Data> data = read(b, "4 5");
    if (data) {
      const std::span<const Int> values = data->get(x);
      check(values.size() == 2 && values[1] == 5, "the forward handle reads");
    }
  }
}

void a_redefinition_needs_a_binding_that_always_precedes_it() {
  {
    SpecBuilder b;
    auto x = b.forward<Field<Int, 0, Presence::Maybe>>("x");
    auto branch = b.if_(1, [&](auto &t) { t.read(x, int_); });
    Line line = 0;
    branch.else_([&](auto &e) {
      line = here() + 1;
      e.read(x, float_);
    });
    expect_failure(b, BuildErrorKind::Binding, line, "float at depth 0");
  }
  {
    SpecBuilder b;
    auto x = b.forward<Field<Int, 0, Presence::Maybe>>("x");
    b.if_(1, [&](auto &t) { t.read(x, int_); });
    const Line line = here() + 1;
    b.read(x, float_);
    expect_failure(b, BuildErrorKind::Binding, line);
  }
  {
    // A binding inside a block precedes what follows it in that block.
    SpecBuilder b;
    auto x = b.forward<Field<Int, 0, Presence::Maybe>>("x");
    b.if_(1, [&](auto &t) {
      t.read(x, int_);
      t.if_(1, [&](auto &nested) { nested.read(x, float_); });
    });
    expect_build(b);
  }
}

void a_known_shape_binds_a_static_forward_handle() {
  {
    SpecBuilder b;
    auto a = b.read("a", int_.dim(2));
    auto x = b.forward<Field<Int>>("x");
    const Line line = here() + 1;
    b.set(x, a[0]);
    expect_failure(b, BuildErrorKind::Binding, line);
  }
  {
    SpecBuilder b;
    auto a = b.read("a", int_.dim(2));
    auto n = b.read("n", int_);
    auto whole = b.forward<Field<Int, 1>>("whole");
    b.set(whole, a);
    auto scalar = b.forward<Field<Int>>("scalar");
    b.set(scalar, n);
    auto computed = b.forward<Field<Int>>("computed");
    b.set(computed, n + 1);
    const std::optional<iris::Data> data = read(b, "1 2 3");
    if (data) {
      check(data->get(whole).size() == 2, "whole");
      check(data->get(scalar) == 3, "scalar");
      check(data->get(computed) == 4, "computed");
    }
  }
  {
    // A dynamic forward handle takes what a static one cannot certify.
    SpecBuilder b;
    auto a = b.read("a", int_.dim(2));
    auto x = b.forward<DynField>("x");
    b.let(x, a[0]);
    const std::optional<iris::Data> data = read(b, "7 8");
    if (data) {
      const std::optional<iris::ValueRef> value = data->get(x);
      check(value && value->integer() == 7, "a dynamic forward handle");
    }
  }
}

void a_named_type_handle_belongs_to_its_declaration() {
  {
    SpecBuilder b;
    auto old = b.if_(1, [](auto &s) { return s.define_type("t", int_); });
    Line line = 0;
    b.if_(1, [&](auto &s) {
      s.define_type("t", float_);
      line = here() + 1;
      s.read("x", old.result());
    });
    expect_failure(b, BuildErrorKind::Scope, line, "`t`");
  }
  {
    // After its block no declaration is visible.
    SpecBuilder b;
    auto old = b.if_(1, [](auto &s) { return s.define_type("t", int_); });
    const Line line = here() + 1;
    b.read("x", old.result());
    expect_failure(b, BuildErrorKind::Scope, line);
  }
  {
    SpecBuilder b;
    auto first = b.if_(1, [](auto &s) {
      auto t = s.define_type("t", int_);
      return s.read("first", t);
    });
    auto second = b.if_(1, [](auto &s) {
      auto t = s.define_type("t", float_);
      return s.read("second", t);
    });
    const std::optional<iris::Data> data = read(b, "1 2.5");
    if (data) {
      check(data->get(first.result()) == std::optional<Int>(1), "first");
      check(data->get(second.result()) == std::optional<double>(2.5), "second");
    }
  }
}

void only_a_store_defines_the_entry_named_underscore() {
  {
    SpecBuilder b;
    auto stored = b.forward<DynField>("_");
    b.repeat_until("END", {.store = stored}, [](auto & /*body*/) {});
    const Line line = here() + 1;
    b.read(stored, int_);
    expect_failure(b, BuildErrorKind::Binding, line, "`store`");
  }
  {
    SpecBuilder b;
    auto stored = b.forward<Field<Str>>("_");
    b.repeat_until("END", {.store = stored}, [](auto & /*body*/) {});
    auto copy = b.let("s", stored);
    b.line(pattern("{}", ref(stored)));
    expect_text(b,
                "repeat until \"END\" store _ {}\nlet s = _\nline \"{$_}\"\n");
    const std::optional<iris::Data> data = read(b, "END\nEND");
    if (data) {
      check(data->get(stored) == "END", "the stored line");
      check(data->get(copy).text() == "END", "the copy");
    }
  }
}

void an_undefined_name_cannot_be_defined() {
  {
    SpecBuilder b;
    auto zz = b.undefined("zz");
    b.check(!defined(zz));
    check(b.try_source().has_value(), "an undefined name prints");
    const Line line = here() + 1;
    b.let(zz, lit(1));
    expect_failure(b, BuildErrorKind::Binding, line, "undefined");
  }
  {
    SpecBuilder b;
    auto underscore = b.undefined("_");
    const Line line = here() + 1;
    b.repeat_until("END", {.store = underscore}, [](auto & /*body*/) {});
    expect_failure(b, BuildErrorKind::Binding, line, "undefined");
  }
}

void a_forward_record_binds_once() {
  {
    SpecBuilder b;
    auto job = b.forward_record("job", "jobs");
    b.repeat(1, [&](auto &r) { r.record_as(job); });
    Line line = 0;
    b.repeat(1, [&](auto &r) {
      line = here() + 1;
      r.record_as(job);
    });
    expect_failure(b, BuildErrorKind::Binding, line);
  }
  {
    SpecBuilder b;
    auto own = b.repeat(1, [](auto &r) { return r.record(); });
    Line line = 0;
    b.repeat(1, [&](auto &r) {
      line = here() + 1;
      r.record_as(own);
    });
    expect_failure(b, BuildErrorKind::Binding, line);
  }
  {
    SpecBuilder b;
    auto first = b.forward_record("first", "firsts");
    auto second = b.forward_record("second", "seconds");
    Line line = 0;
    b.repeat(1, [&](auto &r) {
      r.record_as(first);
      line = here() + 1;
      r.record_as(second);
    });
    b.let("n", size(first));
    expect_failure(b, BuildErrorKind::Binding, line, "already bound");
  }
  {
    SpecBuilder b;
    auto record = b.forward_record("_", "rows");
    b.repeat(1, [&](auto &r) {
      r.record_as(record);
      r.read("x", int_);
    });
    auto n = b.let("n", size(record));
    const std::optional<iris::Data> data = read(b, "1");
    if (data)
      check(data->get(n) == 1, "the size of the forward record");
  }
}

// Printing

void settings() {
  SpecBuilder b("alpha-beta.1");
  b.format("two words");
  b.comment("#");
  b.stop({"a", "b"});
  b.tokens(",;");
  b.separators(":");
  b.ignore_trailing();
  expect_text(b, "format alpha-beta.1\nformat \"two words\"\ncomment "
                 "\"#\"\nstop \"a\" \"b\"\ntokens \",;\"\nseparators "
                 "\":\"\nignore trailing\n");
}

void format_names_are_bare_only_when_they_have_that_spelling() {
  const auto names =
      std::to_array<std::pair<std::string_view, std::string_view>>({
          {"a", "format a\n"},
          {"a-b", "format a-b\n"},
          {"a.b-2", "format a.b-2\n"},
          {"a-1.2", "format \"a-1.2\"\n"},
          {"a-1e5", "format \"a-1e5\"\n"},
          {"1a", "format \"1a\"\n"},
          {"a-", "format \"a-\"\n"},
          {"", "format \"\"\n"},
          {"a b", "format \"a b\"\n"},
      });

  for (const auto &[name, expected] : names)
    expect_text(SpecBuilder(name), expected);
}

void declarations() {
  SpecBuilder b;
  auto n = b.read("n", int_);
  b.read("a", float_);
  b.read("b", num);
  b.read("c", word);
  b.read("d", rest);
  b.read("e", int_.dim(2).dim(n));
  b.read("f", int_.dim(2).dim(2).layout(1).offset(n));
  b.read("g", alphabet("ab").blank(" ").strip("\t").base(-1).dim(2).dim(2));
  b.read("h", alphabet("ab").dim(2).dim(n));
  b.read(discard, int_);
  expect_text(b, "n: int\na: float\nb: num\nc: word\nd: rest\ne[2][n]: int\n"
                 "f[2][2]: int layout 1 offset n\ng[2][2]: alphabet \"ab\" "
                 "blank \" \" strip \"\\t\" base -1\nh[2][n]: alphabet "
                 "\"ab\"\n_: int\n");
}

void ranges() {
  SpecBuilder b;
  auto n = b.read("n", int_);
  b.read("a", Range{.from = 0, .below = 10});
  b.read("b", Range{.from = 0, .to = 10});
  b.read("c", Range{.from = 0});
  b.read("d", Range{.below = 10});
  b.read("e", Range{.to = 10});
  b.read("f", Range{.from = 0, .below = n});
  b.read("g", Range{.from = 0, .below = 10}.dim(n));
  b.read("h", Range{.from = (n + 1).dynamic()});
  b.read("i", Range{.from = host::int_min, .to = host::int_max});
  expect_text(b, "n: int\na: 0..<10\nb: 0..10\nc: 0..\nd: ..<10\ne: ..10\n"
                 "f: 0..<n\ng[n]: 0..<10\nh: n + 1..\n"
                 "i: gecode.int_min..gecode.int_max\n");
}

void lists_types_and_params() {
  SpecBuilder b;
  b.list("a", int_.until(0));
  b.list("b", word.until("end"));
  b.list("c", int_.until(eol).offset(1));
  auto t = b.define_type("t", Range{.from = 0});
  b.read("x", t);
  auto u = b.define_type("u", t);
  b.list("d", ty(u).until(1));
  b.param("p");
  b.param("q", 3);
  expect_text(b, "list a: int until 0\nlist b: word until \"end\"\nlist c: "
                 "int until eol offset 1\ntype t = 0..\nx: t\ntype u = "
                 "t\nlist d: u until 1\nparam p\nparam q = 3\n");
}

void line_statements() {
  SpecBuilder b;
  auto n = b.read("n", int_);
  b.line("plain");
  b.find("a \"quoted\" \\ line\n");
  b.skip_line();
  b.skip_lines(n);
  b.index(n + 1);
  b.expect("x");
  b.expect({"y", "z"});
  expect_text(b, "n: int\nline \"plain\"\nfind \"a \\\"quoted\\\" \\\\ "
                 "line\\n\"\nskip line\nskip n lines\nindex n + 1\nexpect "
                 "\"x\"\nexpect \"y\" \"z\"\n");
}

void patterns() {
  SpecBuilder b;
  auto x = b.capture("x", int_);
  auto y = b.capture("y", untyped);
  auto z = b.capture("z", Range{.from = 1});
  auto w = b.forward<Field<Str>>("w");
  b.line(pattern("a {} b {}", x, y));
  b.line(pattern("{}", z));
  b.line(pattern("{}", ref(x)));
  b.line(pattern("{}", validate(float_)));
  b.line(pattern("{} {}", blank, discard));
  b.line(pattern("{}", discard_as(word)));
  b.line(pattern("{}", bind(w, word)));
  b.line(pattern("tail {}", rest_of_line));
  b.line(Pattern().text("a").part(rest_of_line).text("  "));
  b.line(pattern("lone } brace"));
  b.line("lone } brace");
  expect_text(b, "line \"a {x:int} b {y}\"\nline \"{z:1..}\"\nline "
                 "\"{$x}\"\nline \"{:float}\"\nline \"{} {_}\"\nline "
                 "\"{_:word}\"\nline \"{w:word}\"\nline \"tail ...\"\nline "
                 "\"a...  \"\nline \"lone } brace\"\nline \"lone } brace\"\n");
}

void repeats() {
  SpecBuilder b;
  auto n = b.read("n", int_);
  b.repeat(n, [](auto & /*body*/) {});
  b.repeat(n, [](auto &r) { r.read("a", int_); });
  b.repeat_until_eof([](auto &r) { r.read("b", int_); });
  b.repeat_until_eof([](auto & /*body*/) {});
  b.repeat_until("end", [](auto & /*body*/) {});
  auto stored = b.forward<Field<Str>>("stored");
  b.repeat_until({"x", "y"}, {.store = stored},
                 [](auto &r) { r.read("c", int_); });
  auto underscore = b.forward<Field<Str>>("_");
  b.repeat_until("z", {.store = underscore}, [](auto & /*body*/) {});
  expect_text(b, "n: int\nrepeat n {}\nrepeat n {\n  a: int\n}\nrepeat until "
                 "eof {\n  b: int\n}\nrepeat until eof {}\nrepeat until "
                 "\"end\" {}\nrepeat until \"x\" \"y\" store stored {\n  c: "
                 "int\n}\nrepeat until \"z\" store _ {}\n");
}

void user_names_for_records_always_print() {
  SpecBuilder b;
  b.repeat(2, [](auto &r) {
    r.each_name("job");
    r.as_name("jobs");
  });
  expect_text(b, "repeat 2 as jobs {\n  each job\n}\n");
}

void a_forward_record_serves_use_before_the_repeat() {
  SpecBuilder b;
  auto job = b.forward_record("job", "jobs");
  auto x = b.forward<Field<Int, 1>>("x");
  b.let("n", size(job));
  b.let("total", sum(job, [&](const auto &j) { return j.get(x); }));
  b.repeat(2, [&](auto &r) {
    r.record_as(job);
    r.read(x, int_);
  });
  expect_text(b, "let n = size(jobs)\nlet total = sum(element_1 in jobs: "
                 "element_1.x)\nrepeat 2 as jobs {\n  each job\n  x: "
                 "int\n}\n");
}

void sections() {
  SpecBuilder b;
  auto stored = b.forward<Field<Str, 0, Presence::Maybe>>("stored");
  auto a = b.optional_capture("a", int_);
  b.sections([](auto & /*arms*/) {});
  b.sections({{.until = "END", .store = stored, .skip_other = true},
              {.until = {"x", "y"}}},
             [&](auto &s) {
               s.arm("plain");
               s.optional_arm(pattern("a {}", a));
               s.arm("b", {.last = true});
               s.optional_arm("c", {.last = true});
               s.arm("d", [](auto & /*body*/) {});
               s.optional_arm("e", {.last = true},
                              [](auto &e) { e.read("x", int_); });
             });
  expect_text(b, "sections {}\nsections until \"END\" store stored skip other "
                 "until \"x\" \"y\" {\n  \"plain\"\n  \"a {a:int}\" optional\n "
                 " \"b\" last\n  \"c\" optional last\n  \"d\" {}\n  \"e\" "
                 "optional last {\n    x: int\n  }\n}\n");
}

void branches() {
  SpecBuilder b;
  auto n = b.read("n", int_);
  b.if_(n, [](auto & /*then*/) {});
  b.if_(n, [](auto &t) { t.read("a", int_); });
  b.if_(n, [](auto &t) { t.read("b", int_); }).else_([](auto &e) {
    e.read("c", int_);
  });
  auto chain = b.if_(n > 1, [](auto & /*then*/) {});
  chain.else_if(n > 0, [](auto & /*then*/) {});
  chain.else_([](auto & /*otherwise*/) {});
  auto branch = b.forward<DynField>("if");
  b.if_(n, [](auto & /*then*/) {}).else_read_if(branch, int_);
  auto last = b.if_(n, [](auto &t) { t.read("d", int_); });
  last.else_if(n < 0, [](auto &t) { t.read("e", int_); });
  expect_text(b, "n: int\nif n {}\nif n {\n  a: int\n}\nif n {\n  b: int\n} "
                 "else {\n  c: int\n}\nif n > 1 {} else if n > 0 {} else "
                 "{}\nif n {} else if: int\nif n {\n  d: int\n} else if n < 0 "
                 "{\n  e: int\n}\n");
}

void let_and_check() {
  SpecBuilder b;
  auto x = b.let("x", lit(1));
  b.set(x, lit(2));
  b.check(x);
  b.check(x > 0, "a \"quoted\" message");
  expect_text(b, "let x = 1\nlet x = 2\ncheck x\ncheck x > 0 \"a "
                 "\\\"quoted\\\" message\"\n");
}

void instances_and_a_later_format() {
  SpecBuilder b("one");
  b.instances([](auto &i) { i.read("x", int_); });
  b.format("two");
  expect_text(b, "format one\ninstances {\n  x: int\n}\nformat two\n");
}

/// Check that `let v = make(x, y, z)` prints as `let v = expected`
template <class F>
void expect_expression(
    F make, std::string_view expected,
    std::source_location where = std::source_location::current()) {
  SpecBuilder b;
  auto x = b.read("x", int_);
  auto y = b.read("y", int_);
  auto z = b.read("z", int_);
  b.let("v", make(x, y, z));
  expect_text(b,
              "x: int\ny: int\nz: int\nlet v = " + std::string(expected) + "\n",
              true, where);
}

using F = Field<Int>;

void literals() {
  expect_expression([](F, F, F) { return lit(1); }, "1");
  expect_expression([](F, F, F) { return lit(-1); }, "-1");
  expect_expression([](F, F, F) { return lit(1.0); }, "1.0");
  expect_expression([](F, F, F) { return lit(-1.5); }, "-1.5");
  expect_expression([](F, F, F) { return lit(1e300); }, "1e300");
  expect_expression([](F, F, F) { return lit(1e-5); }, "1e-5");
  expect_expression([](F, F, F) { return lit(0.1); }, "0.1");
  expect_expression([](F, F, F) { return lit(9007199254740992.0); },
                    "9007199254740992.0");
  expect_expression([](F, F, F) { return lit("a\tb"); }, R"("a\tb")");
  expect_expression([](F, F, F) { return lit(std::string("s")); }, "\"s\"");
  expect_expression([](F, F, F) { return lit(4294967295U); }, "4294967295");
  expect_expression(
      [](F, F, F) { return lit(std::numeric_limits<Int>::min()); },
      "(-9223372036854775807 - 1)");
  expect_expression(
      [](F, F, F) { return lit(std::numeric_limits<Int>::max()); },
      "9223372036854775807");
  expect_expression([](F, F, F) { return host::int_max + 0; },
                    "gecode.int_max + 0");
  expect_expression([](F, F, F) { return -host::int_min; }, "-gecode.int_min");
}

void host_constants_convert_to_expressions() {
  expect_expression([](F, F, F) { return lit(host::int_max); },
                    "gecode.int_max");
  expect_expression([](F, F, F) { return lit(host::int_min) * 2; },
                    "gecode.int_min * 2");
  expect_expression([](F, F, F) { return size(host::int_max.dynamic()); },
                    "size(gecode.int_max)");
  expect_expression([](F, F, F) { return count(host::int_min.dynamic()); },
                    "count(gecode.int_min)");
  expect_expression([](F, F, F) { return host::int_max.dynamic()[0]; },
                    "gecode.int_max[0]");
  expect_expression([](F, F, F) { return host::int_max.dynamic() == "s"; },
                    R"(gecode.int_max == "s")");
  expect_expression(
      [](F, F, F) {
        return sum(over(host::int_max.dynamic()).named("e"),
                   [](const auto &e) { return e; });
      },
      "sum(e in gecode.int_max: e)");
}

void parentheses_follow_the_precedence_levels() {
  expect_expression([](F x, F y, F z) { return x + y + z; }, "x + y + z");
  expect_expression([](F x, F y, F z) { return x + (y + z); }, "x + (y + z)");
  expect_expression([](F x, F y, F z) { return x - (y - z); }, "x - (y - z)");
  expect_expression([](F x, F y, F z) { return x * y + z; }, "x * y + z");
  expect_expression([](F x, F y, F z) { return x * (y + z); }, "x * (y + z)");
  expect_expression([](F x, F y, F z) { return (x + y) / z; }, "(x + y) / z");
  expect_expression([](F x, F y, F z) { return x % (y * z); }, "x % (y * z)");
  expect_expression([](F x, F y, F z) { return x < (y < z); }, "x < (y < z)");
  // Chained comparisons group to the left in C++ and in Iris alike.
  // NOLINTBEGIN(bugprone-chained-comparison)
#if defined(__GNUC__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wparentheses"
#endif
  expect_expression([](F x, F y, F z) { return x < y < z; }, "x < y < z");
  expect_expression([](F x, F y, F z) { return x == y != z; }, "x == y != z");
  expect_expression([](F x, F y, F z) { return x == (y != z); },
                    "x == (y != z)");
  expect_expression([](F x, F y, F z) { return x <= y >= z > 1; },
                    "x <= y >= z > 1");
  expect_expression([](F x, F y, F z) { return x < y == z; }, "x < y == z");
#if defined(__GNUC__)
#pragma GCC diagnostic pop
#endif
  // NOLINTEND(bugprone-chained-comparison)
  expect_expression([](F x, F y, F z) { return (x && y) || z; }, "x && y || z");
  expect_expression([](F x, F y, F z) { return x && (y || z); },
                    "x && (y || z)");
  expect_expression([](F x, F y, F z) { return (x && y) && z; }, "x && y && z");
  expect_expression([](F x, F y, F z) { return x && (y && z); },
                    "x && (y && z)");
  expect_expression([](F x, F y, F) { return !(x && y); }, "!(x && y)");
  expect_expression([](F x, F y, F) { return !x && y; }, "!x && y");
  expect_expression([](F x, F y, F) { return -(x + y); }, "-(x + y)");
  expect_expression([](F x, F, F) { return -(-x); }, "--x");
  expect_expression([](F x, F, F) { return !(!x); }, "!!x");
  expect_expression([](F x, F, F) { return -(!x); }, "-!x");
  expect_expression([](F x, F y, F) { return x - (-y); }, "x - -y");
}

void powers_bind_tighter_than_unary_on_the_left_only() {
  expect_expression([](F, F, F) { return -pow(2, 2); }, "-2 ^ 2");
  expect_expression([](F, F, F) { return pow(-lit(2), 2); }, "(-2) ^ 2");
  expect_expression([](F, F, F) { return pow(lit(-2), 2); }, "(-2) ^ 2");
  expect_expression([](F, F, F) { return pow(-2, 2); }, "(-2) ^ 2");
  expect_expression([](F, F, F) { return pow(2, pow(3, 4)); }, "2 ^ 3 ^ 4");
  expect_expression([](F, F, F) { return pow(pow(2, 3), 4); }, "(2 ^ 3) ^ 4");
  expect_expression([](F x, F, F) { return pow(2, -x); }, "2 ^ -x");
  expect_expression([](F x, F y, F) { return pow(x + y, 2); }, "(x + y) ^ 2");
  expect_expression([](F x, F y, F) { return pow(2, x + y); }, "2 ^ (x + y)");
  expect_expression([](F x, F y, F z) { return x * pow(y, z); }, "x * y ^ z");
}

void operators_take_literals_on_either_side() {
  expect_expression([](F x, F, F) { return 1 + x; }, "1 + x");
  expect_expression([](F x, F, F) { return Int{2} * (x - 1); }, "2 * (x - 1)");
  expect_expression([](F x, F, F) { return 7U / x; }, "7 / x");
  expect_expression([](F x, F, F) { return 7 % x; }, "7 % x");
  expect_expression([](F x, F, F) { return 1 + host::int_max - x; },
                    "1 + gecode.int_max - x");
}

void calls_and_comprehensions() {
  SpecBuilder b;
  auto a = b.read("a", int_.dim(2));
  b.let("v1", size(a));
  b.let("v2", count(a));
  b.let("v3", min(a));
  b.let("v4", max(a, 1));
  b.let("v5", sum(1, 2, 3));
  b.let("v6", distinct(std::vector{lit(1), lit(2)}));
  b.let("v7", defined(a));
  b.let("v8", all(a, [](const auto &x) { return x > 0; }));
  b.let("v9", any(a, [](const auto &x) { return x > 0; }));
  b.let("v10", sum(a, [](const auto &x) { return x; }));
  b.let("v11",
        count(over(a).named("item"), [](const auto &x) { return x > 1; }));
  b.let("v12", sum(a, [&](const auto &x) {
          return sum(a, [&](const auto &y) { return x.dynamic() * y; });
        }));
  b.let("v13", a[0] + a[1]);
  expect_text(b, "a[2]: int\nlet v1 = size(a)\nlet v2 = count(a)\nlet v3 = "
                 "min(a)\nlet v4 = max(a, 1)\nlet v5 = sum(1, 2, 3)\nlet v6 = "
                 "distinct(1, 2)\nlet v7 = defined(a)\nlet v8 = all(element_1 "
                 "in a: element_1 > 0)\nlet v9 = any(element_2 in a: "
                 "element_2 > 0)\nlet v10 = sum(element_3 in a: "
                 "element_3)\nlet v11 = count(item in a: item > 1)\nlet v12 = "
                 "sum(element_4 in a: sum(element_5 in a: element_4 * "
                 "element_5))\nlet v13 = a[0] + a[1]\n");
}

void internal_names_skip_every_name_in_the_spec() {
  SpecBuilder b;
  b.read("record_1", int_);
  b.read("records_1", int_);
  auto a = b.read("element_1", int_.dim(1));
  auto [job, x] = b.repeat(
      1, [](auto &r) { return std::pair{r.record(), r.read("x", int_)}; });
  b.let("v", sum(job, [&](const auto &j) { return j.get(x); }));
  b.check(job.get(x));
  b.let("u", all(a, [](auto e) { return e; }));
  expect_text(b, "record_1: int\nrecords_1: int\nelement_1[1]: int\nrepeat 1 "
                 "as records_2 {\n  each record_2\n  x: int\n}\nlet v = "
                 "sum(element_2 in records_2: element_2.x)\ncheck "
                 "record_2.x\nlet u = all(element_3 in element_1: "
                 "element_3)\n");
}

void records_project_and_index() {
  SpecBuilder b;
  auto [outer, inner, x] = b.repeat(2, [](auto &r) {
    auto [in, field] = r.repeat(
        2, [](auto &s) { return std::pair{s.record(), s.read("x", int_)}; });
    return std::tuple{r.record(), in, field};
  });
  b.let("a", outer[0].get(x)[1]);
  b.check(size(outer.records(inner)));
  b.let("c", outer[1].records(inner)[0].get(x));
  b.let("d", size(outer.list()));
  b.check(defined(outer));
  b.let("f", sum(outer, [&](const auto &o) {
          return sum(o.records(inner), [&](const auto &i) { return i.get(x); });
        }));
  expect_text(b, "repeat 2 as records_1 {\n  each record_1\n  repeat 2 as "
                 "records_2 {\n    x: int\n  }\n}\nlet a = "
                 "records_1[0].x[1]\ncheck size(record_1.records_2)\nlet c = "
                 "records_1[1].records_2[0].x\nlet d = size(records_1)\ncheck "
                 "defined(records_1)\nlet f = sum(element_1 in records_1: "
                 "sum(element_2 in element_1.records_2: element_2.x))\n");
}

void a_member_never_glues_to_a_literal_or_a_name_gecode() {
  SpecBuilder b;
  auto x = b.read("x", int_);
  auto g = b.undefined("gecode");
  b.let("a", lit(1).dynamic().get(x));
  b.let("b", g[0].get(x));
  b.let("c", g.dynamic().get(x));
  b.let("d", g.dynamic().get(g).get(x));
  b.let("e", (lit(1).dynamic() + 2).dynamic().get(x));
  expect_text(b,
              "x: int\nlet a = (1).x\nlet b = gecode[0].x\nlet c = gecode . "
              "x\nlet d = gecode . gecode . x\nlet e = (1 + 2).x\n",
              false);
}

void keywords_at_the_start_of_an_expression_take_parentheses() {
  SpecBuilder b;
  auto until = b.read("until", int_);
  auto line = b.read("line", int_);
  auto eol_name = b.read("eol", int_);
  auto int_name = b.read("int", int_);
  auto layout = b.read("layout", int_);
  auto alpha = b.read("alphabet", int_);
  auto t = b.read("t", int_);
  b.repeat(until, [](auto & /*body*/) {});
  b.repeat(until + 1, [](auto & /*body*/) {});
  b.skip_lines(line);
  b.skip_lines(line * 2);
  b.list("a", int_.until(eol_name));
  b.list("b", int_.until(eol_name + 1));
  b.read("c", Range{.from = 0, .to = layout});
  b.read("d", Range{.from = 0, .below = layout});
  b.read("e", Range{.to = layout + 1});
  b.read("f", Range{.from = int_name, .to = t});
  b.read("g", Range{.from = int_name + 1});
  b.read("h", Range{.from = alpha});
  b.read("j", Range{.from = t});
  b.line(pattern("{}", validate(Range{.from = 0, .to = layout})));
  expect_text(b, "until: int\nline: int\neol: int\nint: int\nlayout: "
                 "int\nalphabet: int\nt: int\nrepeat (until) {}\nrepeat "
                 "(until + 1) {}\nskip (line) lines\nskip (line * 2) "
                 "lines\nlist a: int until (eol)\nlist b: int until (eol + "
                 "1)\nc: 0..(layout)\nd: 0..<(layout)\ne: ..(layout + 1)\nf: "
                 "(int)..t\ng: (int + 1)..\nh: (alphabet)..\nj: t..\nline "
                 "\"{:0..(layout)}\"\n");
}

void a_named_type_takes_parentheses_as_a_lower_bound() {
  SpecBuilder b;
  auto named = b.undefined("named");
  b.define_type("named", int_);
  b.read("x", Range{.from = named.dynamic()});
  expect_text(b, "type named = int\nx: (named)..\n", false);
}

void every_stop_word_is_guarded_as_an_upper_bound() {
  for (std::string_view stop_word :
       {"comment", "stop", "expect",    "tokens",   "separators", "ignore",
        "param",   "list", "line",      "find",     "skip",       "index",
        "repeat",  "each", "instances", "sections", "if",         "let",
        "check",   "type", "layout",    "offset",   "until"}) {
    SpecBuilder b;
    auto bound = b.let(stop_word, lit(5));
    auto x = b.read("x", Range{.from = 0, .to = bound});
    const std::string text = b.source();
    check(text.ends_with("x: 0..(" + std::string(stop_word) + ")\n"), text);
    const std::optional<iris::Data> data = read(b, "3");
    if (data)
      check(data->get(x) == 3, "the bound reads");
  }
}

void numbers_round_trip() {
  SpecBuilder b;
  const std::array values{0.1, 1e-7, 1.7976931348623157e308,
                          123456789012345680.0, -0.0};
  std::vector<Field<double>> fields;
  fields.reserve(values.size());
  for (std::size_t i = 0; i < std::size(values); i++)
    fields.push_back(b.let("v" + std::to_string(i), lit(values[i])));
  auto minimum = b.let("minimum", lit(std::numeric_limits<Int>::min()));
  const std::optional<iris::Data> data = read(b, "");
  if (!data)
    return;
  for (std::size_t i = 0; i < std::size(values); i++)
    check(std::bit_cast<std::uint64_t>(data->get(fields[i])) ==
              std::bit_cast<std::uint64_t>(values[i]),
          "float " + std::to_string(i) + " keeps its bits");
  check(data->get(minimum) == std::numeric_limits<Int>::min(), "the minimum");
}

void the_language_decides_the_rest() {
  {
    // The language rejects a subnormal literal; the builder prints it.
    SpecBuilder b;
    const Line line = here() + 1;
    b.let("tiny", lit(5e-324));
    expect_failure(b, BuildErrorKind::Spec, line, "E-SPEC-NUMBER");
  }
  {
    // An undefined name is an error when a read evaluates it.
    SpecBuilder b;
    auto x = b.undefined("x");
    b.let("y", x);
    b.read(discard, int_);
    check(b.source().contains("let y = x"), "the undefined name prints");
    expect_build(b);
  }
}

void a_million_term_chain_prints() {
  SpecBuilder b;
  auto x = b.read("x", int_);
  auto chain = x + 1;
  // Moving the chain keeps each step constant; a copy would copy it all.
  for (int i = 0; i < 1'000'000; i++)
    chain = std::move(chain) + x;
  b.let("v", chain);
  const std::string text = b.source();
  check(std::ranges::count(text, '+') == 1'000'001, "every term prints");
}

void a_deep_right_chain_prints_without_recursion() {
  SpecBuilder b;
  auto x = b.read("x", int_);
  auto chain = -x;
  for (int i = 0; i < 20'000; i++)
    chain = -std::move(chain);
  b.let("v", chain);
  const std::string text = b.source();
  check(std::ranges::count(text, '-') == 20'001, "every minus prints");
}

void three_hundred_dimensions_read_through_a_dynamic_handle() {
  SpecBuilder b;
  const std::vector<int> ones(300, 1);
  auto deep = b.read("deep", int_.dims(ones));
  const std::optional<iris::Data> data = read(b, "7");
  if (!data)
    return;
  iris::ValueRef value = data->get(deep);
  for (int i = 0; i < 299; i++)
    value = value[0];
  check(value.ints().size() == 1 && value.ints()[0] == 7, "the deep value");
}

void ten_nested_repeats_stay_static() {
  SpecBuilder b;
  auto leaf = b.repeat(1, [](auto &s1) {
    return s1.repeat(1, [](auto &s2) {
      return s2.repeat(1, [](auto &s3) {
        return s3.repeat(1, [](auto &s4) {
          return s4.repeat(1, [](auto &s5) {
            return s5.repeat(1, [](auto &s6) {
              return s6.repeat(1, [](auto &s7) {
                return s7.repeat(1, [](auto &s8) {
                  return s8.repeat(1, [](auto &s9) {
                    return s9.repeat(
                        1, [](auto &s10) { return s10.read("leaf", int_); });
                  });
                });
              });
            });
          });
        });
      });
    });
  });
  static_assert(std::same_as<decltype(leaf), Field<Int, 10>>);
  const std::optional<iris::Data> data = read(b, "9");
  if (!data)
    return;
  const iris::Array<Int, 10> value = data->get(leaf);
  check(value[0][0][0][0][0][0][0][0][0][0] == 9, "the leaf");
}

// Reading through handles

void capture_types_name_the_records_and_elements_they_use() {
  SpecBuilder b;
  auto a = b.read("a", int_.dim(2));
  auto x =
      b.capture("x", Range{.from = 0, .to = sum(a, [](auto v) { return v; })});
  b.line(pattern("{}", x));
  auto [record, y, z] = b.repeat(2, [](auto &r) {
    auto here_record = r.record();
    auto value = r.read("y", int_);
    auto captured =
        r.capture("z", Range{.from = 0, .to = here_record.get(value)});
    r.line(pattern("{}", captured));
    return std::tuple{here_record, value, captured};
  });
  auto w = b.capture(
      "w", Range{.from = 0,
                 .to = sum(record, [&](const auto &r) { return r.get(y); })});
  b.line(pattern("{}", w));
  expect_text(b, "a[2]: int\nline \"{x:0..sum(element_1 in a: "
                 "element_1)}\"\nrepeat 2 as records_1 {\n  each record_1\n  "
                 "y: int\n  line \"{z:0..record_1.y}\"\n}\nline "
                 "\"{w:0..sum(element_2 in records_1: element_2.y)}\"\n");
  const std::optional<iris::Data> data = read(b, "1 2\n3\n1\n1\n2\n2\n3\n");
  if (!data)
    return;
  check(data->get(x) == 3, "x");
  const std::span<const Int> zs = data->get(z);
  check(zs.size() == 2 && zs[0] == 1 && zs[1] == 2, "z");
  check(data->get(w) == 3, "w");
}

void an_empty_array_has_the_typed_empty_view() {
  SpecBuilder b;
  auto ints = b.read("ints", int_.dim(0));
  auto floats = b.read("floats", float_.dim(0));
  auto strs = b.read("strs", word.dim(0));
  auto nums = b.read("nums", num.dim(0));
  auto rows = b.read("rows", float_.dim(0).dim(3));
  auto none = b.repeat(0, [](auto &r) { return r.read("none", word); });
  const std::optional<iris::Data> data = read(b, "");
  if (!data)
    return;
  check(data->get(ints).empty(), "ints");
  check(data->get(floats).empty(), "floats");
  check(data->get(strs).empty(), "strs");
  check(std::visit([](auto span) { return span.empty(); }, data->get(nums)),
        "nums");
  check(data->get(rows).empty(), "rows");
  check(data->get(none).empty(), "a repeat of nothing");
}

void rows_walk_one_level_at_a_time() {
  SpecBuilder b;
  auto x = b.read("x", float_.dim(2).dim(2));
  const std::optional<iris::Data> data = read(b, "1 2 3 4");
  if (!data)
    return;
  const iris::Array<double, 2> rows = data->get(x);
  std::vector<double> seen;
  for (std::span<const double> row : rows)
    seen.insert(seen.end(), row.begin(), row.end());
  check(seen == std::vector<double>{1, 2, 3, 4}, "the rows");
  constexpr Field<double, 1> shallow("x");
  bool threw = false;
  try {
    (void)data->get(shallow);
  } catch (const iris::AccessError &) {
    threw = true;
  }
  check(threw, "a handle of the wrong depth throws");
}

void an_absent_optional_entry_is_empty() {
  SpecBuilder b;
  auto n = b.read("n", int_);
  auto x = b.if_(n, [](auto &t) { return t.read("x", int_); });
  const std::optional<iris::Data> data = read(b, "0");
  if (data)
    check(!data->get(x.result()).has_value(), "absent");
}

void numbers_keep_the_type_the_data_gave() {
  SpecBuilder b;
  auto row = b.read("row", num.dim(2));
  auto one = b.read("one", num);
  const std::optional<iris::Data> data = read(b, "1 2 2.5");
  if (!data)
    return;
  const iris::Numbers numbers = data->get(row);
  check(std::holds_alternative<std::span<const Int>>(numbers), "ints");
  check(data->get(one) == Number(2.5), "a float");
}

void a_declared_handle_reports_what_went_wrong() {
  SpecBuilder b;
  b.read("x", int_);
  const std::optional<iris::Data> data = read(b, "1");
  if (!data)
    return;
  static constexpr Field<Int> missing("missing");
  static constexpr Field<Str> wrong_type("x");
  static constexpr Field<Int, 1> wrong_depth("x");
  static constexpr Field<Int, 0, Presence::Maybe> absent("missing");
  static constexpr Field<Int> right("x");
  const auto throws = [&](const auto &handle) {
    try {
      (void)data->get(handle);
    } catch (const iris::AccessError &) {
      return true;
    }
    return false;
  };
  check(throws(missing), "missing");
  check(throws(wrong_type), "wrong type");
  check(throws(wrong_depth), "wrong depth");
  check(!data->get(absent).has_value(), "absent");
  check(data->get(right) == 1, "right");
}

void a_dynamic_handle_checks_what_it_knows() {
  const auto throws = [](const iris::Data &data, const auto &handle) {
    try {
      (void)data.get(handle);
    } catch (const iris::AccessError &) {
      return true;
    }
    return false;
  };
  {
    SpecBuilder b;
    b.read("x", int_.dim(1).dim(1));
    const std::optional<iris::Data> data = read(b, "5");
    if (data) {
      static constexpr Field<iris::Dynamic, 0> scalar("x");
      static constexpr Field<iris::Dynamic, 1> shallow("x");
      static constexpr Field<iris::Dynamic, 2> right("x");
      check(throws(*data, scalar), "a known depth of 0");
      check(throws(*data, shallow), "a known depth of 1");
      check(data->get(right).size() == 1, "the known depth of 2");
      check(data->get(DynField("x")).has_value(), "an unknown depth");
    }
  }
  {
    SpecBuilder b;
    auto ints = b.read("ints", int_.dims(std::vector{1}));
    auto nums = b.read("nums", num.dims(std::vector{1}));
    const std::optional<iris::Data> data = read(b, "1 2.5");
    if (data) {
      check(data->get(ints)[0].integer() == 1, "ints of unknown depth");
      check(data->get(nums)[0].number() == 2.5, "nums of unknown depth");
    }
  }
  {
    SpecBuilder b;
    auto ints = b.read("ints", int_.dims(std::vector{1}));
    auto nums = b.read("nums", num.dims(std::vector{1}));
    b.read(ints, word);
    b.read(nums, word);
    const std::optional<iris::Data> data = read(b, "1 2 a b");
    if (data) {
      check(throws(*data, ints), "an int handle on a word");
      check(throws(*data, nums), "a num handle on a word");
    }
  }
}

void typed_handles_set_parameters() {
  SpecBuilder b;
  auto scale = b.param("scale", 3);
  auto ratio = b.param("ratio", 1.5);
  auto whole = b.param("whole", 1.5);
  auto zero = b.param("zero", 1.5);
  auto label = b.param("label", lit("default"));
  auto x = b.read("x", num);
  auto number = b.param("number", x);
  auto inferred_float = b.param("inferred_float");
  auto inferred_int = b.param("inferred_int");
  const std::optional<Spec> spec = expect_build(b);
  if (!spec)
    return;
  iris::Params params;
  params.set(scale, 7);
  params.set(whole, 2);
  params.set(zero, -0.0);
  params.set(label, "a b,c=1");
  params.set(number, 2.5);
  params.set(inferred_float, 2.0);
  params.set(inferred_int, -12);
  check(params.get("inferred_float") == "2.0", "a whole float keeps `.0`");
  const auto read_with = [&](const iris::Params &values) {
    std::expected<iris::Data, iris::Error> data =
        spec->try_read_text("1.0", "input", values);
    check(data.has_value(), data ? "" : data.error().what());
    return data ? std::optional<iris::Data>(std::move(*data)) : std::nullopt;
  };
  const std::optional<iris::Data> data = read_with(params);
  if (!data)
    return;
  check(data->get(scale) == 7, "an int");
  check(data->get(whole) == 2.0, "an integer for a float");
  check(data->get(zero) == 0 && std::signbit(data->get(zero)),
        "a negative zero");
  check(data->get(label) == "a b,c=1", "a string, exactly");
  const iris::ValueRef as_number = data->get(number);
  check(as_number.type() == iris::ValueType::Float && as_number.number() == 2.5,
        "a num");
  const iris::ValueRef as_float = data->get(inferred_float);
  check(as_float.type() == iris::ValueType::Float && as_float.number() == 2,
        "a float of unknown type");
  const iris::ValueRef as_int = data->get(inferred_int);
  check(as_int.type() == iris::ValueType::Int && as_int.integer() == -12,
        "an int of unknown type");
  for (const double value :
       {0.1, 1.0 / 3, 1e-5, 123456789.125, 1e16, -3.5e-200, 1e300,
        std::numeric_limits<double>::min(), std::numeric_limits<double>::max(),
        -std::numeric_limits<double>::max()}) {
    params.set(ratio, value);
    const std::optional<iris::Data> again = read_with(params);
    check(again && std::bit_cast<std::uint64_t>(again->get(ratio)) ==
                       std::bit_cast<std::uint64_t>(value),
          "the float round trips: " +
              std::string(params.get("ratio").value_or("")));
  }
}

// The running example of docs/builders.md

constexpr std::string_view msg_distinct =
    "a job uses a machine twice; each job must use every machine once";
constexpr std::string_view msg_total = "the total duration is too large";

struct Jobshop {
  std::string text;
  Spec spec;
  Field<Str, 0, Presence::Maybe> name;
  Field<Int> n_jobs;
  Field<Int> n_machines;
  Field<Int, 2> machine;
  Field<Int, 2> duration;
};

std::optional<Jobshop> jobshop() {
  SpecBuilder b("jobshop");
  b.comment({"#", "+++"});
  auto handles = b.instances([](auto &i) {
    auto name = i.optional_capture("name", word);
    auto n_jobs = i.capture("n_jobs", Range{.from = 1});
    auto n_machines = i.capture("n_machines", Range{.from = 1});
    auto jobs = i.sections({.skip_other = true}, [&](auto &s) {
      s.optional_arm(pattern("instance {}", name));
      return s.arm(
          pattern("{} {}", n_jobs, n_machines), {.last = true}, [&](auto &a) {
            a.check(n_jobs <= (host::int_max - 2) / 2 / n_machines,
                    "there are too many jobs and machines");
            return a.repeat(n_jobs, [&](auto &job) {
              auto [machine,
                    duration] = job.repeat(n_machines, [&](auto &step) {
                return std::tuple{
                    step.read("machine", Range{.from = 0, .below = n_machines}),
                    step.read("duration", Range{.from = 0})};
              });
              return std::tuple{job.record(), machine, duration};
            });
          });
    });
    auto [job, machine, duration] = jobs;
    i.check(distinct(job.get(machine)), msg_distinct);
    i.check(sum(job, [&](const auto &j) { return sum(j.get(duration)); }) <=
                host::int_max,
            msg_total);
    return std::tuple{name, n_jobs, n_machines, machine, duration};
  });
  auto [name, n_jobs, n_machines, machine, duration] = handles;
  std::optional<Spec> spec = expect_build(b);
  if (!spec)
    return std::nullopt;
  return Jobshop{b.source(), std::move(*spec), name,    n_jobs,
                 n_machines, machine,          duration};
}

void the_jobshop_text_differs_only_in_internal_names() {
  const std::optional<Jobshop> built = jobshop();
  if (!built)
    return;
  const std::string expected = R"(format jobshop
comment "#" "+++"
instances {
  sections skip other {
    "instance {name:word}" optional
    "{n_jobs:1..} {n_machines:1..}" last {
      check n_jobs <= (gecode.int_max - 2) / 2 / n_machines "there are too many jobs and machines"
      repeat n_jobs as records_1 {
        each record_1
        repeat n_machines {
          machine: 0..<n_machines
          duration: 0..
        }
      }
    }
  }
  check distinct(record_1.machine) "a job uses a machine twice; each job must use every machine once"
  check sum(element_1 in records_1: sum(element_1.duration)) <= gecode.int_max "the total duration is too large"
}
)";
  check(built->text == expected, "the text is\n" + built->text);
  const std::optional<iris::Data> data = [&]() -> std::optional<iris::Data> {
    std::expected<iris::Data, iris::Error> read = built->spec.try_read_text(
        "instance tiny\n2 2\n0 3 1 4\n1 5 0 6\n", "input.txt");
    if (!read) {
      check(false, read.error().what());
      return std::nullopt;
    }
    return std::move(*read);
  }();
  if (!data)
    return;
  // The documented types
  const std::int64_t jobs = data->get(built->n_jobs);
  const std::int64_t machines = data->get(built->n_machines);
  const std::span<const std::int64_t> first_job = data->get(built->machine)[0];
  const std::int64_t last_duration = data->get(built->duration)[1][1];
  const std::optional<std::string_view> label = data->get(built->name);
  check(jobs == 2 && machines == 2, "the sizes");
  check(first_job.size() == 2 && first_job[0] == 0 && first_job[1] == 1,
        "the first job");
  check(last_duration == 6, "the last duration");
  check(label == "tiny", "the label");
}

/// Read a flat JSON object of strings, as params.json holds
iris::Params params_of(const std::filesystem::path &path) {
  iris::Params params;
  std::ifstream file(path);
  const std::string text((std::istreambuf_iterator<char>(file)),
                         std::istreambuf_iterator<char>());
  const std::regex pair(R"re("([^"]*)"\s*:\s*"([^"]*)")re");
  for (auto it = std::sregex_iterator(text.begin(), text.end(), pair);
       it != std::sregex_iterator(); ++it)
    params.set((*it)[1].str(), (*it)[2].str());
  return params;
}

void every_jobshop_case_reads_like_the_text_spec() {
  const std::optional<Jobshop> built = jobshop();
  if (!built)
    return;
  const std::filesystem::path cases =
      std::filesystem::path(IRIS_REPOSITORY) / "formats/jobshop/cases";
  int compared = 0;
  for (const auto &entry : std::filesystem::directory_iterator(cases)) {
    const std::filesystem::path &dir = entry.path();
    const Spec text = Spec::load(dir / "spec.iris");
    const iris::Params params = std::filesystem::exists(dir / "params.json")
                                    ? params_of(dir / "params.json")
                                    : iris::Params();
    const std::expected<iris::Data, iris::Error> expected =
        text.try_read(dir / "input.txt", params);
    const std::expected<iris::Data, iris::Error> actual =
        built->spec.try_read(dir / "input.txt", params);
    const std::string name = dir.filename().string();
    check(expected.has_value() == actual.has_value(), name + ": both read");
    if (expected && actual)
      check(expected->dzn() == actual->dzn(), name + ": the same data");
    else if (!expected && !actual)
      check(expected.error().code() == actual.error().code(),
            name + ": the same error");
    compared++;
  }
  check(compared == 3, "three cases");
}

// Determinism

std::string program() {
  SpecBuilder b("determinism");
  auto n = b.capture("n", Range{.from = 1});
  b.line(pattern("{}", n));
  auto [record, x] = b.repeat(
      n, [](auto &r) { return std::pair{r.record(), r.read("x", int_)}; });
  b.check(sum(record, [&](const auto &a) {
            return sum(record, [&](const auto &c) {
              return a.get(x).dynamic() * c.get(x);
            });
          }) >= 0);
  return b.source();
}

void concurrent_builders_print_the_same_text() {
  const std::string expected = program();
  check(program() == expected, "the same program prints the same text");
  std::array<std::vector<std::string>, 2> texts;
  {
    std::jthread first([&] {
      for (int i = 0; i < 200; i++)
        texts[0].push_back(program());
    });
    std::jthread second([&] {
      for (int i = 0; i < 200; i++)
        texts[1].push_back(program());
    });
  }
  for (const std::vector<std::string> &list : texts)
    for (const std::string &text : list)
      check(text == expected, "a concurrent builder prints the same text");
}

} // namespace

int main() {
  try {
    a_statement_through_an_outer_scope_is_rejected();
    an_arm_through_an_outer_block_is_rejected();
    a_branch_continued_after_later_statements_is_rejected();
    a_scope_used_after_its_block_is_rejected();
    a_handle_from_another_builder_is_rejected();
    a_declared_handle_reads_but_does_not_build();
    defining_a_name_twice_needs_the_handle();
    values_with_no_spelling_are_rejected();
    a_bound_after_an_open_range_is_fine_before_other_statements();
    a_capture_binds_exactly_once_in_its_scope();
    the_presence_of_a_capture_follows_its_arm();
    a_handle_that_is_not_a_capture_is_not_guessed();
    a_forward_handle_must_be_bound();
    a_forward_handle_must_be_bound_as_declared();
    a_redefinition_needs_a_binding_that_always_precedes_it();
    a_known_shape_binds_a_static_forward_handle();
    a_named_type_handle_belongs_to_its_declaration();
    only_a_store_defines_the_entry_named_underscore();
    an_undefined_name_cannot_be_defined();
    a_forward_record_binds_once();
    settings();
    format_names_are_bare_only_when_they_have_that_spelling();
    declarations();
    ranges();
    lists_types_and_params();
    line_statements();
    patterns();
    repeats();
    user_names_for_records_always_print();
    a_forward_record_serves_use_before_the_repeat();
    sections();
    branches();
    let_and_check();
    instances_and_a_later_format();
    literals();
    host_constants_convert_to_expressions();
    parentheses_follow_the_precedence_levels();
    powers_bind_tighter_than_unary_on_the_left_only();
    operators_take_literals_on_either_side();
    calls_and_comprehensions();
    internal_names_skip_every_name_in_the_spec();
    records_project_and_index();
    a_member_never_glues_to_a_literal_or_a_name_gecode();
    keywords_at_the_start_of_an_expression_take_parentheses();
    a_named_type_takes_parentheses_as_a_lower_bound();
    every_stop_word_is_guarded_as_an_upper_bound();
    numbers_round_trip();
    the_language_decides_the_rest();
    a_million_term_chain_prints();
    a_deep_right_chain_prints_without_recursion();
    three_hundred_dimensions_read_through_a_dynamic_handle();
    ten_nested_repeats_stay_static();
    capture_types_name_the_records_and_elements_they_use();
    an_empty_array_has_the_typed_empty_view();
    rows_walk_one_level_at_a_time();
    an_absent_optional_entry_is_empty();
    numbers_keep_the_type_the_data_gave();
    a_declared_handle_reports_what_went_wrong();
    a_dynamic_handle_checks_what_it_knows();
    typed_handles_set_parameters();
    concurrent_builders_print_the_same_text();
    the_jobshop_text_differs_only_in_internal_names();
    every_jobshop_case_reads_like_the_text_spec();
  } catch (const std::exception &error) {
    std::cerr << "uncaught exception: " << error.what() << '\n';
    return EXIT_FAILURE;
  }
  if (failures > 0) {
    std::cerr << failures << " checks failed\n";
    return EXIT_FAILURE;
  }
  return EXIT_SUCCESS;
}
