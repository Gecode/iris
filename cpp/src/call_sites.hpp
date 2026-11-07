#pragma once

// Where the statements of a reader spec from a spec builder came from in the
// host program, and the errors that name them.

#include "memory.hpp"

#include <iris/error.hpp>
#include <iris/spec.hpp>

#include <cstddef>
#include <expected>
#include <optional>
#include <source_location>
#include <string>
#include <string_view>
#include <utility>

namespace iris::detail {

/// Access to the private parts of Error
struct ErrorAccess {
  /// Return the reader-spec line of the statement that `error` belongs to,
  /// if the error records it
  static std::optional<std::size_t> owner_line(const Error &error) {
    return error.owner_line_;
  }
  /// Record that `error` belongs to the statement on reader-spec line
  /// `line`
  static void set_owner_line(Error &error, std::optional<std::size_t> line) {
    error.owner_line_ = line;
  }
};

/// Return `text` with its control characters escaped as Rust's
/// `escape_default` escapes them, so that a terminal does not act on them:
/// emitted strings keep some control characters raw, and a file name can
/// hold them too
std::string escape_controls(std::string_view text);

/// Return `file:line:column` of `site`, with its control characters escaped
std::string site_text(const std::source_location &site);

/**
 * The call site of each line of a reader spec from a spec builder
 *
 * A spec builder prints one statement per line, so a line names the
 * statement that starts on it, or for a line of `}` alone the call that
 * wrote the block. A spec parsed from text has no call sites.
 */
class CallSites {
public:
  /// No call sites, as for a spec parsed from text
  CallSites() = default;
  /// The call site of each line, the first line first
  explicit CallSites(Seq<std::optional<std::source_location>> lines)
      : lines_(std::move(lines)) {}

  /// Whether there are no call sites
  [[nodiscard]] bool empty() const { return lines_.empty(); }
  /// Return the call site of line `line`, counted from 1
  [[nodiscard]] std::optional<std::source_location> at(std::size_t line) const {
    if (line == 0 || line > lines_.size())
      return std::nullopt;
    return lines_[line - 1];
  }
  /// Return `error` with the call site of the statement it belongs to: the
  /// one it records as its owner, or else the one on its reader-spec line.
  /// The code, message and locations stay as they are.
  [[nodiscard]] Error locate(const Error &error) const;

private:
  Seq<std::optional<std::source_location>> lines_;
};

/// Access to the private parts of Spec, for the spec builder
struct SpecAccess {
  /// Parse `text` from `origin` like Spec::try_parse, and keep `sites`, so
  /// that every error from reading with the spec names its call site. A
  /// parse error comes back as it is, for the builder to locate.
  static std::expected<Spec, Error>
  try_parse(std::string_view text, std::string_view origin, CallSites sites);
  /// Return the call sites of `spec`
  static const CallSites &call_sites(const Spec &spec);
};

} // namespace iris::detail
