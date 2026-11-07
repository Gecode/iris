// Errors point at the host code: every statement records the line that
// called it, and failures carry that line beside their location in the
// emitted text, when the spec is built and when it reads. A port of
// rust/iris-build/tests/call_sites.rs.
//
// The table of call sites and the owner line of an error are internal, so
// this test includes the library's own header to look at them.

#include "call_sites.hpp"

#include <iris/build.hpp>
#include <iris/iris.hpp>

#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <exception>
#include <expected>
#include <iostream>
#include <limits>
#include <optional>
#include <source_location>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace {

using namespace iris::build;
using iris::Data;
using iris::ErrorCode;
using iris::Field;
using iris::Presence;
using iris::Spec;
using Line = std::uint_least32_t;
using Int = std::int64_t;

int failures = 0;

void check(bool condition, std::string_view what,
           std::source_location where = std::source_location::current()) {
  if (condition)
    return;
  failures++;
  std::cerr << where.file_name() << ':' << where.line()
            << ": check failed: " << what << '\n';
}

/// Return the line of the call
Line here(std::source_location where = std::source_location::current()) {
  return where.line();
}

/// Return the name of this file, as a call site names it
std::string_view this_file() {
  return std::source_location::current().file_name();
}

/// Return how `site` prints: `file:line:column`, or `none`
std::string describe(const std::optional<std::source_location> &site) {
  return site ? iris::detail::site_text(*site) : "none";
}

/// Check that `site` is line `line` of this file
void expect_here(const std::optional<std::source_location> &site, Line line,
                 std::source_location where = std::source_location::current()) {
  check(site && std::string_view(site->file_name()) == this_file() &&
            site->line() == line,
        "the call site is line " + std::to_string(line) + ", not " +
            describe(site),
        where);
}

/// Return the error that building `b` fails with
std::optional<BuildError>
build_error(const SpecBuilder &b,
            std::source_location where = std::source_location::current()) {
  std::expected<Spec, BuildError> built = b.try_build();
  if (built) {
    check(false, "the builder should fail", where);
    return std::nullopt;
  }
  return std::move(built.error());
}

/// Return the language error that building `b` fails with
std::optional<iris::Error>
spec_error(const SpecBuilder &b,
           std::source_location where = std::source_location::current()) {
  std::optional<BuildError> error = build_error(b, where);
  if (!error)
    return std::nullopt;
  check(error->kind() == BuildErrorKind::Spec &&
            error->language_error().has_value(),
        std::string("the emitted text is rejected: ") + error->what(), where);
  return error->language_error();
}

/// Return the spec that `b` builds
std::optional<Spec>
built(const SpecBuilder &b,
      std::source_location where = std::source_location::current()) {
  std::expected<Spec, BuildError> spec = b.try_build();
  if (!spec) {
    check(false, std::string("the builder fails: ") + spec.error().what(),
          where);
    return std::nullopt;
  }
  return std::move(*spec);
}

/// Return the error that reading `input` with the spec `b` builds fails
/// with
std::optional<iris::Error>
read_error(const SpecBuilder &b, std::string_view input,
           std::source_location where = std::source_location::current()) {
  const std::optional<Spec> spec = built(b, where);
  if (!spec)
    return std::nullopt;
  std::expected<Data, iris::Error> data =
      spec->try_read_text(input, "input.txt");
  if (data) {
    check(false, "the read should fail", where);
    return std::nullopt;
  }
  return std::move(data.error());
}

/// Return the spec line of `error`, or 0
std::size_t spec_line(const std::optional<iris::Error> &error) {
  if (!error || !error->spec_location())
    return 0;
  return error->spec_location()->line.value_or(0);
}

// Language errors at build()

void a_parse_error_names_the_statement_after_instances() {
  SpecBuilder b;
  b.instances([](auto & /*body*/) {});
  const Line line = here() + 1;
  b.read("after", int_);
  std::optional<BuildError> error = build_error(b);
  if (!error || !error->language_error())
    return;
  const iris::Error &spec = *error->language_error();
  expect_here(error->call_site(), line);
  expect_here(spec.call_site(), line);
  check(spec.spec_location() && spec.spec_location()->origin == "builder.iris",
        "the origin");
  check(spec_line(spec) == 2, "line 2");
  check(error->emitted_line() == "after: int", "the emitted line");
  check(error->text() == "instances {}\nafter: int\n", "the text");
}

