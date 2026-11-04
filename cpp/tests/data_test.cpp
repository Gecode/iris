#include <iris/iris.hpp>

#include <cstdint>
#include <cstdlib>
#include <exception>
#include <filesystem>
#include <iostream>
#include <optional>
#include <source_location>
#include <span>
#include <string_view>
#include <system_error>
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

} // namespace

int main() {
  try {
    reads_entries();
    reads_floats();
    rejects_misuse();
    passes_params();
    returns_errors();
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
