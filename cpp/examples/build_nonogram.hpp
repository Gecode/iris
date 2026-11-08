#pragma once

#include <iris/build.hpp>

#include <cstdint>
#include <source_location>
#include <string_view>
#include <utility>

/// A nonogram reader: the board size and the clues of each row and column
struct Nonogram {
  iris::Spec spec;
  iris::Field<std::int64_t> width;
  iris::Field<std::int64_t> height;
  iris::Field<std::int64_t, 2> row;
  iris::Field<std::int64_t, 2> col;
};

/// The clue lines of one side of the board
struct Clues {
  iris::build::Record lines;
  iris::Field<std::int64_t, 2> clues;
};

/// Reads `count` lines of comma-separated clues into the entry `name`. An
/// error in them names the line that calls this function.
inline Clues
clue_lines(iris::build::Scope<> &s, const iris::Field<std::int64_t> &count,
           std::string_view name,
           std::source_location site = std::source_location::current()) {
  using namespace iris::build;
  return s.repeat(
      count,
      [&](auto &line) {
        auto clues = line.list(name, Range{.from = 0}.until(eol), site);
        return Clues{line.record(), clues};
      },
      site);
}

/// A 0 clue says the line is empty, so it must stand alone
inline auto zero_stands_alone(const iris::Field<std::int64_t, 2> &clues) {
  using namespace iris::build;
  return count(clues) <= 1 || min(clues) > 0;
}

/// The clues and one gap between each pair of them fit into `length`
inline auto clues_fit(const iris::Field<std::int64_t, 2> &clues,
                      const iris::Field<std::int64_t> &length) {
  using namespace iris::build;
  return sum(clues) + count(clues) - 1 <= length;
}

inline Nonogram nonogram() {
  using namespace iris::build;
  SpecBuilder b("nonogram");
  b.separators(",");
  auto width = b.capture("width", Range{.from = 1});
  auto height = b.capture("height", Range{.from = 1});
  auto [rows, cols] = b.sections({.skip_other = true}, [&](auto &s) {
    s.arm(pattern("width {}", width));
    s.arm(pattern("height {}", height));
    return std::pair{
        s.arm("rows", [&](auto &a) { return clue_lines(a, height, "row"); }),
        s.arm("columns", [&](auto &a) { return clue_lines(a, width, "col"); })};
  });
  auto row = rows.clues;
  auto col = cols.clues;

  b.check(height <= 1'000'000 / width,
          "the board must have at most a million fields");
  b.check(size(rows.lines.list()) == height && size(cols.lines.list()) == width,
          "the numbers of rows and columns differ from the board size");
  b.check(zero_stands_alone(row), "a 0 must be the only clue of its row");
  b.check(zero_stands_alone(col), "a 0 must be the only clue of its column");
  b.check(clues_fit(row, width),
          "the clues of a row do not fit into the width of the board");
  b.check(clues_fit(col, height),
          "the clues of a column do not fit into the height of the board");
  return {b.build(), width, height, row, col};
}