void a_resolution_error_names_the_statement_that_uses_the_name() {
  SpecBuilder b;
  auto outside = b.read("outside", int_);
  auto record = b.repeat(2, [](auto &s) {
    s.read("inside", int_);
    return s.record();
  });
  const Line line = here() + 1;
  b.let("bad", record.get(outside));
  std::optional<BuildError> error = build_error(b);
  if (!error || !error->language_error())
    return;
  check(error->language_error()->code() == ErrorCode::SpecRecordScope,
        "E-SPEC-RECORD-SCOPE");
  expect_here(error->call_site(), line);
  check(spec_line(error->language_error()) == 6, "line 6");
  check(error->emitted_line() == "let bad = record_1.outside",
        "the emitted line");
}

void the_display_shows_the_code_message_emitted_line_and_call_site() {
  SpecBuilder b;
  b.instances([](auto & /*body*/) {});
  const Line line = here() + 1;
  b.read("after", int_);
  std::optional<BuildError> error = build_error(b);
  if (!error || !error->language_error())
    return;
  const iris::Error &spec = *error->language_error();
  const std::string site = describe(spec.call_site());
  check(site.starts_with(std::string(this_file()) + ":" + std::to_string(line) +
                         ":"),
        "the call site " + site);
  // The language error ends with the call site, which the builder error
  // shows after the emitted line instead
  const std::string message = error->message();
  check(spec.what() == message + "\ncall site: " + site,
        std::string("the language error: ") + spec.what());
  const std::string expected = std::string(iris::code_name(spec.code())) +
                               ": " + message +
                               "\n     2 | after: int\ncall site: " + site;
  check(error->what() == expected,
        std::string("the display: ") + error->what() + "\nnot\n" + expected);
  check(message == "builder.iris:2: " + spec.message(), "the message");
}

/// A builder whose statement at the returned line follows `instances`,
/// which the language rejects, and whose emitted line starts with `emitted`
template <class F>
void after_instances(
    F statement, std::string_view emitted,
    std::source_location where = std::source_location::current()) {
  SpecBuilder b;
  b.instances([](auto & /*body*/) {});
  const Line line = statement(b);
  std::optional<BuildError> error = build_error(b, where);
  if (!error)
    return;
  expect_here(error->call_site(), line, where);
  check(error->emitted_line() && error->emitted_line()->starts_with(emitted),
        "the emitted line starts with " + std::string(emitted), where);
}

void every_statement_names_its_call() {
  after_instances(
      [](SpecBuilder &b) {
        const Line line = here() + 1;
        b.param("p");
        return line;
      },
      "param p");
  after_instances(
      [](SpecBuilder &b) {
        const Line line = here() + 1;
        b.param("q", 1);
        return line;
      },
      "param q");
  after_instances(
      [](SpecBuilder &b) {
        const Line line = here() + 1;
        b.define_type("t", int_);
        return line;
      },
      "type t");
  after_instances(
      [](SpecBuilder &b) {
        const Line line = here() + 1;
        b.list("l", int_.until(0));
        return line;
      },
      "list l");
  after_instances(
      [](SpecBuilder &b) {
        const Line line = here() + 1;
        b.let("v", 1);
        return line;
      },
      "let v");
  after_instances(
      [](SpecBuilder &b) {
        const Line line = here() + 1;
        b.check(lit(1) > 0, "message");
        return line;
      },
      "check");
  after_instances(
      [](SpecBuilder &b) {
        const Line line = here() + 1;
        b.expect("word");
        return line;
      },
      "expect");
  after_instances(
      [](SpecBuilder &b) {
        const Line line = here() + 1;
        b.skip_line();
        return line;
      },
      "skip line");
  after_instances(
      [](SpecBuilder &b) {
        const Line line = here() + 1;
        b.skip_lines(1);
        return line;
      },
      "skip 1 lines");
  after_instances(
      [](SpecBuilder &b) {
        auto m = b.capture("m", word);
        const Line line = here() + 1;
        b.line(pattern("m {}", m));
        return line;
      },
      "line");
  after_instances(
      [](SpecBuilder &b) {
        const Line line = here() + 1;
        b.find("x");
        return line;
      },
      "find");
  after_instances(
      [](SpecBuilder &b) {
        const Line line = here() + 1;
        b.index(0);
        return line;
      },
      "index");
  after_instances(
      [](SpecBuilder &b) {
        const Line line = here() + 1;
        b.if_(1, [](auto & /*then*/) {});
        return line;
      },
      "if 1");
  after_instances(
      [](SpecBuilder &b) {
        const Line line = here() + 1;
        b.repeat(2, [](auto & /*body*/) {});
        return line;
      },
      "repeat 2");
  after_instances(
      [](SpecBuilder &b) {
        const Line line = here() + 1;
        b.repeat_until_eof([](auto &s) { s.read("x", int_); });
        return line;
      },
      "repeat until eof");
  after_instances(
      [](SpecBuilder &b) {
        const Line line = here() + 1;
        b.repeat_until("END", [](auto & /*body*/) {});
        return line;
      },
      "repeat until");
  after_instances(
      [](SpecBuilder &b) {
        const Line line = here() + 1;
        b.sections([](auto &arms) { arms.arm("one"); });
        return line;
      },
      "sections");
  after_instances(
      [](SpecBuilder &b) {
        const Line line = here() + 1;
        b.instances([](auto & /*body*/) {});
        return line;
      },
      "instances");
}

