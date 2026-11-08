#pragma once

#include <iris/build.hpp>

#include <cstdint>
#include <source_location>
#include <string>
#include <string_view>

/// A TSPLIB reader. A file gives either an explicit distance matrix or node
/// coordinates, from which the distances follow.
struct Tsplib {
  iris::Spec spec;
  iris::Field<std::string, 0, iris::Presence::Maybe> name;
  iris::Field<std::int64_t> n;
  iris::Field<std::string, 0, iris::Presence::Maybe> edge_weight_type;
  iris::Field<std::int64_t, 2, iris::Presence::Maybe> weight;
  iris::Field<double, 1, iris::Presence::Maybe> x;
  iris::Field<double, 1, iris::Presence::Maybe> y;
};

/// The entries of one section of numbered coordinates
struct Coordinates {
  iris::build::Record nodes;
  iris::Field<double, 1, iris::Presence::Maybe> x;
  iris::Field<double, 1, iris::Presence::Maybe> y;
};

/// Whether `entry` is one of `values`: `entry == "A" || entry == "B"`
template <class... Values>
auto one_of(const iris::Field<std::string, 0, iris::Presence::Maybe> &entry,
            std::string_view first, Values... rest) {
  return ((entry == first) || ... || (entry == std::string_view(rest)));
}

/// Adds the optional section `title` of `n` lines `node x y`, with the
/// entries named `{prefix}node`, `{prefix}x` and `{prefix}y`
inline Coordinates coordinate_section(
    iris::build::Arms<> &arms, std::string_view title,
    const iris::Field<std::int64_t> &n, const std::string &prefix,
    std::source_location site = std::source_location::current()) {
  using namespace iris::build;
  return arms.optional_arm(
      Pattern().text(title),
      [&](auto &a) {
        return a.repeat(
            n,
            [&](auto &line) {
              line.read(prefix + "node", int_, site);
              return Coordinates{line.record(),
                                 line.read(prefix + "x", float_, site),
                                 line.read(prefix + "y", float_, site)};
            },
            site);
      },
      site);
}

inline Tsplib tsplib() {
  using namespace iris::build;
  SpecBuilder b("tsplib");
  b.comment("COMMENT");
  auto name = b.optional_capture("name", rest);
  auto problem_type = b.optional_capture("type", word);
  auto n = b.capture("n", Range{.from = 2, .to = 46340});
  auto edge_weight_type = b.optional_capture("edge_weight_type", word);
  auto edge_weight_format = b.optional_capture("edge_weight_format", word);
  auto node_coord_type = b.optional_capture("node_coord_type", word);
  auto display_data_type = b.optional_capture("display_data_type", word);
  auto [coordinates, weight] = b.sections({.until = "EOF"}, [&](auto &s) {
    s.optional_arm(pattern("NAME : {}", name));
    s.optional_arm(pattern("TYPE : {}", problem_type));
    s.arm(pattern("DIMENSION : {}", n));
    s.optional_arm(pattern("EDGE_WEIGHT_TYPE : {}", edge_weight_type));
    s.optional_arm(pattern("EDGE_WEIGHT_FORMAT : {}", edge_weight_format));
    s.optional_arm(pattern("NODE_COORD_TYPE : {}", node_coord_type));
    s.optional_arm(pattern("DISPLAY_DATA_TYPE : {}", display_data_type));
    auto nodes = coordinate_section(s, "NODE_COORD_SECTION", n, "");
    auto weights = s.optional_arm("EDGE_WEIGHT_SECTION", [&](auto &a) {
      a.if_(!defined(edge_weight_format),
            [&](auto &t) { t.set(edge_weight_format, "FULL_MATRIX"); });
      auto bound = host::int_max / n;
      return a.read("weight",
                    Range{.from = 0, .to = bound}.dim(n).dim(n).layout(
                        edge_weight_format));
    });
    coordinate_section(s, "DISPLAY_DATA_SECTION", n, "display_");
    return std::pair{nodes, weights};
  });
  b.if_(!defined(edge_weight_type),
        [&](auto &t) { t.set(edge_weight_type, "EXPLICIT"); });

  auto x = coordinates.x;
  auto y = coordinates.y;
  b.if_(edge_weight_type == "EXPLICIT", [&](auto &t) {
     t.check(defined(weight), "no EDGE_WEIGHT_SECTION");
     t.check(size(weight) == n, "edge weights do not match DIMENSION");
   }).else_([&](auto &e) {
    e.check(one_of(edge_weight_type, "EUC_2D", "CEIL_2D", "ATT", "GEO"),
            "EDGE_WEIGHT_TYPE must be EXPLICIT, EUC_2D, CEIL_2D, ATT, or GEO");
    e.check(defined(x), "no NODE_COORD_SECTION");
    e.check(size(coordinates.nodes.list()) == n,
            "coordinates do not match DIMENSION");
    e.check(x >= -1e9 && x <= 1e9 && y >= -1e9 && y <= 1e9,
            "coordinates must be between -1e9 and 1e9");
  });
  return {b.build(), name, n, edge_weight_type, weight, x, y};
}
