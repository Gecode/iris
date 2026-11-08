// A conformance driver that reads with the builders of the format library in
// cpp/examples.
//
// The first argument names a format directory under formats/, such as
// dimacs-cnf; the rest is the protocol of iris-driver. The driver answers with
// the spec that the function of the same name, dimacs_cnf() in
// examples/build_dimacs_cnf.hpp, builds, in place of the case's own spec, so
// the runner judges every case of the format against the builder:
//
//   iris-format-builder-driver jobshop read SPEC INPUT

#include "build_dimacs_cnf.hpp"
#include "build_jobshop.hpp"
#include "build_nonogram.hpp"
#include "build_sudoku.hpp"
#include "build_tsplib.hpp"
#include "build_tsptw.hpp"
#include "driver.hpp"

#include <iris/error.hpp>
#include <iris/spec.hpp>

#include <exception>
#include <expected>
#include <iostream>
#include <optional>
#include <string>
#include <string_view>

namespace {

/// The spec that the builder of the format directory `format` builds
std::optional<iris::Spec> built_spec(std::string_view format) {
  if (format == "dimacs-cnf")
    return dimacs_cnf().spec;
  if (format == "jobshop")
    return jobshop().spec;
  if (format == "nonogram")
    return nonogram().spec;
  if (format == "sudoku")
    return sudoku().spec;
  if (format == "tsplib")
    return tsplib().spec;
  if (format == "tsptw")
    return tsptw().spec;
  return std::nullopt;
}

} // namespace

int main(int argc, char **argv) {
  if (argc < 2) {
    std::cerr << "iris-format-builder-driver: expected a format name\n";
    return 2;
  }
  std::optional<iris::Spec> spec;
  try {
    spec = built_spec(argv[1]);
  } catch (const std::exception &error) {
    std::cerr << "iris-format-builder-driver: " << error.what() << '\n';
    return 3;
  }
  if (!spec) {
    std::cerr << "iris-format-builder-driver: no builder for the format "
              << argv[1] << '\n';
    return 2;
  }
  return iris::driver::run(
      argc - 1, argv + 1,
      [&](const std::string & /*path*/)
          -> std::expected<iris::Spec, iris::Error> { return *spec; });
}
