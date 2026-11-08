// `using namespace iris::build;` makes iris::build::pow visible as if it were
// declared in the global namespace, beside the C library's ::pow, which
// <math.h> declares everywhere and <cmath> declares on MSVC. Unqualified
// `pow` is then ambiguous, unless `using iris::build::pow;` declares it in
// the scope of the using-directive, as the builder's documentation says.
// The file uses `pow` both ways the documentation and the examples do: in an
// unnamed namespace and in a function.

#include <iris/build.hpp>

#include <cmath>
#include <math.h> // NOLINT(modernize-deprecated-headers): it declares ::pow

namespace {

using namespace iris::build;
#ifndef IRIS_EXPECT_FAILURE
using iris::build::pow;
#endif

void square(SpecBuilder &b) {
  auto n = b.read("n", int_);
  b.let("square", pow(n, 2));
}

} // namespace

void build() {
  using namespace iris::build;
#ifndef IRIS_EXPECT_FAILURE
  using iris::build::pow;
#endif
  SpecBuilder b;
  square(b);
  auto m = b.read("m", int_);
  b.let("power", pow(2, m));
}
