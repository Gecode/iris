// Every `{` of a pattern starts a `{}`.

#include <iris/build.hpp>

void build() {
  iris::build::SpecBuilder b;
#ifdef IRIS_EXPECT_FAILURE
  b.line("a { b");
#else
  b.line("a } b");
#endif
}
