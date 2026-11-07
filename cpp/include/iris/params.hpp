#pragma once

#include <array>
#include <charconv>
#include <concepts>
#include <cstddef>
#include <cstdint>
#include <initializer_list>
#include <limits>
#include <memory>
#include <memory_resource>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <variant>

namespace iris {

struct Dynamic;

namespace detail {
struct ParamsImpl;

/// An integer a typed parameter takes: not a bool or a character, and no
/// wider than 64 bits
template <class T>
concept ParamInteger =
    std::integral<T> && !std::same_as<T, bool> && !std::same_as<T, char> &&
    !std::same_as<T, wchar_t> && !std::same_as<T, char8_t> &&
    !std::same_as<T, char16_t> && !std::same_as<T, char32_t> &&
    sizeof(T) <= sizeof(std::int64_t);

/// A float a typed parameter takes, which converts to double exactly
template <class T>
concept ParamFloat = std::same_as<T, double> || std::same_as<T, float>;

/// A string a typed parameter takes
template <class T>
concept ParamString = std::convertible_to<const T &, std::string_view>;

/**
 * Whether a parameter handle with value type V takes `T`: an `int`
 * handle an integer, a `float` handle or a `num` handle (Number) a number,
 * a `string` handle a string, and a handle of unknown type any of them,
 * as the conversion in Specification §9.2 accepts them
 */
template <class V, class T>
concept ParamAccepts = (std::same_as<V, std::int64_t> && ParamInteger<T>) ||
                       ((std::same_as<V, double> ||
                         std::same_as<V, std::variant<std::int64_t, double>>) &&
                        (ParamInteger<T> || ParamFloat<T>)) ||
                       (std::same_as<V, std::string> && ParamString<T>) ||
                       (std::same_as<V, Dynamic> &&
                        (ParamInteger<T> || ParamFloat<T> || ParamString<T>));

/**
 * A handle a parameter can be set through: one whose entry is a single
 * value, or whose depth is not known (see Field in `<iris/handle.hpp>`).
 * A caller value always produces a single value (Specification §9.2).
 */
template <class H>
concept ParamHandle = requires(const H &handle) {
  typename H::Value;
  { handle.name() } -> std::convertible_to<std::string_view>;
} && (H::depth == 0 || H::depth == std::numeric_limits<std::size_t>::max());

} // namespace detail

class Spec;

/**
 * Values for the `param` statements of a reader spec, as name and value
 * strings in the order they were first set (Specification §9.1)
 *
 * A value is set by name as a string, or through a typed handle as a value
 * of the handle's type, such as a handle that `param("scale", 3)` returns
 * in `<iris/build.hpp>`.
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

  /**
   * Set the parameter of `handle` to `value`, formatted as the conversion
   * in Specification §9.2 expects
   *
   * Only a value of the handle's type compiles: an integer for an `int`
   * handle, an integer or a float for a `float` or `num` handle, a string
   * for a `string` handle, and any of them for a handle of unknown type. A
   * float is written in the shortest form that converts back to the same
   * number, with `.0` on a whole number so that it stays a float; a
   * negative zero keeps its sign. A float that is not finite, or nonzero
   * and below the smallest normal float, fails the conversion when the
   * spec reads. A name that is already set keeps its place.
   */
  template <class H, class T>
    requires detail::ParamHandle<H> &&
             detail::ParamAccepts<typename H::Value, T>
  void set(const H &handle, const T &value) {
    const std::string_view name = handle.name();
    if constexpr (detail::ParamInteger<T>) {
      std::array<char, std::numeric_limits<std::uint64_t>::digits10 + 2> text{};
      const char *end =
          std::to_chars(text.data(), text.data() + text.size(), value).ptr;
      set(name, std::string_view(text.data(), end));
    } else if constexpr (detail::ParamFloat<T>) {
      set_float(name, static_cast<double>(value));
    } else {
      set(name, std::string_view(value));
    }
  }

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
  /// Set `name` to the text of float `value` (see the typed set)
  void set_float(std::string_view name, double value);
  /// Return the values, which are shared with no other Params
  detail::ParamsImpl &own();
  /// Return the values, which may be shared
  [[nodiscard]] const detail::ParamsImpl &values() const;

  std::pmr::memory_resource *memory_;
  /// The values, or null without values
  std::shared_ptr<detail::ParamsImpl> impl_;
};

} // namespace iris