// Read-time errors

void a_failed_check_names_the_check_in_its_block() {
  SpecBuilder b("row");
  auto n = b.read("n", int_);
  Line line = 0;
  b.repeat(n, [&](auto &s) {
    auto x = s.read("x", int_);
    line = here() + 1;
    s.check(x > 0, "not positive");
  });
  std::optional<iris::Error> error = read_error(b, "2 1 -1");
  if (!error)
    return;
  check(error->code() == ErrorCode::CheckFailed, "E-CHECK-FAILED");
  expect_here(error->call_site(), line);
  check(spec_line(error) == 5, "line 5");
  check(std::string_view(error->what())
            .ends_with("\ncall site: " + describe(error->call_site())),
        std::string("the display ends with the call site: ") + error->what());
}

void a_read_failure_names_the_declaration_that_read() {
  SpecBuilder b;
  const Line line = here() + 1;
  b.read("n", int_);
  std::optional<iris::Error> error = read_error(b, "word");
  if (error)
    expect_here(error->call_site(), line);
}

void a_catalog_failure_names_the_statement() {
  SpecBuilder b;
  const Line line = here() + 1;
  b.read("n", int_);
  b.instances([](auto & /*body*/) {});
  const std::optional<Spec> spec = built(b);
  if (!spec)
    return;
  std::expected<Data, iris::Error> data =
      spec->try_catalog_text("word", "input.txt");
  check(!data, "not a number");
  if (!data)
    expect_here(data.error().call_site(), line);
}

/// A rejection in a catalog is a summary in the data, not an error, so it
/// has no call site
void a_catalog_rejection_has_no_call_site() {
  SpecBuilder b;
  b.instances([](auto &s) {
    auto n = s.read("n", int_);
    s.check(n > 0, "not positive");
  });
  const std::optional<Spec> spec = built(b);
  if (!spec)
    return;
  std::expected<Data, iris::Error> data =
      spec->try_catalog_text("0\n1\n", "input.txt");
  check(data.has_value(), "the catalog reads both instances");
  if (!data)
    return;
  const std::optional<iris::ValueRef> failed = data->find("failed");
  check(failed.has_value(), "the first instance is rejected");
  if (!failed || failed->size() != 2)
    return;
  const std::string_view summary = (*failed)[0].text();
  check(summary.contains("not positive"),
        "the summary: " + std::string(summary));
  check(!summary.contains("call site"),
        "the summary has no call site: " + std::string(summary));
  check((*failed)[1].text().empty(), "the second instance is accepted");
}

void a_deferred_rejection_keeps_its_call_site() {
  SpecBuilder b;
  Line line = 0;
  b.instances([&](auto &s) {
    auto n = s.read("n", int_);
    line = here() + 1;
    s.check(n > 0);
  });
  std::optional<iris::Error> error = read_error(b, "0");
  if (!error)
    return;
  check(error->code() == ErrorCode::CheckFailed, "E-CHECK-FAILED");
  expect_here(error->call_site(), line);
}

void a_failure_without_a_statement_has_no_call_site() {
  SpecBuilder b;
  b.read("n", int_);
  std::optional<iris::Error> error = read_error(b, "1 2");
  if (!error)
    return;
  check(error->code() == ErrorCode::TrailingInput, "E-TRAILING-INPUT");
  check(!error->call_site(), "no call site");
  check(!std::string_view(error->what()).contains("call site"),
        std::string("no call site in the display: ") + error->what());
}

