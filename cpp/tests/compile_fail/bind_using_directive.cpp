// `using namespace iris::build;` makes iris::build::bind visible as if it
// were declared in the global namespace, beside the socket function ::bind,
// which POSIX <sys/socket.h> and Windows <winsock2.h> declare. Unqualified
// `bind` is then ambiguous, unless `using iris::build::bind;` declares it in
// the scope of the using-directive, as the builder's documentation says.
// The file uses `bind` both ways the documentation does: in an unnamed
// namespace and in a function.

#include <iris/build.hpp>

#ifdef _WIN32
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <winsock2.h>
#else
#include <sys/socket.h>
#endif

namespace {

using namespace iris::build;
#ifndef IRIS_EXPECT_FAILURE
using iris::build::bind;
#endif

void first_word(SpecBuilder &b) {
  auto w = b.forward<Field<std::string>>("w");
  b.line(pattern("{}", bind(w, word)));
}

} // namespace

void build() {
  using namespace iris::build;
#ifndef IRIS_EXPECT_FAILURE
  using iris::build::bind;
#endif
  SpecBuilder b;
  first_word(b);
  auto n = b.forward<Field<std::int64_t>>("n");
  b.line(pattern("n {}", bind(n, int_)));
}
