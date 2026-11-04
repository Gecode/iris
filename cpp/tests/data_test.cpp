#include <iris/iris.hpp>

#include "json.hpp"

#include <array>
#include <bit>
#include <cfenv>
#include <charconv>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <exception>
#include <filesystem>
#include <iostream>
#include <limits>
#include <optional>
#include <source_location>
#include <span>
#include <string>
#include <string_view>
#include <system_error>
#include <thread>
#include <vector>

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

/// Check that `f` throws AccessError
template <class F>
void misuse(F f, std::string_view what,
            std::source_location where = std::source_location::current()) {
  try {
    (void)f();
  } catch (const iris::AccessError &) {
    return;
  }
  check(false, what, where);
}

/// Test whether `values` are `expected`
bool same(std::span<const std::int64_t> values,
          const std::vector<std::int64_t> &expected) {
  return std::vector<std::int64_t>(values.begin(), values.end()) == expected;
}

void reads_entries() {
  const iris::Spec spec =
      iris::Spec::parse("n: int\nrepeat n {list c: int until 0}\n"
                        "w: word f: float x[0]: int",
                        "spec.iris");
  const iris::Data data =
      spec.read_text("3\n1 2 0\n0\n5 0\nhello 2.5\n", "input.txt");
  check(data.size() == 5, "entries");
  check((data.name(0) == "n") && (data.name(1) == "c") &&
            (data.name(2) == "w") && (data.name(3) == "f") &&
            (data.name(4) == "x"),
        "definition order");
  check(data.integer("n") == 3, "integer");
  check(data.number("n") == 3.0, "integer as number");
  check(data.text("w") == "hello", "word");
  check(data.value(1).type() == iris::ValueType::Int, "type");
  check(iris::type_name(data.value(1).type()) == "int", "type name");
  check(data.value(1).depth() == 2, "depth");
  // A ragged array with an empty row
  check(data.size("c") == 3, "rows");
  check(same(data.ints("c", {0}), {1, 2}), "first row");
  check(data.size("c", {1}) == 0, "empty row");
  check(data.ints("c", {1}).empty(), "empty row values");
  check(data.integer("c", {2, 0}) == 5, "path");
  const iris::ValueRef c = data.at("c");
  check((c[0][1].integer() == 2) && (c.at({2, 0}).integer() == 5), "traversal");
  check((c[2].depth() == 1) && (c[2].size() == 1), "row view");
  check(data.size("x") == 0, "empty array");
  check(data.ints("x").empty(), "empty array values");
  const std::optional<iris::ValueRef> f = data.find("f");
  check(f && (f->number() == 2.5), "find");
  check(!data.find("missing"), "no entry");
  check(iris::Data().size() == 0, "empty data");
}

void reads_floats() {
  const iris::Spec spec = iris::Spec::parse("f: float g[2]: num");
  const iris::Data data = spec.read_text("2.5 1 0.5");
  check(data.number("f") == 2.5, "float");
  check(data.value(1).type() == iris::ValueType::Float, "promoted");
  const std::span<const double> g = data.floats("g");
  check((g.size() == 2) && (g[0] == 1.0) && (g[1] == 0.5), "floats");
}

void rejects_misuse() {
  const iris::Spec spec =
      iris::Spec::parse("w: word repeat 2 {list c: int until 0}");
  const iris::Data data = spec.read_text("a 1 0 0");
  misuse([&] { return data.integer("w"); }, "integer of a string");
  misuse([&] { return data.integer("missing"); }, "unknown name");
  misuse([&] { return data.at("c", {2}); }, "index out of range");
  misuse([&] { return data.ints("c"); }, "ints of depth 2");
  misuse([&] { return data.floats("c", {0}); }, "floats of ints");
  misuse([&] { return data.at("c")[1][0]; }, "element of an empty row");
  misuse([&] { return data.value(2); }, "entry out of range");
  misuse([&] { return data.name(2); }, "name out of range");
  misuse([&] { return data.at("w").size(); }, "size of a single value");
}