/// The same text, parsed and built, differs only in the call site
void a_spec_parsed_from_text_has_no_call_sites() {
  SpecBuilder b;
  auto n = b.read("n", int_);
  b.check(n > 0, "not positive");
  const std::optional<Spec> from_builder = built(b);
  if (!from_builder)
    return;
  const Spec parsed = Spec::parse(b.source(), "builder.iris");
  check(iris::detail::SpecAccess::call_sites(parsed).empty(), "no call sites");
  check(!iris::detail::SpecAccess::call_sites(*from_builder).empty(),
        "the built spec has call sites");

  const auto read = [](const Spec &spec) {
    return spec.try_read_text("0", "input.txt").error();
  };
  const iris::Error text_error = read(parsed);
  const iris::Error builder_error = read(*from_builder);
  check(!text_error.call_site(), "no call site from text");
  check(text_error.code() == builder_error.code(), "the code");
  check(text_error.message() == builder_error.message(), "the message");
  check(spec_line(text_error) == spec_line(builder_error), "the spec line");
  const auto &text_input = text_error.input_location();
  const auto &builder_input = builder_error.input_location();
  check(text_input && builder_input &&
            text_input->column == builder_input->column,
        "the input location");
  check(builder_error.call_site().has_value(), "a call site from the builder");
  check(std::string(text_error.what()) +
                "\ncall site: " + describe(builder_error.call_site()) ==
            builder_error.what(),
        std::string("the displays: ") + text_error.what() + "\nand\n" +
            builder_error.what());

  std::expected<Spec, iris::Error> rejected =
      Spec::try_parse("instances {}\nafter: int\n", "builder.iris");
  check(!rejected, "the text is rejected");
  if (rejected)
    return;
  check(!rejected.error().call_site(), "no call site");
  check(!std::string_view(rejected.error().what()).contains("call site"),
        std::string("no call site in the display: ") + rejected.error().what());
}

// The builder's own checks

void a_builder_check_names_the_statement_that_failed_it() {
  SpecBuilder b;
  b.read("x", int_);
  const Line line = here() + 1;
  b.read("x", int_);
  std::optional<BuildError> error = build_error(b);
  if (!error)
    return;
  check(error->kind() == BuildErrorKind::Redefinition, "a redefinition");
  expect_here(error->call_site(), line);
  check(std::string_view(error->what())
            .ends_with("\ncall site: " + describe(error->call_site())),
        std::string("the display ends with the call site: ") + error->what());
}

void a_handle_from_another_builder_names_the_statement_that_used_it() {
  SpecBuilder other;
  auto foreign = other.read("x", int_);
  SpecBuilder b;
  const Line line = here() + 1;
  b.let("y", foreign);
  std::optional<BuildError> error = build_error(b);
  if (!error)
    return;
  check(error->kind() == BuildErrorKind::Provenance, "a provenance error");
  expect_here(error->call_site(), line);
}

void a_handle_never_bound_names_the_call_that_made_it() {
  {
    SpecBuilder b;
    const Line line = here() + 1;
    [[maybe_unused]] const auto later = b.forward<Field<Int>>("later");
    std::optional<BuildError> error = build_error(b);
    if (error) {
      check(error->kind() == BuildErrorKind::Binding, "a binding error");
      expect_here(error->call_site(), line);
    }
  }
  {
    SpecBuilder b;
    const Line line = here() + 1;
    b.capture("n", int_);
    std::optional<BuildError> error = build_error(b);
    if (error)
      expect_here(error->call_site(), line);
  }
  {
    SpecBuilder b;
    const Line line = here() + 1;
    b.forward_record("each", "list");
    std::optional<BuildError> error = build_error(b);
    if (error)
      expect_here(error->call_site(), line);
  }
}

void a_binding_failure_names_the_definition() {
  SpecBuilder b;
  auto name = b.undefined("name");
  const Line line = here() + 1;
  b.read(name, int_);
  std::optional<BuildError> error = build_error(b);
  if (!error)
    return;
  check(error->kind() == BuildErrorKind::Binding, "a binding error");
  expect_here(error->call_site(), line);
}

void an_unprintable_value_names_the_statement_that_holds_it() {
  SpecBuilder b;
  b.read("x", int_);
  const Line line = here() + 1;
  b.let("y", lit(std::numeric_limits<double>::quiet_NaN()));
  std::optional<BuildError> error = build_error(b);
  if (!error)
    return;
  check(error->kind() == BuildErrorKind::Unprintable, "an unprintable value");
  expect_here(error->call_site(), line);
}

/// The C++ guard is the branch that `if_` returns
void a_guard_used_late_names_the_guard_call() {
  SpecBuilder b;
  auto n = b.read("n", int_);
  auto branch = b.if_(n > 0, [](auto & /*then*/) {});
  b.read("m", int_);
  const Line line = here() + 1;
  branch.else_([](auto & /*otherwise*/) {});
  std::optional<BuildError> error = build_error(b);
  if (!error)
    return;
  check(error->kind() == BuildErrorKind::Scope, "a scope error");
  expect_here(error->call_site(), line);
}

/// A helper's statements name the helper's line unless it takes the call
/// site of its caller, which C++ passes on as a defaulted argument
void read_x(Scope<> &s, Line &line) {
  line = here() + 1;
  s.read("x", int_);
}
void read_x_for(Scope<> &s,
                std::source_location site = std::source_location::current()) {
  s.read("x", int_, site);
}

