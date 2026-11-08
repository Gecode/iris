#include "build_dimacs_cnf.hpp"

#include <iris/iris.hpp>

#include <cstddef>
#include <cstdint>
#include <print>
#include <span>
#include <string>

namespace {

/// `row` as `[1, 2, 3]`
std::string list(std::span<const std::int64_t> row) {
  std::string text = "[";
  for (std::size_t i = 0; i < row.size(); ++i)
    text += (i == 0 ? "" : ", ") + std::to_string(row[i]);
  return text + "]";
}

} // namespace

int main() {
  const DimacsCnf cnf = dimacs_cnf();
  const iris::Data data =
      cnf.spec.read("formats/dimacs-cnf/cases/three-variables/input.txt");
  std::println("{} variables", data.get(cnf.vars));
  for (const std::span<const std::int64_t> clause : data.get(cnf.clause))
    std::println("{}", list(clause));
}
