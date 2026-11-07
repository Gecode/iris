#pragma once

#include "call_sites.hpp"
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

/// Throw the parse-time error `code` with `message` for line `line` of the
/// reader spec from `origin`, which belongs to the statement on line
/// `owner` (profile P)
[[noreturn]] inline void fail_in(ErrorCode code, std::string_view origin,
                                 std::size_t line, std::size_t owner,
                                 std::string_view message) {
  Error error(code, std::string(message),
              {.spec_location = Location{
                   .origin = std::string(origin), .line = line, .column = {}}});
  ErrorAccess::set_owner_line(error, owner);
  throw error;
}

/// Where a read-time error happened: the input position and its excerpt,
/// the reader-spec line if a statement runs, the execution context if
/// there is one, and the line of the statement the error belongs to if it
/// is not the one that runs
struct ReadSite {
  Location input;
  std::string excerpt;
  std::optional<Location> spec;
  std::optional<std::string> context;
  std::optional<std::size_t> owner;
};

/// Return the read-time error `code` with `message` at `site` (profile R)
inline Error read_error(ErrorCode code, std::string_view message,
                        ReadSite site) {
  Error error(code, std::string(message),
              {.input_location = std::move(site.input),
               .spec_location = std::move(site.spec),
               .excerpt = std::move(site.excerpt),
               .context = std::move(site.context)});
  ErrorAccess::set_owner_line(error, site.owner);
  return error;
}

/// Return the failed check `message` at `site` with the witness `path`
/// (profile C)
inline Error check_error(std::string_view message, ReadSite site,
                         const Seq<std::size_t> &path) {
  Error error(
      ErrorCode::CheckFailed, std::string(message),
      {.input_location = std::move(site.input),
       .spec_location = std::move(site.spec),
       .excerpt = std::move(site.excerpt),
       .context = std::move(site.context),
       .record_path = std::vector<std::size_t>(path.begin(), path.end())});
  ErrorAccess::set_owner_line(error, site.owner);
  return error;
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
  Error result(error.code(), error.message(), std::move(details));
  ErrorAccess::set_owner_line(result, ErrorAccess::owner_line(error));
  return result;
}

} // namespace iris::detail
