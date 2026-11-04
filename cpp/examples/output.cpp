#include <iris/iris.hpp>

#include <print>

int main() {
  const iris::Spec spec =
      iris::Spec::load("formats/dimacs-cnf/dimacs-cnf.iris");
  const iris::Data data =
      spec.read("formats/dimacs-cnf/cases/three-variables/input.txt");

  std::print("{}", data.dzn());
}