void a_helper_records_its_own_line_unless_it_passes_its_caller_on() {
  {
    SpecBuilder b;
    Line line = 0;
    read_x(b, line);
    std::optional<iris::Error> error = read_error(b, "word");
    if (error)
      expect_here(error->call_site(), line);
  }
  {
    SpecBuilder b;
    const Line line = here() + 1;
    read_x_for(b);
    std::optional<iris::Error> error = read_error(b, "word");
    if (error)
      expect_here(error->call_site(), line);
  }
}

// The call site of every statement

/// Records what each statement's text starts with and the line of its call,
/// in text order
class Expected {
public:
  /// Expect the statement on the next line to print text starting with
  /// `start`
  void next(std::string_view start,
            std::source_location where = std::source_location::current()) {
    sites_.emplace_back(start, where.line() + 1);
  }

  /// Check each statement's call site in the text of `spec`
  void check_sites(
      const Spec &spec, std::string_view text,
      std::source_location where = std::source_location::current()) const {
    const iris::detail::CallSites &sites =
        iris::detail::SpecAccess::call_sites(spec);
    std::size_t cursor = 0;
    for (const auto &[start, line] : sites_) {
      const std::size_t offset = text.find(start, cursor);
      if (offset == std::string_view::npos) {
        check(false,
              "no `" + std::string(start) + "` after byte " +
                  std::to_string(cursor) + " of:\n" + std::string(text),
              where);
        return;
      }
      const std::size_t number =
          1 + static_cast<std::size_t>(
                  std::count(text.begin(), text.begin() + offset, '\n'));
      const std::optional<std::source_location> site = sites.at(number);
      check(site && std::string_view(site->file_name()) == this_file() &&
                site->line() == line,
            "`" + std::string(start) + "` on emitted line " +
                std::to_string(number) + " has the call site " +
                describe(site) + ", not line " + std::to_string(line) +
                ", in:\n" + std::string(text),
            where);
      cursor = offset + start.size();
    }
  }

private:
  std::vector<std::pair<std::string_view, Line>> sites_;
};

/// Check the call sites of the spec `b` builds against `expected`
void expect_sites(
    const SpecBuilder &b, const Expected &expected,
    std::source_location where = std::source_location::current()) {
  const std::optional<Spec> spec = built(b, where);
  if (spec)
    expected.check_sites(*spec, b.source(), where);
}

void every_setting_and_plain_statement_records_its_call() {
  Expected e;
  e.next("format first");
  SpecBuilder b("first");
  e.next("format second");
  b.format("second");
  e.next("comment");
  b.comment("#");
  e.next("stop");
  b.stop("END");
  e.next("tokens");
  b.tokens(":");
  e.next("separators");
  b.separators(",");
  e.next("ignore trailing");
  b.ignore_trailing();
  e.next("param p");
  b.param("p");
  e.next("param q");
  b.param("q", 1);
  e.next("type t");
  b.define_type("t", int_);
  e.next("n: int");
  auto n = b.read("n", int_);
  e.next("list l");
  b.list("l", int_.until(0));
  e.next("let v");
  auto v = b.let("v", 1);
  e.next("let v");
  b.set(v, 2);
  e.next("check");
  b.check(n >= 0);
  e.next("expect");
  b.expect("word");
  e.next("skip line");
  b.skip_line();
  e.next("skip 1 lines");
  b.skip_lines(1);
  auto m = b.capture("m", word);
  e.next("line");
  b.line(pattern("m {}", m));
  e.next("find");
  b.find("x");
  e.next("index");
  b.index(0);
  expect_sites(b, e);
}

void every_block_statement_records_its_call() {
  Expected e;
  SpecBuilder b;
  e.next("n: int");
  auto n = b.read("n", int_);
  e.next("if n");
  auto branch = b.if_(n > 0, [&](auto &s) {
    e.next("a: int");
    s.read("a", int_);
  });
  e.next("else if n");
  branch.else_if(n < 0, [&](auto &s) {
    e.next("b: int");
    s.read("b", int_);
  });
  e.next("else");
  branch.else_([&](auto &s) {
    e.next("c: int");
    s.read("c", int_);
  });
  e.next("repeat 2");
  b.repeat(2, [&](auto &s) {
    e.next("d: int");
    s.read("d", int_);
  });
  e.next("repeat until \"END\"");
  b.repeat_until("END", [&](auto &s) {
    e.next("f: int");
    s.read("f", int_);
  });
  e.next("sections");
  b.sections([&](auto &arms) {
    e.next("\"one\"");
    arms.arm("one", [&](auto &s) {
      e.next("h: int");
      s.read("h", int_);
    });
  });
  e.next("repeat until eof");
  b.repeat_until_eof([&](auto &s) {
    e.next("i: float");
    s.read("i", float_);
  });
  e.next("instances");
  b.instances([&](auto &s) {
    e.next("j: int");
    s.read("j", int_);
  });
  expect_sites(b, e);
}

