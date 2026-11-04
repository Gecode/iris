#pragma once

#include <iris/error.hpp>

#include <cstddef>
#include <optional>
#include <string>
#include <utility>
#include <vector>

namespace iris::detail {

/*
 * Errors by location profile (Specification §11.3.2)
 */

/// Throw the parse-time error `code` with `message` for line `line` of the
/// reader spec from `origin`; parse-time errors have no column (profile P)
[[noreturn]] inline void fail_at(ErrorCode code, const std::string &origin,
                                 std::size_t line, std::string message) {
  throw Error(code, std::move(message), std::nullopt,
              Location{.origin = origin, .line = line, .column = std::nullopt});
}

/// Where a read-time error happened: the input position, the reader-spec
/// line if a statement runs, and the execution context if there is one
struct ReadSite {
  Location input;
  std::optional<Location> spec;
  std::optional<std::string> context;
};

/// Return the read-time error `code` with `message` at `site` (profile R)
inline Error read_error(ErrorCode code, std::string message, ReadSite site) {
  return {code, std::move(message), std::move(site.input), std::move(site.spec),
          std::move(site.context)};
}

/// Return the failed check `message` at `site` with the witness `path`
/// (profile C)
inline Error check_error(std::string message, ReadSite site,
                         std::vector<std::size_t> path) {
  return {ErrorCode::CheckFailed,  std::move(message),
          std::move(site.input),   std::move(site.spec),
          std::move(site.context), std::move(path)};
}

/// Return the error `code` with `message` at `input`, an input location
/// alone (profile T)
inline Error input_error(ErrorCode code, std::string message, Location input) {
  return {code, std::move(message), std::move(input)};
}

/// Return the error `code` with `message` for the reader spec from `origin`
/// as a whole (profile U)
inline Error spec_error(ErrorCode code, std::string message,
                        std::string origin) {
  return {code, std::move(message), std::nullopt,
          Location{.origin = std::move(origin),
                   .line = std::nullopt,
                   .column = std::nullopt}};
}

} // namespace iris::detail
