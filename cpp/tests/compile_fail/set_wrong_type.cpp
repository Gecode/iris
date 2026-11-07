// `set` keeps a handle's value type: an integer handle takes no string.

#include <iris/build.hpp>

void build() {
  iris::build::SpecBuilder b;
  auto n = b.read("n", iris::build::int_);
#ifdef IRIS_EXPECT_FAILURE
  b.set(n, iris::build::lit("text"));
#else
  b.set(n, iris::build::lit(1));
#endif
}