/// Call sites are mapped by emitted line, so the branches have bodies: an
/// empty block keeps the next branch on its line
void the_guards_of_if_record_their_calls() {
  Expected e;
  SpecBuilder b;
  auto n = b.read("n", int_);
  auto branch = b.if_(n > 0, [](auto &s) { s.read("a", int_); });
  e.next("else if n");
  branch.else_if(n < 0, [](auto &s) { s.read("b", int_); });
  e.next("else if:");
  branch.else_read_if("if", int_);
  check(b.source() ==
            "n: int\nif n > 0 {\n  a: int\n} else if n < 0 {\n  b: int\n} "
            "else if: int\n",
        "the text");
  expect_sites(b, e);
}

void a_closing_brace_belongs_to_the_call_that_wrote_the_block() {
  Expected e;
  SpecBuilder b;
  e.next("}");
  b.repeat(2, [](auto &s) { s.read("x", int_); });
  b.read("y", int_);
  check(b.source() == "repeat 2 {\n  x: int\n}\ny: int\n", "the text");
  expect_sites(b, e);
}

void an_empty_block_gives_its_brace_to_the_call_that_wrote_it() {
  Expected e;
  SpecBuilder b;
  e.next("repeat until");
  b.repeat_until("END", [](auto & /*body*/) {});
  check(b.source() == "repeat until \"END\" {}\n", "the text");
  expect_sites(b, e);
}

// Errors that lie past their statement

/// Return line `number` of `text`, counted from 1, without its indentation
std::string_view statement_on(std::string_view text, std::size_t number) {
  std::size_t start = 0;
  for (std::size_t k = 1; k < number && start < text.size(); k++)
    start = std::min(text.find('\n', start), text.size()) + 1;
  if (start >= text.size())
    return {};
  std::string_view line = text.substr(start, text.find('\n', start) - start);
  line.remove_prefix(std::min(line.find_first_not_of(' '), line.size()));
  return line;
}

/// A declaration's form is checked at the token after it, which may start
/// another statement. The location stays there; the call site does not.
void expect_owner(
    const SpecBuilder &b, Line line, std::size_t emitted_line,
    std::optional<std::string_view> emitted,
    std::source_location where = std::source_location::current()) {
  std::optional<BuildError> error = build_error(b, where);
  if (!error || !error->language_error() || !error->text())
    return;
  const iris::Error &spec = *error->language_error();
  check(spec.code() == ErrorCode::SpecType, "E-SPEC-TYPE", where);
  check(spec.message() == "'rest' values cannot be arrays", "the message",
        where);
  expect_here(error->call_site(), line, where);
  expect_here(spec.call_site(), line, where);
  check(spec_line(spec) == emitted_line,
        "emitted line " + std::to_string(emitted_line), where);
  check(error->emitted_line() == emitted,
        "the emitted line " + std::string(emitted.value_or("(none)")), where);

  // The owner is the line of the declaration, in the text alone too
  const std::string &text = *error->text();
  const std::optional<std::size_t> owner =
      iris::detail::ErrorAccess::owner_line(spec);
  check(owner.has_value(), "the error names its statement", where);
  if (!owner)
    return;
  check(statement_on(text, *owner).starts_with("bad[2]: rest"),
        "the owner is the declaration of bad, not line " +
            std::to_string(*owner),
        where);
  std::expected<Spec, iris::Error> parsed =
      Spec::try_parse(text, "builder.iris");
  check(!parsed, "the text is rejected", where);
  if (parsed)
    return;
  const iris::Error &from_text = parsed.error();
  check(!from_text.call_site(), "no call site from text", where);
  check(iris::detail::ErrorAccess::owner_line(from_text) == owner,
        "the same owner from text", where);
  check(from_text.code() == spec.code(), "the same code", where);
  check(from_text.message() == spec.message(), "the same message", where);
  check(spec_line(from_text) == spec_line(spec), "the same line", where);
}

void an_error_on_the_next_statement_names_the_statement_it_belongs_to() {
  SpecBuilder b;
  const Line line = here() + 1;
  b.read("bad", rest.dim(2));
  b.read("next", int_);
  expect_owner(b, line, 2, "next: int");
}

