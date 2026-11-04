// The parse fuzz target: it parses arbitrary bytes as a reader spec and
// renders every error the way the conformance driver does. It checks the
// property that the Rust parse target checks: E-SPEC-UTF8 is reported exactly
// when the bytes are not valid UTF-8 (Specification §5.1).

#include "json.hpp"

#include <iris/spec.hpp>

#include <array>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <span>
#include <string_view>

namespace {

/// Whether `bytes` is strict UTF-8: shortest forms of scalar values only,
/// without surrogates. Written apart from the library's check, so that the
/// two check each other.
bool is_strict_utf8(std::span<const std::uint8_t> bytes) {
  std::size_t next = 0;
  while (next < bytes.size()) {
    const std::uint8_t lead = bytes[next];
    std::size_t length = 0;
    std::uint32_t scalar = 0;
    if (lead < 0x80U) {
      length = 1;
      scalar = lead;
    } else if ((lead & 0xe0U) == 0xc0U) {
      length = 2;
      scalar = lead & 0x1fU;
    } else if ((lead & 0xf0U) == 0xe0U) {
      length = 3;
      scalar = lead & 0x0fU;
    } else if ((lead & 0xf8U) == 0xf0U) {
      length = 4;
      scalar = lead & 0x07U;
    } else {
      return false;
    }
    if (bytes.size() - next < length)
      return false;
    for (std::size_t offset = 1; offset < length; ++offset) {
      const std::uint8_t continuation = bytes[next + offset];
      if ((continuation & 0xc0U) != 0x80U)
        return false;
      scalar = (scalar << 6U) | (continuation & 0x3fU);
    }
    constexpr std::array<std::uint32_t, 5> shortest_minimum{0, 0, 0x80, 0x800,
                                                            0x10000};
    if ((scalar < shortest_minimum.at(length)) || (scalar > 0x10ffffU) ||
        ((scalar >= 0xd800U) && (scalar <= 0xdfffU)))
      return false;
    next += length;
  }
  return true;
}

} // namespace

// libFuzzer fixes the name and signature of the entry point.
// NOLINTNEXTLINE(readability-identifier-naming)
extern "C" int LLVMFuzzerTestOneInput(const std::uint8_t *data,
                                      std::size_t size) {
  const std::span<const std::uint8_t> bytes(data, size);
  // Reading bytes through char is the aliasing the standard allows.
  const std::string_view source(reinterpret_cast<const char *>(data), size);
  const auto spec = iris::Spec::try_parse(source, "fuzz.iris");
  const bool rejected_utf8 =
      !spec && (spec.error().code() == iris::ErrorCode::SpecUtf8);
  if (rejected_utf8 == is_strict_utf8(bytes))
    std::abort();
  if (!spec) {
    [[maybe_unused]] const std::string_view message = spec.error().what();
    [[maybe_unused]] const auto json = iris::driver::encode_error(spec.error());
  }
  return 0;
}
