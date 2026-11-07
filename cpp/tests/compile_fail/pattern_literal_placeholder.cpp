// A string literal pattern has no placeholders: use pattern() for those.

#include <iris/build.hpp>

void build() {
  iris::build::SpecBuilder b;
#ifdef IRIS_EXPECT_FAILURE
  b.line("n = {}");
#else
  b.line("n = 1");
#endif
}
