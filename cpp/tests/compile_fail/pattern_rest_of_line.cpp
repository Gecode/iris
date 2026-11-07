// `...` in a pattern is spelled rest_of_line, never as text.

#include <iris/build.hpp>

void build() {
  using namespace iris::build;
  SpecBuilder b;
#ifdef IRIS_EXPECT_FAILURE
  b.line(pattern("a ... b"));
#else
  b.line(pattern("a {} b", rest_of_line));
#endif
}
