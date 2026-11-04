#pragma once

#include <iris/data.hpp>
#include <iris/error.hpp>
#include <iris/params.hpp>

#include <expected>
#include <filesystem>
#include <memory>
#include <memory_resource>
#include <string_view>

namespace iris {

namespace detail {
struct SpecImpl;
} // namespace detail

/**
 * A parsed and checked reader spec
 *
 * Copies share the parsed spec, which never changes, so they are cheap.
 * Reading does not change it either, so threads can read with one spec at
 * the same time.
 *
 * Every allocation the library makes for the spec, and for the data it
 * reads, comes from the memory resource given when the spec is parsed: the
 * default resource unless another is given. The resource must outlive the
 * spec and the data, and be thread-safe if threads read with the spec at
 * the same time. Errors, which leave the library, are the exception: they
 * use the global heap.
 *
 * Each operation that throws an Error has a `try_` variant that returns it
 * instead. Both still throw std::system_error for a file that cannot be
 * read, AccessError for misuse, and std::bad_alloc.
 */
class Spec {
public:
  /// Parse the reader spec `text` from `origin`, which names it in errors,
  /// and allocate from `memory`. Throws Error for every parse-time error.
  static Spec
  parse(std::string_view text, std::string_view origin = "<spec>",
        std::pmr::memory_resource &memory = *std::pmr::get_default_resource());
  /// Parse like parse(), but return the Error instead of throwing it.
  static std::expected<Spec, Error> try_parse(
      std::string_view text, std::string_view origin = "<spec>",
      std::pmr::memory_resource &memory = *std::pmr::get_default_resource());
  /// Parse the reader spec in the file at `path`, which names it in errors,
  /// and allocate from `memory`. Throws like parse(), and
  /// std::system_error if the file cannot be read.
  static Spec
  load(const std::filesystem::path &path,
       std::pmr::memory_resource &memory = *std::pmr::get_default_resource());
  /// Load like load(), but return the Error instead of throwing it.
  static std::expected<Spec, Error> try_load(
      const std::filesystem::path &path,
      std::pmr::memory_resource &memory = *std::pmr::get_default_resource());

  /// Return the format name from the `format` metadata, or an empty name.
  [[nodiscard]] std::string_view name() const;

  /// Read the input file at `path` with the parameters `params`. Throws
  /// Error for every language error, and std::system_error if the file
  /// cannot be read.
  [[nodiscard]] Data read(const std::filesystem::path &path,
                          const Params &params = {}) const;
  /// Read the input `text` from `origin`, which names it in errors, with the
  /// parameters `params`. Throws Error for every language error.
  [[nodiscard]] Data read_text(std::string_view text,
                               std::string_view origin = "<input>",
                               const Params &params = {}) const;
  /// Read like read(), but return a language error instead of throwing it.
  [[nodiscard]] std::expected<Data, Error>
  try_read(const std::filesystem::path &path, const Params &params = {}) const;
  /// Read like read_text(), but return the Error instead of throwing it.
  [[nodiscard]] std::expected<Data, Error>
  try_read_text(std::string_view text, std::string_view origin = "<input>",
                const Params &params = {}) const;

  /// Catalog the instances of the input file at `path` with the parameters
  /// `params`: read every instance and return the entries `instances`,
  /// `name` if every instance has a name, and `failed` if an instance is
  /// rejected (Specification §9.6). Throws like read().
  [[nodiscard]] Data catalog(const std::filesystem::path &path,
                             const Params &params = {}) const;
  /// Catalog the input `text` from `origin` like catalog(). Throws Error for
  /// every language error.
  [[nodiscard]] Data catalog_text(std::string_view text,
                                  std::string_view origin = "<input>",
                                  const Params &params = {}) const;
  /// Catalog like catalog(), but return a language error instead of
  /// throwing it.
  [[nodiscard]] std::expected<Data, Error>
  try_catalog(const std::filesystem::path &path,
              const Params &params = {}) const;
  /// Catalog like catalog_text(), but return the Error instead of throwing
  /// it.
  [[nodiscard]] std::expected<Data, Error>
  try_catalog_text(std::string_view text, std::string_view origin = "<input>",
                   const Params &params = {}) const;

private:
  explicit Spec(std::shared_ptr<const detail::SpecImpl> impl);

  std::shared_ptr<const detail::SpecImpl> impl_;
};

} // namespace iris
