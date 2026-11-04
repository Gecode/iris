#pragma once

#include <iris/error.hpp>

#include <cstddef>
#include <string>
#include <utility>

namespace iris::detail {

/// Throw the parse-time error `code` with `message` for line `line` of the
/// reader spec from `origin`; parse-time errors have no column
[[noreturn]] inline void fail_at(ErrorCode code, const std::string &origin,
                                 std::size_t line, std::string message) {
  throw Error(code, std::move(message), std::nullopt,
              Location{.origin = origin, .line = line, .column = std::nullopt});
}

} // namespace iris::detail
