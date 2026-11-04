// Tests of the public API for parsing reader specs. The conformance suite
// tests the language itself.

#include <iris/iris.hpp>

#include <cstdlib>
#include <exception>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <source_location>
#include <string>
#include <string_view>
#include <system_error>

namespace {

int failures = 0;

void check(bool condition, std::string_view what,
           std::source_location where = std::source_location::current()) {
  if (condition)
    return;
  failures++;
  std::cerr << where.file_name() << ':' << where.line()
            << ": check failed: " << what << '\n';
}

/// Return the error that parsing `text` from spec.iris throws
iris::Error parse_error(std::string_view text) {
  try {
    iris::Spec::parse(text, "spec.iris");
  } catch (const iris::Error &error) {
    return error;
  }
  failures++;
  std::cerr << "parsing succeeded unexpectedly: " << text << '\n';
  return {iris::ErrorCode::SpecSyntax, "no error"};
}

void parses_format_name() {
  const iris::Spec spec = iris::Spec::parse("format dimacs-cnf.v2\nx: int\n");
  check(spec.name() == "dimacs-cnf.v2", "format name");
  check(iris::Spec::parse("x: int").name().empty(), "no format name");
}

void reports_syntax_error() {
  const iris::Error error = parse_error("n: int\na[2: int\n");
  check(error.code() == iris::ErrorCode::SpecSyntax, "code");
  check(iris::code_name(error.code()) == "E-SPEC-SYNTAX", "code name");
  check(error.message() == "unknown statement 'a'", "message");
  check(!error.input_location().has_value(), "no input location");
  check(error.spec_location().has_value(), "spec location");
  if (error.spec_location()) {
    check(error.spec_location()->origin == "spec.iris", "origin");
    check(error.spec_location()->line == 2, "line");
    check(!error.spec_location()->column.has_value(), "no column");
  }
  check(!error.context().has_value(), "no context");
  check(!error.record_path().has_value(), "no record path");
  check(std::string_view(error.what()) == "spec.iris:2: unknown statement 'a'",
        "envelope");
}

void reports_invalid_utf8() {
  const iris::Error error = parse_error("x: int\n# \xff\n");
  check(error.code() == iris::ErrorCode::SpecUtf8, "UTF-8 code");
  check(iris::code_name(error.code()) == "E-SPEC-UTF8", "UTF-8 code name");
  check(error.spec_location() && (error.spec_location()->line == 2) &&
            (error.spec_location()->column == 3),
        "UTF-8 line and byte column");
  check(std::string_view(error.what()) ==
            "spec.iris:2:3: reader spec is not valid UTF-8",
        "UTF-8 envelope");
  // A truncated sequence is located at its first byte
  const iris::Error truncated = parse_error("ab\xe2\x82");
  check(truncated.spec_location() && (truncated.spec_location()->column == 3),
        "truncated sequence column");
  // Validation precedes lexing, so it wins over an earlier bad character
  const iris::Error first = parse_error("@\n\"\xc0\x80\"");
  check(first.code() == iris::ErrorCode::SpecUtf8, "UTF-8 before lexing");
}

void returns_errors_from_try_parse() {
  const auto failed = iris::Spec::try_parse("let x=@", "spec.iris");
  check(!failed.has_value(), "try_parse error");
  if (!failed)
    check(failed.error().code() == iris::ErrorCode::SpecCharacter,
          "try_parse error code");
  const auto parsed = iris::Spec::try_parse("format f x: int");
  check(parsed.has_value() && (parsed->name() == "f"), "try_parse spec");
}

void loads_files() {
  const std::filesystem::path directory =
      std::filesystem::temp_directory_path() / "iris-spec-test";
  std::filesystem::create_directories(directory);
  const std::filesystem::path good = directory / "good.iris";
  const std::filesystem::path bad = directory / "bad.iris";
  std::ofstream(good, std::ios::binary) << "format loaded\nx: int\n";
  std::ofstream(bad, std::ios::binary) << "x: int\na[2: int\n";
  check(iris::Spec::load(good).name() == "loaded", "load");
  check(iris::Spec::try_load(good).has_value(), "try_load");
  const auto failed = iris::Spec::try_load(bad);
  check(!failed && (failed.error().code() == iris::ErrorCode::SpecSyntax) &&
            failed.error().spec_location() &&
            (failed.error().spec_location()->origin == bad.string()) &&
            (failed.error().spec_location()->line == 2),
        "try_load error names the file");
  try {
    static_cast<void>(iris::Spec::load(bad));
    check(false, "load throws");
  } catch (const iris::Error &error) {
    check(error.code() == iris::ErrorCode::SpecSyntax, "load error");
  }
  bool thrown = false;
  try {
    check(!iris::Spec::try_load(directory / "missing.iris"),
          "no spec from a missing file");
  } catch (const std::system_error &) {
    thrown = true;
  }
  check(thrown, "missing spec file");
  std::filesystem::remove_all(directory);
}

void shares_copies() {
  const iris::Spec spec = iris::Spec::parse("format shared");
  iris::Spec copy = iris::Spec::parse("format other");
  copy = spec;
  check(copy.name().data() == spec.name().data(),
        "copies share the parsed spec");
}

// The float rules of §4.5 hold with every standard library's from_chars;
// no parse case covers them for reader-spec literals.
void converts_float_literals() {
  check(iris::Spec::try_parse("let x=2.2250738585072013e-308").has_value(),
        "rounds to the smallest normal");
  check(iris::Spec::try_parse("let x=0e-9999 let y=1.e2").has_value(),
        "exact zero and 1.e2");
  for (const std::string_view literal : {"5e-324", "1e-400", "1e309"})
    check(parse_error("let x=" + std::string(literal)).code() ==
              iris::ErrorCode::SpecNumber,
          literal);
}

} // namespace

int main() {
  try {
    parses_format_name();
    reports_syntax_error();
    reports_invalid_utf8();
    returns_errors_from_try_parse();
    loads_files();
    shares_copies();
    converts_float_literals();
  } catch (const std::exception &error) {
    std::cerr << "unexpected exception: " << error.what() << '\n';
    return EXIT_FAILURE;
  }
  if (failures > 0) {
    std::cerr << failures << " checks failed\n";
    return EXIT_FAILURE;
  }
  std::cout << "all checks passed\n";
  return EXIT_SUCCESS;
}
