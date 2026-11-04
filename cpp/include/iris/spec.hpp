#pragma once

#include <iris/data.hpp>
#include <iris/error.hpp>
#include <iris/params.hpp>

#include <expected>
#include <filesystem>
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
/// Reading does not change it either, so threads can read with one spec at
/// the same time.
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

  /// Read the input file at `path` with the parameters `params`. Throws
  /// Error for every language error, and std::system_error if the file
  /// cannot be read.
  [[nodiscard]] Data read(const std::filesystem::path &path,
                          const Params &params = {}) const;
  /// Read the input `text` from `origin`, which names it in errors, with the
  /// parameters `params`. Throws Error for every language error.
  [[nodiscard]] Data read_text(std::string_view text,
                               std::string origin = "<input>",
                               const Params &params = {}) const;
  /// Read like read(), but return a language error instead of throwing it.
  /// A file that cannot be read still throws std::system_error.
  [[nodiscard]] std::expected<Data, Error>
  try_read(const std::filesystem::path &path, const Params &params = {}) const;
  /// Read like read_text(), but return the Error instead of throwing it.
  [[nodiscard]] std::expected<Data, Error>
  try_read_text(std::string_view text, std::string origin = "<input>",
                const Params &params = {}) const;

private:
  explicit Spec(std::shared_ptr<const detail::SpecImpl> impl);

  std::shared_ptr<const detail::SpecImpl> impl_;
};

} // namespace iris