void passes_params() {
  iris::Params params{{"a", "1"}, {"b", "2"}};
  params.set("a", "3");
  params.set("c", "x");
  check(params.size() == 3, "params size");
  check((params.name(0) == "a") && (params.name(1) == "b") &&
            (params.name(2) == "c"),
        "first-set order");
  check(params.get("a") == "3", "replaced value");
  check(!params.get("d"), "unset name");
  misuse([&] { return params.name(3); }, "param name out of range");
  iris::Params copy = params;
  copy.set("b", "changed");
  check((params.get("b") == "2") && (copy.get("b") == "changed"),
        "copies are independent");

  const iris::Spec spec = iris::Spec::parse("param a param b=1.0 param c");
  const iris::Data data = spec.read_text("", "<input>", params);
  check(data.integer("a") == 3, "inferred integer");
  check(data.number("b") == 2.0, "float default");
  check(data.text("c") == "x", "inferred string");
}

void returns_errors() {
  const iris::Spec spec = iris::Spec::parse("x: int", "spec.iris");
  const auto failed = spec.try_read_text("\nabc", "input.txt");
  check(!failed.has_value(), "try_read_text error");
  if (!failed) {
    const iris::Error &error = failed.error();
    check(error.code() == iris::ErrorCode::ExpectedValue, "code");
    check(error.input_location() && (error.input_location()->line == 2) &&
              (error.input_location()->column == 1),
          "input location");
    check(error.spec_location() && (error.spec_location()->line == 1) &&
              !error.spec_location()->column,
          "spec line");
    check(error.context() == "x", "context");
    check(!error.record_path(), "no record path");
  }
  const auto unknown =
      spec.try_read_text("1", "input.txt", iris::Params{{"k", "1"}});
  check(!unknown && (unknown.error().code() == iris::ErrorCode::ParamUnknown) &&
            !unknown.error().input_location(),
        "unknown parameter");
  const auto read = spec.try_read_text("7", "input.txt");
  check(read && (read->integer("x") == 7), "try_read_text data");
  try {
    (void)spec.read_text("x");
    check(false, "read_text throws");
  } catch (const iris::Error &error) {
    check(error.code() == iris::ErrorCode::ExpectedValue, "thrown code");
  }
  bool thrown = false;
  try {
    (void)spec.read(std::filesystem::path("no/such/input.txt"));
  } catch (const std::system_error &) {
    thrown = true;
  }
  check(thrown, "missing file");
}

void shows_diagnostics() {
  const iris::Spec spec = iris::Spec::parse("x: int", "spec.iris");
  const auto failed = spec.try_read_text("\nabc \t\n", "input.txt");
  check(!failed && (failed.error().excerpt() == "abc"), "excerpt");
  if (!failed)
    check(std::string_view(failed.error().what()) ==
              "input.txt:2:1: " + failed.error().message() +
                  "\n     2 | abc"
                  "\n       | ^"
                  "\n  while reading: x"
                  "\n  spec: spec.iris:1",
          "diagnostic");
  const std::string long_line(120, 'y');
  const auto cut = spec.try_read_text(long_line, "input.txt");
  check(!cut && (cut.error().excerpt() == std::string(100, 'y') + "..."),
        "excerpt cut after 100 bytes");

  // An earlier rejection in the instance may explain the error
  const iris::Spec instances = iris::Spec::parse(
      "instances {name: word n: int check n > 0 \"positive\" m: int}",
      "spec.iris");
  const auto noted = instances.try_read_text("a 0 x\n", "input.txt",
                                             iris::Params{{"instance", "b"}});
  check(!noted && (noted.error().code() == iris::ErrorCode::ExpectedValue) &&
            (noted.error().earlier_rejection() == "positive"),
        "earlier rejection");
  if (!noted)
    check(
        std::string_view(noted.error().what())
            .ends_with("\n  after an earlier error in the instance: positive"),
        "earlier rejection note");
  // The rejected instance itself has no note
  const auto selected = instances.try_read_text(
      "a 0 1\n", "input.txt", iris::Params{{"instance", "a"}});
  check(!selected &&
            (selected.error().code() == iris::ErrorCode::CheckFailed) &&
            !selected.error().earlier_rejection(),
        "rejection without note");
}

