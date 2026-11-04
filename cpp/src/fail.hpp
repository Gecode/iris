#pragma once

#include "input.hpp"
#include "memory.hpp"

#include <iris/error.hpp>

#include <cstddef>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace iris::detail {

/*
 * Errors by location profile (Specification §11.3.2)
 *
 * An Error holds standard strings on the global heap (see memory.hpp), so
 * these functions take the library's strings as views and copy them.
 */

/// Throw the parse-time error `code` with `message` for line `line` of the
/// reader spec from `origin`; parse-time errors have no column (profile P)
[[noreturn]] inline void fail_at(ErrorCode code, std::string_view origin,
                                 std::size_t line, std::string_view message) {
  throw Error(code, std::string(message),
              {.spec_location = Location{
                   .origin = std::string(origin), .line = line, .column = {}}});
}

/// Where a read-time error happened: the input position and its excerpt,
/// the reader-spec line if a statement runs, and the execution context if
/// there is one
struct ReadSite {
  Location input;
  std::string excerpt;
  std::optional<Location> spec;
  std::optional<std::string> context;
};

/// Return the read-time error `code` with `message` at `site` (profile R)
inline Error read_error(ErrorCode code, std::string_view message,
                        ReadSite site) {
  return {code,
          std::string(message),
          {.input_location = std::move(site.input),
           .spec_location = std::move(site.spec),
           .excerpt = std::move(site.excerpt),
           .context = std::move(site.context)}};
}

/// Return the failed check `message` at `site` with the witness `path`
/// (profile C)
inline Error check_error(std::string_view message, ReadSite site,
                         const Seq<std::size_t> &path) {
  return {ErrorCode::CheckFailed,
          std::string(message),
          {.input_location = std::move(site.input),
           .spec_location = std::move(site.spec),
           .excerpt = std::move(site.excerpt),
           .context = std::move(site.context),
           .record_path = std::vector<std::size_t>(path.begin(), path.end())}};
}

/// Return the error `code` with `message` at the position of `in`, an
/// input location alone (profile T)
inline Error input_error(ErrorCode code, std::string_view message,
                         const Input &in) {
  return {code,
          std::string(message),
          {.input_location = Location{.origin = std::string(in.origin),
                                      .line = in.line,
                                      .column = in.here().column()},
           .excerpt = in.excerpt(in.start)}};
}

/// Return the error `code` with `message` for the reader spec from `origin`
/// as a whole (profile U)
inline Error spec_error(ErrorCode code, std::string_view message,
                        std::string_view origin) {
  return {code,
          std::string(message),
          {.spec_location = Location{
               .origin = std::string(origin), .line = {}, .column = {}}}};
}

/// Return `error` with the note that `summary` rejected the instance
/// earlier
inline Error after_rejection(const Error &error, std::string_view summary) {
  ErrorDetails details = error.details();
  details.earlier_rejection = std::string(summary);
  return {error.code(), error.message(), std::move(details)};
}

} // namespace iris::detail
