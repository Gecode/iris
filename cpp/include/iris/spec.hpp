#pragma once

#include <iris/error.hpp>

#include <expected>
#include <memory>
#include <string>
#include <string_view>

namespace iris {

namespace detail {
struct SpecImpl;
} // namespace detail

/// A parsed and checked reader spec.
///
/// Copies share the parsed spec, which never changes, so they are cheap.
class Spec {
public:
  /// Parse the reader spec `text` from `origin`, which names it in errors.
  /// Throws Error for every parse-time error.
  static Spec parse(std::string_view text, std::string origin = "<spec>");
  /// Parse like parse(), but return the Error instead of throwing it.
  static std::expected<Spec, Error> try_parse(std::string_view text,
                                              std::string origin = "<spec>");

  /// Return the format name from the `format` metadata, or an empty name.
  [[nodiscard]] const std::string &name() const;

private:
  explicit Spec(std::shared_ptr<const detail::SpecImpl> impl);

  std::shared_ptr<const detail::SpecImpl> impl_;
};

} // namespace iris