void catalogs_instances() {
  const iris::Spec spec = iris::Spec::parse(
      "instances {name: word n: int check n > 0 \"positive\"}", "spec.iris");
  const iris::Data catalog = spec.catalog_text("a 1\nb 0\n", "input.txt");
  check((catalog.size() == 3) && (catalog.name(0) == "instances") &&
            (catalog.name(1) == "name") && (catalog.name(2) == "failed"),
        "catalog entries");
  check(catalog.integer("instances") == 2, "instance count");
  check((catalog.text("name", {0}) == "a") &&
            (catalog.text("name", {1}) == "b"),
        "instance names");
  check((catalog.text("failed", {0}).empty()) &&
            (catalog.text("failed", {1}) == "positive"),
        "rejection summaries");
  // The selector does not restrict a catalog
  const auto selected = spec.try_catalog_text("a 1\nb 2\n", "input.txt",
                                              iris::Params{{"instance", "b"}});
  check(selected && (selected->size() == 2) &&
            (selected->integer("instances") == 2),
        "try_catalog_text data");
  const auto failed = spec.try_catalog_text("a x\n", "input.txt");
  check(!failed && (failed.error().code() == iris::ErrorCode::ExpectedValue) &&
            (failed.error().context() == "instance [1] > n"),
        "try_catalog_text error");
  bool thrown = false;
  try {
    check(!spec.try_catalog(std::filesystem::path("no/such/input.txt")),
          "no catalog of a missing file");
  } catch (const std::system_error &) {
    thrown = true;
  }
  check(thrown, "missing catalog file");
}

void renders_dzn() {
  const iris::Spec spec =
      iris::Spec::parse("n: int x[2]: float var: word", "spec.iris");
  const iris::Data data = spec.read_text("3 0.5 20 \"q\"\n", "input.txt");
  check(data.dzn() == "n = 3;\nx = [0.5, 20.0];\n'var' = \"\\\"q\\\"\";\n",
        "dzn");
  check(iris::Data().dzn().empty(), "dzn without entries");
}

/**
 * Return the spelling of finite nonzero `f` from appendix B.3 by the
 * library's own conversions, which follow the algorithm only when rounding
 * to nearest
 */
std::string library_spelling(double f) {
  const double m = std::fabs(f);
  int precision = 1;
  double power = 10.0;
  while ((m < 1e17) && (m >= power)) {
    precision++;
    power *= 10.0;
  }
  std::array<char, 32> text{};
  for (;; precision++) {
    char *end = std::to_chars(text.data(), text.data() + text.size(), f,
                              std::chars_format::general, precision)
                    .ptr;
    double back = 0.0;
    std::from_chars(text.data(), end, back);
    if ((back == f) || (precision == 17)) {
      std::string t(text.data(), end);
      if (t.find_first_of(".e") == std::string::npos)
        t += ".0";
      return t;
    }
  }
}

/// Return the shortest decimal that reads back as `f`
std::string shortest(double f) {
  std::array<char, 32> text{};
  return {text.data(),
          std::to_chars(text.data(), text.data() + text.size(), f).ptr};
}

void renders_floats_exactly() {
  // Powers of two and ten with their neighbours, integers near 1e17, and
  // random bit patterns, all normal as input floats are
  std::vector<double> values;
  const auto add = [&values](double f) {
    for (const double x : {f, std::nextafter(f, 0.0), std::nextafter(f, 2 * f)})
      if (std::isnormal(x))
        values.push_back(x);
  };
  for (int e = -1022; e <= 1023; e++)
    add(std::ldexp(1.0, e));
  for (int e = -307; e <= 308; e++)
    add(std::stod("1e" + std::to_string(e)));
  for (const double f : {9007199254740992.0, 99999999999999984.0, 1e17, 0.3})
    add(f);
  // SplitMix64, for the same bit patterns on every platform
  std::uint64_t state = 2026;
  for (int i = 0; i < 20000; i++) {
    std::uint64_t z = (state += 0x9e3779b97f4a7c15U);
    z = (z ^ (z >> 30U)) * 0xbf58476d1ce4e5b9U;
    z = (z ^ (z >> 27U)) * 0x94d049bb133111ebU;
    const auto f = std::bit_cast<double>(z ^ (z >> 31U));
    if (std::isnormal(f))
      values.push_back(f);
  }
  std::string input;
  for (const double f : values)
    input += shortest(f) + '\n';
  const std::string spec = "f[" + std::to_string(values.size()) + "]: float";
  const std::pmr::string dzn = iris::Spec::parse(spec).read_text(input).dzn();
  // The leaves between "f = [" and "];\n"
  std::string_view rest = std::string_view(dzn).substr(5);
  for (const double f : values) {
    const std::size_t end = rest.find_first_of(",]");
    if (rest.substr(0, end) != library_spelling(f)) {
      check(false, "float spelling of " + shortest(f) + ": " +
                       std::string(rest.substr(0, end)));
      return;
    }
    rest.remove_prefix(rest[end] == ',' ? end + 2 : end);
  }
  check(rest == "];\n", "all floats spelled");
}

