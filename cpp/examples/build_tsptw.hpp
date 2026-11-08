#pragma once

#include <iris/build.hpp>

#include <cstdint>
#include <tuple>

/// A reader for the travelling salesman problem with time windows: a distance
/// matrix and a time window for each node, with the depot as node 0
struct Tsptw {
  iris::Spec spec;
  iris::Field<std::int64_t> n;
  iris::Field<std::int64_t, 2> dist;
  iris::Field<std::int64_t, 1> ready;
  iris::Field<std::int64_t, 1> due;
};

inline Tsptw tsptw() {
  using namespace iris::build;
  SpecBuilder b("tsptw");
  b.comment("#");
  auto n = b.read("n", Range{.from = 2, .to = 1000});
  auto distance = Range{.from = 0, .to = host::int_max / 2 / n};
  auto dist = b.read("dist", distance.dim(n).dim(n));

  auto time = b.define_type("time", Range{.from = 0, .to = host::int_max / 2});
  auto [nodes, ready, due] = b.repeat(n, [&](auto &node) {
    node.as_name("nodes");
    return std::tuple{node.record(), node.read("ready", time),
                      node.read("due", time)};
  });

  b.check(nodes.get(ready) <= nodes.get(due),
          "the time window of a node is invalid");
  b.check(nodes.list()[0].get(ready) == 0,
          "the depot (node 0) must open at time 0");
  return {b.build(), n, dist, ready, due};
}
