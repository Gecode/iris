// Reads a DIMACS CNF instance with the installed or fetched library.

#include <iris/iris.hpp>

#include <cstdint>
#include <cstdlib>
#include <iostream>
#include <span>

int main() {
  const iris::Spec spec = iris::Spec::parse(R"(format dimacs-cnf
comment "c"
line "p cnf {vars:0..} {clauses:0..}"
repeat clauses {
  list clause: -vars..vars until 0
}
)",
                                            "dimacs-cnf.iris");
  const auto data =
      spec.try_read_text("c two clauses\np cnf 3 2\n1 -2 0\n2 3 0\n");
  if (!data) {
    std::cerr << data.error().what() << '\n';
    return EXIT_FAILURE;
  }
  const std::span<const std::int64_t> second = data->ints("clause", {1});
  if ((spec.name() != "dimacs-cnf") || (data->integer("vars") != 3) ||
      (second.size() != 2) || (second[1] != 3)) {
    std::cerr << "unexpected data:\n" << data->dzn();
    return EXIT_FAILURE;
  }
  std::cout << data->dzn();
  return EXIT_SUCCESS;
}
