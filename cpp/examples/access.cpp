#include <iris/iris.hpp>

#include <cstddef>
#include <cstdint>
#include <print>

int main() {
  const iris::Spec spec =
      iris::Spec::load("formats/dimacs-cnf/dimacs-cnf.iris");
  const iris::Data data =
      spec.read("formats/dimacs-cnf/cases/three-variables/input.txt");

  for (std::size_t i = 0; i < data.size(); ++i) {
    const iris::ValueRef value = data.value(i);
    std::println("{} {} {}", data.name(i), iris::type_name(value.type()),
                 value.depth());
  }
  std::println("{} {}", data.find("clause").has_value(),
               data.find("model").has_value());
  std::println("{} {}", data.size("clause"), data.size("clause", {1}));
  std::println("{}", data.integer("clause", {1, 0}));

  const iris::ValueRef clauses = data.at("clause");
  for (std::size_t i = 0; i < clauses.size(); ++i) {
    std::print("clause {}:", i);
    for (const std::int64_t literal : clauses[i].ints()) {
      std::print(" {}", literal);
    }
    std::println("");
  }

  try {
    std::println("{}", data.integer("clause"));
  } catch (const iris::AccessError &error) {
    std::println("{}", error.what());
  }
}
