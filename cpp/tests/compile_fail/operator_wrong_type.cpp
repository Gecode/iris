// Arithmetic takes integers: a word is no operand of `+`.

#include <iris/build.hpp>

void build() {
  using namespace iris::build;
  SpecBuilder b;
  auto n = b.read("n", int_);
  auto w = b.read("w", word);
#ifdef IRIS_EXPECT_FAILURE
  b.let("sum", n + w);
#else
  b.let("sum", n + n);
  (void)w;
#endif
}
