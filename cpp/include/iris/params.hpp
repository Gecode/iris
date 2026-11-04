#pragma once

#include <cstddef>
#include <initializer_list>
#include <memory>
#include <memory_resource>
#include <optional>
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
 * The values are allocated from the memory resource given at construction:
 * the default resource unless another is given, which must outlive the
 * values. Copies are cheap: they share the values until one of them
 * changes.
 */
class Params {
public:
  /// No values
  Params();
  /// No values, which will be allocated from `memory`
  explicit Params(std::pmr::memory_resource &memory);
  /// The values `pairs`, set in order
  Params(std::initializer_list<std::pair<std::string_view, std::string_view>>
             pairs);
  /// The values `pairs`, set in order and allocated from `memory`
  Params(std::initializer_list<std::pair<std::string_view, std::string_view>>
             pairs,
         std::pmr::memory_resource &memory);

  /// Set `name` to `value`. A name that is already set keeps its place.
  void set(std::string_view name, std::string_view value);

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
  /// Return the values, which are shared with no other Params
  detail::ParamsImpl &own();
  /// Return the values, which may be shared
  [[nodiscard]] const detail::ParamsImpl &values() const;

  std::pmr::memory_resource *memory_;
  /// The values, or null without values
  std::shared_ptr<detail::ParamsImpl> impl_;
};

} // namespace iris
