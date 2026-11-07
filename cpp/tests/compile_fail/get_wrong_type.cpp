// Data::get returns the type of the handle: a word is no integer.

#include <iris/build.hpp>

#include <cstdint>
#include <string_view>

void read(const iris::Data &data) {
#ifdef IRIS_EXPECT_FAILURE
  constexpr iris::Field<std::int64_t> x("x");
  const std::string_view value = data.get(x);
#else
  constexpr iris::Field<std::string> x("x");
  const std::string_view value = data.get(x);
#endif
  (void)value;
}