/// Set the rounding mode for the lifetime of this object
class RoundingMode {
public:
  explicit RoundingMode(int mode) { std::fesetround(mode); }
  ~RoundingMode() { std::fesetround(FE_TONEAREST); }
  RoundingMode(const RoundingMode &) = delete;
  RoundingMode(RoundingMode &&) = delete;
  RoundingMode &operator=(const RoundingMode &) = delete;
  RoundingMode &operator=(RoundingMode &&) = delete;
};

void ignores_rounding_mode() {
  // Values whose conversions depend on the rounding mode: decimals, an
  // offset, integers beyond 2^53, and float spellings
  const std::string_view spec_text =
      "let f=0.1 g[6]: float h: float offset 1 n[2]: num";
  const std::string_view input =
      "0.3333333333333333 1e23 123456.789 2.2250738585072014e-308 "
      "1.7976931348623157e308 9007199254740993\n0.1\n0.5 9007199254740993\n";
  const std::string expected =
      "f = 0.1;\n"
      "g = [0.3333333333333333, 1e+23, 123456.789, 2.2250738585072014e-308, "
      "1.7976931348623157e+308, 9007199254740992.0];\n"
      "h = 1.1;\n"
      "n = [0.5, 9007199254740992.0];\n";
  const iris::Data nearest = iris::Spec::parse(spec_text).read_text(input);
  check(nearest.dzn() == std::string_view(expected),
        "dzn when rounding to nearest");
  const std::string json = iris::driver::encode_data(nearest);
  check(iris::driver::encode_float(0.1) == "0.1", "JSON float");
  for (const auto &[mode, name] :
       {std::pair{FE_DOWNWARD, "downward"}, std::pair{FE_UPWARD, "upward"},
        std::pair{FE_TOWARDZERO, "toward zero"}}) {
    const RoundingMode rounding(mode);
    const std::string when = std::string(" when rounding ") + name;
    check(nearest.dzn() == std::string_view(expected), "dzn" + when);
    check(iris::driver::encode_data(nearest) == json, "JSON" + when);
    // Reading leaves the caller's environment, flags included, as it was
    std::feclearexcept(FE_ALL_EXCEPT);
    const iris::Data read = iris::Spec::parse(spec_text).read_text(input);
    check(std::fegetround() == mode, "rounding mode restored" + when);
    check(std::fetestexcept(FE_ALL_EXCEPT) == 0, "no flags raised" + when);
    check(read.dzn() == std::string_view(expected), "dzn of data read" + when);
    check(iris::driver::encode_data(read) == json, "JSON of data read" + when);
    check(std::fegetround() == mode, "rounding mode kept" + when);
  }
}

void renders_deep_arrays() {
  // Deep arrays are valid, so rendering them must not exhaust the stack
  constexpr std::size_t depth = 30000;
  std::string spec = "x";
  for (std::size_t i = 0; i < depth; i++)
    spec += "[1]";
  spec += ": int";
  const iris::Data data = iris::Spec::parse(spec).read_text("1");
  std::string dzn;
  std::string json;
  // A thread of its own has less stack than the main thread
  std::thread([&] {
    dzn = data.dzn();
    json = iris::driver::encode_data(data);
  }).join();
  std::string expected = "% x: ";
  for (std::size_t i = 1; i < depth; i++)
    expected += "array[int] of tuple(";
  expected += "array[int] of int" + std::string(depth - 1, ')') + "\nx = ";
  for (std::size_t i = 1; i < depth; i++)
    expected += "[(";
  expected += "[1]";
  for (std::size_t i = 1; i < depth; i++)
    expected += ",)]";
  check(dzn == expected + ";\n", "dzn of a deep array");
  check(json ==
            R"({"iris":"0.1","entries":[{"name":"x","type":"int","depth":)" +
                std::to_string(depth) +
                ",\"value\":" + std::string(depth, '[') + "1" +
                std::string(depth, ']') + "}]}\n",
        "JSON of a deep array");
}

} // namespace

int main() {
  try {
    reads_entries();
    reads_floats();
    rejects_misuse();
    passes_params();
    returns_errors();
    shows_diagnostics();
    catalogs_instances();
    renders_dzn();
    renders_floats_exactly();
    ignores_rounding_mode();
    renders_deep_arrays();
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
