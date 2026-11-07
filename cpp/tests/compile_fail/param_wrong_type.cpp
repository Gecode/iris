// A typed parameter handle takes a value of its type: an integer parameter
// takes no float.

#include <iris/build.hpp>
#include <iris/params.hpp>

void build() {
  iris::build::SpecBuilder b;
  auto scale = b.param("scale", 3);
  iris::Params params;
#ifdef IRIS_EXPECT_FAILURE
  params.set(scale, 1.5);
#else
  params.set(scale, 3);
#endif
}
