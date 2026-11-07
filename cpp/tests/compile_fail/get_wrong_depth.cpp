// Data::get returns the depth of the handle: a row is no single integer.

#include <iris/build.hpp>

#include <cstdint>

void read(const iris::Data &data) {
  constexpr iris::Field<std::int64_t, 1> row("row");
#ifdef IRIS_EXPECT_FAILURE
  const std::int64_t value = data.get(row);
#else
  const std::int64_t value = data.get(row)[0];
#endif
  (void)value;
}
