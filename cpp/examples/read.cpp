#include <iris/iris.hpp>

#include <cstdint>
#include <print>
#include <span>

int main() {
  const iris::Spec spec = iris::Spec::parse("n:int row[n]:int", "example.iris");
  const iris::Data data = spec.read_text("3 10 20 30", "input.txt");

  std::println("{}", data.integer("n"));
  const std::span<const std::int64_t> row = data.ints("row");
  std::println("{} {} {}", row[0], row[1], row[2]);
}
