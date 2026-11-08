#pragma once

#include <iris/build.hpp>

#include <cstdint>
#include <string_view>
#include <tuple>

/// A Sudoku reader: the grid size, the box size, and the cells, with 0 for a
/// blank
struct Sudoku {
  iris::Spec spec;
  iris::Field<std::int64_t> n;
  iris::Field<std::int64_t> box_width;
  iris::Field<std::int64_t> box_height;
  iris::Field<std::int64_t, 2> cells;
};

/// Cell values from 1 to 36, so `1` reads as 1 and `0` as 36
inline constexpr std::string_view sudoku_symbols =
    "123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ0";

inline Sudoku sudoku() {
  using namespace iris::build;
  SpecBuilder b("sudoku");
  auto handles = b.instances([](auto &i) {
    auto n = i.capture("n", Range{.from = 1});
    auto box_width = i.capture("box_width", Range{.from = 1});
    auto box_height = i.capture("box_height", Range{.from = 1});
    i.line(pattern("# size={} box={}x{}", n, box_width, box_height));
    auto cells =
        i.read("cells", alphabet(sudoku_symbols).blank(".").dim(n).dim(n));
    i.check(box_width * box_height == n, "boxes must tile the grid");
    i.check(max(cells) <= n, "a value is larger than the size of the grid");
    return std::tuple{n, box_width, box_height, cells};
  });
  auto [n, box_width, box_height, cells] = handles;
  return {b.build(), n, box_width, box_height, cells};
}
