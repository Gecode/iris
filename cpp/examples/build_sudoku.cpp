#include "build_sudoku.hpp"

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
  const Sudoku puzzle = sudoku();
  const iris::Data data =
      puzzle.spec.read("formats/sudoku/cases/four-by-four/input.txt");
  const std::int64_t n = data.get(puzzle.n);
  std::println("{}x{} grid with {}x{} boxes", n, n, data.get(puzzle.box_width),
               data.get(puzzle.box_height));
  for (const std::span<const std::int64_t> row : data.get(puzzle.cells))
    std::println("{}", list(row));
}
