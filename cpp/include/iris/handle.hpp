#pragma once

#include <iris/data.hpp>

#include <concepts>
#include <cstddef>
#include <cstdint>
#include <iterator>
#include <limits>
#include <optional>
#include <span>
#include <string>
#include <string_view>
#include <type_traits>
#include <utility>
#include <variant>

namespace iris {

namespace build {
class Indexing;
namespace detail {
struct Access;
} // namespace detail
} // namespace build

/// Whether an entry is always defined, or may be left undefined
enum class Presence {
  Required, ///< The entry is always defined
  Maybe,    ///< The entry may be undefined, as after a branch that did not run
};

/// The depth of a handle whose number of array levels is not known
inline constexpr std::size_t dynamic_depth =
    std::numeric_limits<std::size_t>::max();

/// The value type of a handle whose leaf type is not known
struct Dynamic {};

/// A number of a `num` entry: an integer or a float
using Number = std::variant<std::int64_t, double>;

/// The leaves of a one-dimensional `num` array: integers or floats
using Numbers =
    std::variant<std::span<const std::int64_t>, std::span<const double>>;

/**
 * The value types of handles: `std::int64_t`, `double`, `std::string`,
 * Number for `num`, and Dynamic for a type that is not known
 */
template <class V>
concept HandleValue = std::same_as<V, std::int64_t> ||
                      std::same_as<V, double> || std::same_as<V, std::string> ||
                      std::same_as<V, Number> || std::same_as<V, Dynamic>;

template <HandleValue V, std::size_t D> class Array;

namespace detail {

/// Access to the leaves of a value that the public views lack
struct TypedAccess {
  /// Return the number of leaves of the whole value that `v` is part of
  static std::size_t leaves(const ValueRef &v);
  /// Whether the leaves of the whole value of `v` have type `type`, or
  /// there are none
  static bool leaves_match(const ValueRef &v, ValueType type) {
    return v.type() == type || leaves(v) == 0;
  }
};

struct MakeArray;

[[noreturn]] void throw_access(const char *message);

/// The view of a required handle with value type V and depth D
template <class V, std::size_t D> struct ViewOf {
  using Type = Array<V, D>;
};
template <class V> struct ViewOf<V, dynamic_depth> {
  using Type = ValueRef;
};
template <> struct ViewOf<std::int64_t, 0> {
  using Type = std::int64_t;
};
template <> struct ViewOf<double, 0> {
  using Type = double;
};
template <> struct ViewOf<std::string, 0> {
  using Type = std::string_view;
};
template <> struct ViewOf<Number, 0> {
  using Type = Number;
};
template <> struct ViewOf<Dynamic, 0> {
  using Type = ValueRef;
};
template <> struct ViewOf<std::int64_t, 1> {
  using Type = std::span<const std::int64_t>;
};
template <> struct ViewOf<double, 1> {
  using Type = std::span<const double>;
};
template <> struct ViewOf<std::string, 1> {
  using Type = Array<std::string, 1>;
};
template <> struct ViewOf<Number, 1> {
  using Type = Numbers;
};
template <> struct ViewOf<Dynamic, 1> {
  using Type = ValueRef;
};

/// The leaf type that value type V requires, if any
template <class V> constexpr std::optional<ValueType> leaf_type() {
  if constexpr (std::same_as<V, std::int64_t>)
    return ValueType::Int;
  else if constexpr (std::same_as<V, double>)
    return ValueType::Float;
  else if constexpr (std::same_as<V, std::string>)
    return ValueType::String;
  else
    return std::nullopt;
}

/// Throw AccessError unless `v` has `depth` levels
inline void require_depth(const ValueRef &v, std::size_t depth) {
  if (v.depth() != depth)
    throw_access(depth == 0 ? "the value is an array, not a single value"
                            : "the value does not have the handle's depth");
}

/// Throw AccessError unless the leaves of `v` suit value type V
template <class V> void require_leaves(const ValueRef &v) {
  if constexpr (std::same_as<V, Number>) {
    if (v.type() == ValueType::String && TypedAccess::leaves(v) > 0)
      throw_access("the value is of type string, not a number");
  } else if constexpr (constexpr auto type = leaf_type<V>(); type) {
    if (!TypedAccess::leaves_match(v, *type))
      throw_access("the leaves do not have the handle's type");
  }
}

template <class V, std::size_t D> ViewOf<V, D>::Type view(const ValueRef &v);

} // namespace detail

/// The view that Data::get returns for a handle with value type V, depth D
/// and presence P
template <class V, std::size_t D, Presence P>
using View =
    std::conditional_t<P == Presence::Maybe,
                       std::optional<typename detail::ViewOf<V, D>::Type>,
                       typename detail::ViewOf<V, D>::Type>;

/**
 * A view of an array of depth D, whose elements are views of depth D - 1
 *
 * Data::get returns it for an array of strings of depth 1 and for every
 * array of depth 2 or more. Like ValueRef, it stays valid while any copy of
 * the Data it comes from exists.
 */
template <HandleValue V, std::size_t D> class Array {
public:
  /// The view of an element
  using Element = detail::ViewOf<V, D - 1>::Type;

  /// An iterator over the elements
  class Iterator {
  public:
    // NOLINTBEGIN(readability-identifier-naming)
    /// The iterator's category: an input iterator
    using iterator_category = std::input_iterator_tag;
    /// The type of the elements
    using value_type = Element;
    /// The type of the distance between two iterators
    using difference_type = std::ptrdiff_t;
    // NOLINTEND(readability-identifier-naming)

    Iterator() = default;
    /// Return the current element
    Element operator*() const { return (*array_)[index_]; }
    /// Move to the next element
    Iterator &operator++() {
      ++index_;
      return *this;
    }
    /// Move to the next element
    Iterator operator++(int) {
      Iterator before = *this;
      ++index_;
      return before;
    }
    /// Whether both iterators are at the same element
    friend bool operator==(const Iterator &a, const Iterator &b) {
      return a.index_ == b.index_;
    }

  private:
    friend class Array;
    Iterator(const Array &array, std::size_t index)
        : array_(&array), index_(index) {}
    const Array *array_ = nullptr;
    std::size_t index_ = 0;
  };

  /// Return the number of elements
  [[nodiscard]] std::size_t size() const { return value_.size(); }
  /// Whether there are no elements
  [[nodiscard]] bool empty() const { return size() == 0; }
  /// Return element `i`, counted from 0. Throws AccessError if `i` is out
  /// of range.
  [[nodiscard]] Element operator[](std::size_t i) const {
    return detail::view<V, D - 1>(value_[i]);
  }
  /// Return an iterator at the first element
  [[nodiscard]] Iterator begin() const { return {*this, 0}; }
  /// Return an iterator after the last element
  [[nodiscard]] Iterator end() const { return {*this, size()}; }
  /// Return the untyped view of the array
  [[nodiscard]] ValueRef value() const { return value_; }

private:
  friend struct detail::MakeArray;
  explicit Array(const ValueRef &value) : value_(value) {}

  ValueRef value_;
};

namespace detail {

/// Makes the arrays that view() returns
struct MakeArray {
  template <class V, std::size_t D> static Array<V, D> make(const ValueRef &v) {
    return Array<V, D>(v);
  }
};

/// Return the view of `v` for value type V and depth D, or throw
/// AccessError if `v` does not match
template <class V, std::size_t D> ViewOf<V, D>::Type view(const ValueRef &v) {
  if constexpr (D == dynamic_depth) {
    // The depth is unknown, but a known leaf type still holds.
    require_leaves<V>(v);
    return v;
  } else if constexpr (std::same_as<V, Dynamic>) {
    // The leaf type is unknown, but the depth still holds.
    require_depth(v, D);
    if constexpr (D >= 2)
      return MakeArray::make<V, D>(v);
    else
      return v;
  } else if constexpr (D == 0) {
    if constexpr (std::same_as<V, std::int64_t>)
      return v.integer();
    else if constexpr (std::same_as<V, std::string>)
      return v.text();
    else {
      require_depth(v, 0);
      if (v.type() == ValueType::Float)
        return v.number();
      if constexpr (std::same_as<V, double>)
        throw_access("the value is not of type float");
      else {
        if (v.type() == ValueType::String)
          throw_access("the value is of type string, not a number");
        return Number(v.integer());
      }
    }
  } else if constexpr (D == 1) {
    require_depth(v, 1);
    require_leaves<V>(v);
    if constexpr (std::same_as<V, std::int64_t>)
      return v.size() == 0 ? std::span<const std::int64_t>() : v.ints();
    else if constexpr (std::same_as<V, double>)
      return v.size() == 0 ? std::span<const double>() : v.floats();
    else if constexpr (std::same_as<V, std::string>)
      return MakeArray::make<V, 1>(v);
    else {
      if (v.size() == 0 || v.type() == ValueType::Int)
        return Numbers(v.size() == 0 ? std::span<const std::int64_t>()
                                     : v.ints());
      return Numbers(v.floats());
    }
  } else {
    require_depth(v, D);
    require_leaves<V>(v);
    return MakeArray::make<V, D>(v);
  }
}

} // namespace detail

/**
 * A typed handle to an entry: its name, the type of its leaves, its number
 * of array levels, and whether it may be undefined
 *
 * Data::get returns the entry through the handle as the View for V, D and
 * P: an integer, a span of integers, an Array of depth 2, and so on, wrapped
 * in std::optional when P is Presence::Maybe. A handle is a literal type, so
 * it can be declared `constexpr` for an entry that a spec is known to
 * define. The builder in `<iris/build.hpp>` returns handles from its
 * statements and takes them in expressions.
 *
 * @tparam V the value type (see HandleValue)
 * @tparam D the number of array levels, or dynamic_depth
 * @tparam P whether the entry may be undefined
 */
template <class V, std::size_t D = 0, Presence P = Presence::Required>
class Field {
  static_assert(HandleValue<V>, "a handle's value type is std::int64_t, "
                                "double, std::string, Number or Dynamic");

public:
  /// The value type
  using Value = V;
  /// The number of array levels, or dynamic_depth
  static constexpr std::size_t depth = D;
  /// Whether the entry may be undefined
  static constexpr Presence presence = P;

