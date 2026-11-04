#include <iris/iris.hpp>

#include <print>

int main() {
  const iris::Spec spec =
      iris::Spec::load("formats/dimacs-cnf/dimacs-cnf.iris");

  try {
    const iris::Data data =
        spec.read("formats/dimacs-cnf/cases/literal-out-of-range/input.txt");
    std::println("{}", data.size());
  } catch (const iris::Error &error) {
    std::println("{}", iris::code_name(error.code()));
    std::println("{}", error.message());
    const auto &location = error.input_location();
    std::println("{} {}", location->line.value(), location->column.value());
    std::println("{}", error.context().value());
    std::println("{}", error.what());
  }
}
