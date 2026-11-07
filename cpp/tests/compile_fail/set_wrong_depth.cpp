// `set` keeps a handle's depth: a scalar handle takes no row.

#include <iris/build.hpp>

void build() {
  using namespace iris::build;
  SpecBuilder b;
  auto n = b.read("n", int_);
  auto row = b.read("row", int_.dim(2));
#ifdef IRIS_EXPECT_FAILURE
  b.set(n, row);
#else
  b.set(n, row[0] + 0);
#endif
}
