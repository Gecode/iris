// The conformance driver of the C++ library: it answers the requests of the
// conformance runner (conformance/README.md) for parse, read, catalog, and
// dzn with the spec in the request's spec file.

#include "driver.hpp"

int main(int argc, char **argv) {
  return iris::driver::run(argc, argv, iris::driver::parse_spec_file);
}
