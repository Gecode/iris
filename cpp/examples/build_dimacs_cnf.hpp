#pragma once

#include <iris/build.hpp>

#include <cstdint>

/// A DIMACS CNF reader: the number of variables and one row of literals per
/// clause
struct DimacsCnf {
  iris::Spec spec;
  iris::Field<std::int64_t> vars;
  iris::Field<std::int64_t, 2> clause;
};

inline DimacsCnf dimacs_cnf() {
  using namespace iris::build;
  SpecBuilder b("dimacs-cnf");
  b.comment("c");
  b.stop("%"); // SATLIB files end with "%" and "0"
  auto variables = b.capture("vars", Range{.from = 0, .to = host::int_max});
  auto clauses = b.capture("clauses", Range{.from = 0});
  b.line(pattern("p cnf {} {}", variables, clauses));
  auto clause = b.repeat(clauses, [&](auto &c) {
    // The terminating 0 is not stored.
    return c.list("clause",
                  Range{.from = -variables, .to = variables}.until(0));
  });
  return {b.build(), variables, clause};
}
