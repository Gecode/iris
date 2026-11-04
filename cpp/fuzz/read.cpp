// The read fuzz target: it reads one fuzz input as a reader spec and its data
// input, then renders every result the way the conformance driver does.
//
// It splits its input as the Rust read target does: the bytes before the
// first NUL byte are the reader spec and the bytes after it are the data
// input; an input without a NUL byte is a spec with empty data. The Rust read
// corpus therefore works unchanged. Reading and cataloging use the default
// work budgets of Specification chapter 10, so a slow but legal input ends in
// E-BUDGET rather than a timeout.

#include "json.hpp"

#include <iris/spec.hpp>

#include <cstddef>
#include <cstdint>
#include <expected>
#include <string_view>

namespace {

void render(const iris::Error &error) {
  [[maybe_unused]] const std::string_view message = error.what();
  [[maybe_unused]] const auto json = iris::driver::encode_error(error);
}

void render(const std::expected<iris::Data, iris::Error> &result) {
  if (!result) {
    render(result.error());
    return;
  }
  [[maybe_unused]] const auto json = iris::driver::encode_data(*result);
  [[maybe_unused]] const auto dzn = result->dzn();
}

} // namespace

// libFuzzer fixes the name and signature of the entry point.
// NOLINTNEXTLINE(readability-identifier-naming)
extern "C" int LLVMFuzzerTestOneInput(const std::uint8_t *data,
                                      std::size_t size) {
  // Reading bytes through char is the aliasing the standard allows.
  const std::string_view bytes(reinterpret_cast<const char *>(data), size);
  const std::size_t separator = bytes.find('\0');
  const std::string_view spec_source = bytes.substr(0, separator);
  const std::string_view input = (separator == std::string_view::npos)
                                     ? std::string_view()
                                     : bytes.substr(separator + 1);
  const auto spec = iris::Spec::try_parse(spec_source, "fuzz.iris");
  if (!spec) {
    render(spec.error());
    return 0;
  }
  const iris::Params params;
  render(spec->try_read_text(input, "fuzz.txt", params));
  render(spec->try_catalog_text(input, "fuzz.txt", params));
  return 0;
}
