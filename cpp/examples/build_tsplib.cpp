#include "build_tsplib.hpp"

#include <iris/iris.hpp>

#include <cstddef>
#include <format>
#include <print>
#include <span>
#include <string>

namespace {

/// `row` as `[1, 2, 3]`
template <class T> std::string list(std::span<const T> row) {
  std::string text = "[";
  for (std::size_t i = 0; i < row.size(); ++i)
    text += std::format("{}{}", i == 0 ? "" : ", ", row[i]);
  return text + "]";
}

} // namespace

int main() {
  const Tsplib problem = tsplib();
  for (const char *example : {"explicit-five", "euc-2d-five"}) {
    const iris::Data data = problem.spec.read(
        std::format("formats/tsplib/cases/{}/input.txt", example));
    std::println(
        "{}: {} cities, {}", data.get(problem.name).value_or("unnamed"),
        data.get(problem.n), data.get(problem.edge_weight_type).value_or("?"));
    if (const auto weight = data.get(problem.weight)) {
      for (const std::span<const std::int64_t> row : *weight)
        std::println("  {}", list(row));
    }
    const auto x = data.get(problem.x);
    const auto y = data.get(problem.y);
    if (x && y)
      std::println("  x {}\n  y {}", list(*x), list(*y));
  }
}