  /// A handle to the entry called `name`, which must outlive the handle
  explicit constexpr Field(std::string_view name) : name_(name) {}

  /// Return the name of the entry
  [[nodiscard]] constexpr std::string_view name() const { return name_; }

  /// Return the entry of `data` through this handle. Throws AccessError if
  /// a required entry is missing, or if the entry has another type or
  /// depth. Data::get calls it.
  [[nodiscard]] View<V, D, P> view(const Data &data) const {
    const std::optional<ValueRef> found = data.find(name_);
    if (!found) {
      if constexpr (P == Presence::Maybe)
        return std::nullopt;
      else
        throw AccessError("no value named '" + std::string(name_) + "'");
    }
    return detail::view<V, D>(*found);
  }

  /// In a builder expression, element `index` of this entry
  /// (`<iris/build.hpp>`)
  template <class I, class B = build::Indexing>
  [[nodiscard]] auto operator[](I &&index) const {
    return B::index(*this, std::forward<I>(index));
  }

  /// In a builder expression, this entry with its type and depth forgotten
  /// (`<iris/build.hpp>`)
  template <class B = build::Indexing> [[nodiscard]] auto dynamic() const {
    return B::dynamic(*this);
  }

private:
  friend struct build::detail::Access;
  constexpr Field(std::string_view name, std::uint64_t owner)
      : name_(name), owner_(owner) {}

  std::string_view name_;
  std::uint64_t owner_ = 0;
};

/// A handle whose value type and depth are not known, and whose entry may
/// be undefined: Data::get returns a std::optional<ValueRef>
using DynField = Field<Dynamic, dynamic_depth, Presence::Maybe>;

} // namespace iris
