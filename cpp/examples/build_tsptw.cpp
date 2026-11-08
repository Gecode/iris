#include "build_tsptw.hpp"

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
  const Tsptw problem = tsptw();
  const iris::Data data =
      problem.spec.read("formats/tsptw/cases/three-nodes/input.txt");
  std::println("{} nodes", data.get(problem.n));
  const auto dist = data.get(problem.dist);
  const std::span<const std::int64_t> ready = data.get(problem.ready);
  const std::span<const std::int64_t> due = data.get(problem.due);
  for (std::size_t node = 0; node < dist.size(); ++node) {
    std::println("node {}: open {}..{}, distances {}", node, ready[node],
                 due[node], list(dist[node]));
  }
}
