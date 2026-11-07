#pragma once

#include <cstddef>
#include <cstdint>
#include <initializer_list>
#include <memory>
#include <memory_resource>
#include <optional>
#include <span>
#include <stdexcept>
#include <string>
#include <string_view>

namespace iris {

namespace detail {
class Value;
struct DataImpl;
struct TypedAccess;
} // namespace detail

class Spec;

/// The type of the leaves of a value (Specification §3.1)
enum class ValueType {
  Int,    ///< A 64-bit signed integer
  Float,  ///< A double-precision float
  String, ///< A UTF-8 string
};

/// Return the spelling of `type` in canonical data: `int`, `float`, or
/// `string`
std::string_view type_name(ValueType type);

/// Misuse of the data accessors: an unknown name, a value of another type or
/// depth, or an index out of range. It is not a language error.
class AccessError : public std::logic_error {
public:
  using std::logic_error::logic_error;
};

/**
 * A borrowed view of a value read, or of the part of it that a path selects
 *
 * A view of depth 0 is a single value, and a view of depth d > 0 is an array
 * whose elements are views of depth d - 1. The view stays valid while any
 * copy of the Data it comes from exists. Misuse throws AccessError.
 */
class ValueRef {
public:
  /// Return the type of the leaves
  [[nodiscard]] ValueType type() const;
  /// Return the number of array levels, 0 for a single value
  [[nodiscard]] std::size_t depth() const;
  /// Return the number of elements of this array
  [[nodiscard]] std::size_t size() const;
  /// Return element `i` of this array, counted from 0
  [[nodiscard]] ValueRef operator[](std::size_t i) const;
  /// Return the part that `path` selects, one index per level
  [[nodiscard]] ValueRef at(std::span<const std::size_t> path) const;
  /// Return the part that the braced `path` selects, as in `at({1, 0})`
  [[nodiscard]] ValueRef at(std::initializer_list<std::size_t> path) const {
    return at(std::span<const std::size_t>(path.begin(), path.size()));
  }

  /// Return this single integer
  [[nodiscard]] std::int64_t integer() const;
  /// Return this single number, converting an integer to a float
  [[nodiscard]] double number() const;
  /// Return this single string
  [[nodiscard]] std::string_view text() const;
  /// Return the integers of this one-dimensional array of integers
  [[nodiscard]] std::span<const std::int64_t> ints() const;
  /// Return the floats of this one-dimensional array of floats
  [[nodiscard]] std::span<const double> floats() const;

private:
  friend class Data;
  friend struct detail::TypedAccess;
  /// View list or leaf `pos` at level `level` of `value` (see
  /// detail::Value::locate)
  ValueRef(const detail::Value &value, std::size_t level, std::size_t pos)
      : value_(&value), level_(level), pos_(pos) {}
  /// Throw AccessError unless this is an array
  void require_array() const;
  /// Throw AccessError unless this is a single value of type `type`
  void require_single(ValueType type) const;
  /// Throw AccessError unless this is a one-dimensional array of `type`
  void require_row(ValueType type) const;

  const detail::Value *value_;
  std::size_t level_;
  std::size_t pos_;
};

/**
 * The entries a read produces, in definition order (Specification §3.1)
 *
 * Copies share the entries, which never change, so they are cheap. The
 * entries use the memory of the spec that read them (see Spec). The
 * accessors that take a name and a path return the part of the named entry
 * that the path selects; misuse throws AccessError.
 */
class Data {
public:
  /// Data without entries, which allocates nothing
  Data();

  /// Return the number of entries
  [[nodiscard]] std::size_t size() const;
  /// Return the name of entry `i`
  [[nodiscard]] std::string_view name(std::size_t i) const;
  /// Return the value of entry `i`
  [[nodiscard]] ValueRef value(std::size_t i) const;
  /// Return the value of the entry called `name`, or nothing if there is none
  [[nodiscard]] std::optional<ValueRef> find(std::string_view name) const;

  /// Return the entries as a MiniZinc data file (Specification appendix B):
  /// one assignment per entry, each ending with a newline, or nothing
  /// without entries. The text is allocated from the memory resource of the
  /// spec that read the data.
  [[nodiscard]] std::pmr::string dzn() const;

  /// Return the part of entry `name` that `path` selects
  [[nodiscard]] ValueRef at(std::string_view name,
                            std::span<const std::size_t> path = {}) const;
  /// Return the number of elements of the array that `path` selects
  [[nodiscard]] std::size_t size(std::string_view name,
                                 std::span<const std::size_t> path = {}) const;
  /// Return the single integer that `path` selects
  [[nodiscard]] std::int64_t
  integer(std::string_view name, std::span<const std::size_t> path = {}) const;
  /// Return the single number that `path` selects, converting an integer to a
  /// float
  [[nodiscard]] double number(std::string_view name,
                              std::span<const std::size_t> path = {}) const;
  /// Return the single string that `path` selects
  [[nodiscard]] std::string_view
  text(std::string_view name, std::span<const std::size_t> path = {}) const;
  /// Return the integers of the one-dimensional array that `path` selects
  [[nodiscard]] std::span<const std::int64_t>
  ints(std::string_view name, std::span<const std::size_t> path = {}) const;
  /// Return the floats of the one-dimensional array that `path` selects
  [[nodiscard]] std::span<const double>
  floats(std::string_view name, std::span<const std::size_t> path = {}) const;

  /// Return the entry that `handle` names, typed by the handle: for an
  /// iris::Field, the iris::View of its value type, depth and presence.
  /// Throws AccessError if a required entry is missing, or if the entry has
  /// another type or depth than the handle promises.
  template <class Handle>
  [[nodiscard]] auto get(const Handle &handle) const
      -> decltype(handle.view(*this)) {
    return handle.view(*this);
  }

  /// The same as at() with a braced path, as in `data.at("clause", {3})`
  [[nodiscard]] ValueRef at(std::string_view name,
                            std::initializer_list<std::size_t> path) const {
    return at(name, span(path));
  }
  /// The same as size() with a braced path
  [[nodiscard]] std::size_t
  size(std::string_view name, std::initializer_list<std::size_t> path) const {
    return size(name, span(path));
  }
  /// The same as integer() with a braced path
  [[nodiscard]] std::int64_t
  integer(std::string_view name,
          std::initializer_list<std::size_t> path) const {
    return integer(name, span(path));
  }
  /// The same as number() with a braced path
  [[nodiscard]] double number(std::string_view name,
                              std::initializer_list<std::size_t> path) const {
    return number(name, span(path));
  }
  /// The same as text() with a braced path
  [[nodiscard]] std::string_view
  text(std::string_view name, std::initializer_list<std::size_t> path) const {
    return text(name, span(path));
  }
  /// The same as ints() with a braced path, as in `data.ints("clause", {3})`
  [[nodiscard]] std::span<const std::int64_t>
  ints(std::string_view name, std::initializer_list<std::size_t> path) const {
    return ints(name, span(path));
  }
  /// The same as floats() with a braced path
  [[nodiscard]] std::span<const double>
  floats(std::string_view name, std::initializer_list<std::size_t> path) const {
    return floats(name, span(path));
  }

private:
  friend class Spec;
  explicit Data(std::shared_ptr<const detail::DataImpl> impl);
  static std::span<const std::size_t>
  span(std::initializer_list<std::size_t> path) {
    return {path.begin(), path.size()};
  }

  std::shared_ptr<const detail::DataImpl> impl_;
};

} // namespace iris
