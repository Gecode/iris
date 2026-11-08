#include "build_nonogram.hpp"

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

/// `rows` as `[[1, 2], [3]]`
std::string list(const iris::Array<std::int64_t, 2> &rows) {
  std::string text = "[";
  for (std::size_t i = 0; i < rows.size(); ++i)
    text += (i == 0 ? "" : ", ") + list(rows[i]);
  return text + "]";
}

} // namespace

int main() {
  const Nonogram puzzle = nonogram();
  const iris::Data data =
      puzzle.spec.read("formats/nonogram/cases/five-by-five/input.txt");
  std::println("{}x{} board", data.get(puzzle.width), data.get(puzzle.height));
  std::println("rows: {}", list(data.get(puzzle.row)));
  std::println("columns: {}", list(data.get(puzzle.col)));
}
