// A pattern has an argument for each `{}`: here one is extra.

#include <iris/build.hpp>

void build() {
  using namespace iris::build;
  SpecBuilder b;
  auto x = b.capture("x", int_);
  auto y = b.capture("y", int_);
#ifdef IRIS_EXPECT_FAILURE
  b.line(pattern("{}", x, y));
#else
  b.line(pattern("{}", x));
  (void)y;
#endif
}