void an_error_at_the_end_of_a_nested_block_names_the_statement_in_it() {
  SpecBuilder b;
  Line line = 0;
  b.repeat(1, [&](auto &outer) {
    outer.repeat(1, [&](auto &s) {
      line = here() + 1;
      s.read("bad", rest.dim(2));
    });
  });
  expect_owner(b, line, 4, "  }");
}

void an_error_at_the_end_of_an_else_block_names_the_statement_in_it() {
  SpecBuilder b;
  Line line = 0;
  auto branch = b.if_(lit(1) == 1, [](auto & /*then*/) {});
  branch.else_([&](auto &s) {
    line = here() + 1;
    s.read("bad", rest.dim(2));
  });
  expect_owner(b, line, 3, "}");
}

void an_error_at_the_end_of_the_spec_names_the_last_statement() {
  SpecBuilder b;
  b.read("n", int_);
  const Line line = here() + 1;
  b.read("bad", rest.dim(2));
  expect_owner(b, line, 3, std::nullopt);
}

/// `if 1 == 1 { ... }` nested `depth` levels below `s`, with a read at the
/// bottom; sets the line of the read and of the innermost `if`
template <class S> void nest(S &s, int depth, Line &read, Line &block) {
  if (depth == 0) {
    read = here() + 1;
    s.read("x", int_);
    return;
  }
  block = here() + 1;
  s.if_(lit(1) == 1, [&](auto &inner) { nest(inner, depth - 1, read, block); });
}

void an_error_after_a_brace_names_the_block_that_opened_it() {
  SpecBuilder b;
  Line read = 0;
  Line block = 0;
  b.if_(lit(1) == 1,
        [&](Scope<0, Presence::Maybe> &s) { nest(s, 256, read, block); });
  std::optional<BuildError> error = build_error(b);
  if (!error || !error->language_error())
    return;
  check(error->language_error()->code() == ErrorCode::SpecNesting,
        "E-SPEC-NESTING");
  std::string_view emitted = error->emitted_line().value_or("");
  emitted.remove_prefix(
      std::min(emitted.find_first_not_of(' '), emitted.size()));
  check(emitted == "x: int", "the emitted line");
  check(read != block, "the read and the block are on different lines");
  expect_here(error->call_site(), block);
}

void an_error_in_an_arm_names_the_arm() {
  SpecBuilder b;
  auto cap = b.capture("bad", alphabet("ab"));
  Line line = 0;
  b.sections([&](auto &arms) {
    line = here() + 1;
    arms.arm(pattern("{}", cap));
  });
  std::optional<iris::Error> error = spec_error(b);
  if (!error)
    return;
  check(error->code() == ErrorCode::SpecPattern, "E-SPEC-PATTERN");
  expect_here(error->call_site(), line);
}

void a_resolution_error_in_an_arm_names_the_arm() {
  SpecBuilder b;
  auto outside = b.read("outside", int_);
  auto record = b.repeat(2, [](auto &s) {
    s.each_name("row");
    s.read("inside", int_);
    return s.record();
  });
  auto cap = b.capture("cap", Range{.from = 0, .below = record.get(outside)});
  Line line = 0;
  b.sections([&](auto &arms) {
    line = here() + 1;
    arms.arm(pattern("{}", cap));
  });
  std::optional<BuildError> error = build_error(b);
  if (!error || !error->language_error())
    return;
  check(error->language_error()->code() == ErrorCode::SpecRecordScope,
        "E-SPEC-RECORD-SCOPE");
  check(error->emitted_line() == "sections {", "the emitted line");
  expect_here(error->call_site(), line);
}

void a_store_failure_names_the_statement_that_stores() {
  SpecBuilder b;
  auto stored = b.forward<Field<Int>>("stored");
  const Line line = here() + 1;
  b.repeat_until("END", {.store = stored}, [](auto &s) { s.read("x", int_); });
  std::optional<BuildError> error = build_error(b);
  if (!error)
    return;
  check(error->kind() == BuildErrorKind::Binding, "a binding error");
  expect_here(error->call_site(), line);
}

// Read-time errors that lie past their statement

void a_repeat_that_reads_nothing_names_the_repeat() {
  SpecBuilder b;
  const Line line = here() + 1;
  b.repeat_until_eof([](auto &s) { s.let("x", 1); });
  std::optional<iris::Error> error = read_error(b, "1");
  if (!error)
    return;
  check(error->code() == ErrorCode::NoProgress, "E-NO-PROGRESS");
  check(spec_line(error) == 2, "line 2");
  expect_here(error->call_site(), line);
}

