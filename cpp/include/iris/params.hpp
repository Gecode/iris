#pragma once

#include <cstddef>
#include <initializer_list>
#include <memory>
#include <optional>
#include <string>
#include <string_view>
#include <utility>

namespace iris {

namespace detail {
struct ParamsImpl;
} // namespace detail

class Spec;

/**
 * Values for the `param` statements of a reader spec, as name and value
 * strings in the order they were first set (Specification §9.1)
 *
 * Copies are cheap: they share the values until one of them changes.
 */
class Params {
public:
  /// No values
  Params();
  /// The values `pairs`, set in order
  Params(std::initializer_list<std::pair<std::string, std::string>> pairs);

  /// Set `name` to `value`. A name that is already set keeps its place.
  void set(std::string name, std::string value);

  /// Return the number of names set
  [[nodiscard]] std::size_t size() const;
  /// Return name `i`, in the order of first setting; throws AccessError if
  /// there are not that many names
  [[nodiscard]] std::string_view name(std::size_t i) const;
  /// Return the value of `name`, or nothing if it is not set
  [[nodiscard]] std::optional<std::string_view>
  get(std::string_view name) const;

private:
  friend class Spec;
  std::shared_ptr<detail::ParamsImpl> impl_;
};

} // namespace iris
