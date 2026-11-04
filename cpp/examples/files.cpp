#include <iris/iris.hpp>

#include <filesystem>
#include <print>

int main() {
  const iris::Spec spec = iris::Spec::load("formats/jobshop/jobshop.iris");
  const std::filesystem::path input =
      "formats/jobshop/cases/or-library/input.txt";

  const iris::Data catalog = spec.catalog(input);
  std::print("{}", catalog.dzn());

  const iris::Params params{{"instance", "handmade-or"}};
  const iris::Data data = spec.read(input, params);
  std::println("{} {}", data.integer("n_jobs"), data.integer("n_machines"));
}