void a_column_failure_names_the_repeat() {
  SpecBuilder b;
  const Line line = here() + 1;
  b.repeat(2, [](auto &s) {
    auto x = s.read("x", int_);
    s.if_(x > 0, [](auto &then) { then.read("y", int_); });
    s.read("z", int_);
  });
  std::optional<iris::Error> error = read_error(b, "1 2 3 0 4");
  if (!error)
    return;
  check(error->code() == ErrorCode::RepeatMissing, "E-REPEAT-MISSING");
  check(spec_line(error) == 6, "line 6");
  expect_here(error->call_site(), line);
}

void a_read_failure_in_an_arm_names_the_arm() {
  SpecBuilder b;
  auto n = b.capture("n", Range{.from = 1, .to = 2});
  Line line = 0;
  b.sections([&](auto &arms) {
    line = here() + 1;
    arms.arm(pattern("n = {}", n));
  });
  std::optional<iris::Error> error = read_error(b, "n = 3\n");
  if (!error)
    return;
  check(error->code() == ErrorCode::ExpectedValue, "E-EXPECTED-VALUE");
  check(spec_line(error) == 1, "line 1");
  expect_here(error->call_site(), line);
}

void a_line_a_last_arm_left_unread_names_its_arm() {
  SpecBuilder b;
  Line line = 0;
  b.sections([&](auto &arms) {
    line = here() + 1;
    arms.arm("needed");
    arms.arm("end", {.last = true}, [](auto &s) { s.read("x", int_); });
  });
  std::optional<iris::Error> error = read_error(b, "end\n1\n");
  if (!error)
    return;
  check(error->code() == ErrorCode::LineMatch, "E-LINE-MATCH");
  check(spec_line(error) == 4, "line 4");
  expect_here(error->call_site(), line);
}

void a_line_no_arm_matches_names_the_sections() {
  SpecBuilder b;
  const Line line = here() + 1;
  b.sections([](auto &arms) { arms.arm("known"); });
  std::optional<iris::Error> error = read_error(b, "other\n");
  if (!error)
    return;
  check(error->code() == ErrorCode::SectionsUnexpected,
        "E-SECTIONS-UNEXPECTED");
  expect_here(error->call_site(), line);
}

void a_failure_after_sections_end_has_no_call_site() {
  SpecBuilder b;
  b.sections([](auto &arms) { arms.arm("end", {.last = true}); });
  std::optional<iris::Error> error = read_error(b, "end\nmore");
  if (!error)
    return;
  check(error->code() == ErrorCode::TrailingInput, "E-TRAILING-INPUT");
  check(!error->call_site(), "no call site");
}

} // namespace

int main() {
  try {
    a_parse_error_names_the_statement_after_instances();
    a_resolution_error_names_the_statement_that_uses_the_name();
    the_display_shows_the_code_message_emitted_line_and_call_site();
    every_statement_names_its_call();
    a_failed_check_names_the_check_in_its_block();
    a_read_failure_names_the_declaration_that_read();
    a_catalog_failure_names_the_statement();
    a_catalog_rejection_has_no_call_site();
    a_deferred_rejection_keeps_its_call_site();
    a_failure_without_a_statement_has_no_call_site();
    a_spec_parsed_from_text_has_no_call_sites();
    a_builder_check_names_the_statement_that_failed_it();
    a_handle_from_another_builder_names_the_statement_that_used_it();
    a_handle_never_bound_names_the_call_that_made_it();
    a_binding_failure_names_the_definition();
    an_unprintable_value_names_the_statement_that_holds_it();
    a_guard_used_late_names_the_guard_call();
    a_helper_records_its_own_line_unless_it_passes_its_caller_on();
    every_setting_and_plain_statement_records_its_call();
    every_block_statement_records_its_call();
    the_guards_of_if_record_their_calls();
    a_closing_brace_belongs_to_the_call_that_wrote_the_block();
    an_empty_block_gives_its_brace_to_the_call_that_wrote_it();
    an_error_on_the_next_statement_names_the_statement_it_belongs_to();
    an_error_at_the_end_of_a_nested_block_names_the_statement_in_it();
    an_error_at_the_end_of_an_else_block_names_the_statement_in_it();
    an_error_at_the_end_of_the_spec_names_the_last_statement();
    an_error_after_a_brace_names_the_block_that_opened_it();
    an_error_in_an_arm_names_the_arm();
    a_resolution_error_in_an_arm_names_the_arm();
    a_store_failure_names_the_statement_that_stores();
    a_repeat_that_reads_nothing_names_the_repeat();
    a_column_failure_names_the_repeat();
    a_read_failure_in_an_arm_names_the_arm();
    a_line_a_last_arm_left_unread_names_its_arm();
    a_line_no_arm_matches_names_the_sections();
    a_failure_after_sections_end_has_no_call_site();
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
