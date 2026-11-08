// The types of the spec builder, checked at compile time: what Data::get
// returns through a handle, the handles that statements return, the types
// of expressions, and the free functions, which ADL cannot replace.

#include <iris/build.hpp>
#include <iris/iris.hpp>

#include <concepts>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <optional>
#include <span>
#include <string>
#include <string_view>
#include <type_traits>
#include <utility>

namespace {

using iris::Array;
using iris::Dynamic;
using iris::dynamic_depth;
using iris::DynField;
using iris::Field;
using iris::Number;
using iris::Numbers;
using iris::Presence;
using iris::ValueRef;
using Int = std::int64_t;
using Str = std::string;
constexpr Presence maybe = Presence::Maybe;

/// The type that Data::get returns for handle H
template <class H>
using Got = decltype(std::declval<const iris::Data &>().get(std::declval<H>()));

// Depth 0
static_assert(std::same_as<Got<Field<Int>>, Int>);
static_assert(std::same_as<Got<Field<double>>, double>);
static_assert(std::same_as<Got<Field<Str>>, std::string_view>);
static_assert(std::same_as<Got<Field<Number>>, Number>);
static_assert(std::same_as<Got<Field<Dynamic>>, ValueRef>);
// Depth 1
static_assert(std::same_as<Got<Field<Int, 1>>, std::span<const Int>>);
static_assert(std::same_as<Got<Field<double, 1>>, std::span<const double>>);
static_assert(std::same_as<Got<Field<Str, 1>>, Array<Str, 1>>);
static_assert(std::same_as<Got<Field<Number, 1>>, Numbers>);
static_assert(std::same_as<Got<Field<Dynamic, 1>>, ValueRef>);
// Depth 2, and the elements of an array
static_assert(std::same_as<Got<Field<Int, 2>>, Array<Int, 2>>);
static_assert(std::same_as<Got<Field<double, 2>>, Array<double, 2>>);
static_assert(std::same_as<Got<Field<Str, 2>>, Array<Str, 2>>);
static_assert(std::same_as<Got<Field<Number, 2>>, Array<Number, 2>>);
static_assert(std::same_as<Got<Field<Dynamic, 2>>, Array<Dynamic, 2>>);
static_assert(std::same_as<Array<Int, 2>::Element, std::span<const Int>>);
static_assert(std::same_as<Array<Str, 1>::Element, std::string_view>);
static_assert(std::same_as<Array<Number, 2>::Element, Numbers>);
static_assert(std::same_as<Array<Int, 3>::Element, Array<Int, 2>>);
static_assert(std::same_as<Array<Dynamic, 2>::Element, ValueRef>);
// A dynamic depth
static_assert(std::same_as<Got<Field<Int, dynamic_depth>>, ValueRef>);
static_assert(std::same_as<Got<Field<Str, dynamic_depth>>, ValueRef>);
// Presence::Maybe
static_assert(std::same_as<Got<Field<Int, 0, maybe>>, std::optional<Int>>);
static_assert(
    std::same_as<Got<Field<Str, 0, maybe>>, std::optional<std::string_view>>);
static_assert(std::same_as<Got<Field<double, 1, maybe>>,
                           std::optional<std::span<const double>>>);
static_assert(
    std::same_as<Got<Field<Int, 2, maybe>>, std::optional<Array<Int, 2>>>);
static_assert(std::same_as<Got<DynField>, std::optional<ValueRef>>);

// A handle is a literal type, so a declared one can be constexpr.
constexpr Field<Int, 1> constant_handle("x");
static_assert(constant_handle.name() == "x");
static_assert(std::is_trivially_copyable_v<Field<Str, 2, maybe>>);
static_assert(Field<Int, 1>::depth == 1);
static_assert(Field<Int, 1, maybe>::presence == maybe);
static_assert(std::same_as<Field<Number>::Value, Number>);

// The free functions are objects, so a call never looks for functions by
// ADL: with std::string arguments, `min` cannot find std::min.
namespace adl {
using namespace iris::build;
using MinOfStrings =
    decltype(min(std::declval<std::string>(), std::declval<std::string>()));
static_assert(std::same_as<MinOfStrings, Expr<Int>>);
static_assert(std::same_as<decltype(max(constant_handle)), Expr<Int>>);
static_assert(std::is_object_v<std::remove_cvref_t<decltype(min)>>);
static_assert(std::is_object_v<std::remove_cvref_t<decltype(count)>>);
static_assert(std::is_object_v<std::remove_cvref_t<decltype(size)>>);
static_assert(std::is_object_v<std::remove_cvref_t<decltype(pattern)>>);
static_assert(std::is_object_v<std::remove_cvref_t<decltype(ref)>>);
static_assert(std::is_object_v<std::remove_cvref_t<decltype(int_)>>);
} // namespace adl

// A host constant becomes an expression of its type through lit, and one
// with no type through dynamic(), as an array or record operand needs.
namespace hosts {
using namespace iris::build;
static_assert(std::same_as<decltype(lit(host::int_max)), Expr<Int>>);
static_assert(std::same_as<decltype(host::int_min.dynamic()), Expr<Dynamic>>);
static_assert(
    std::same_as<decltype(host::int_max.dynamic()[0]), Expr<Dynamic, Open>>);
static_assert(std::same_as<decltype(size(host::int_max.dynamic())), Expr<Int>>);
} // namespace hosts

/// Whether `a + b` is an expression
template <class A, class B>
concept Addable = requires(A a, B b) { a + b; };
/// Whether `a == b` is an expression
template <class A, class B>
concept Comparable = requires(A a, B b) { a == b; };

// Operators need an operand from a builder, so they never apply to plain
// values, and they check value types.
static_assert(Addable<Field<Int>, int>);
static_assert(Addable<int, Field<Int>>);
static_assert(!Addable<Field<Str>, int>);
static_assert(!Addable<Field<Int>, const char *>);
static_assert(!Addable<Field<double>, Field<Int>>);
static_assert(Addable<Field<Dynamic>, Field<Int>>);
static_assert(Comparable<Field<Str>, const char *>);
static_assert(!Comparable<Field<Str>, int>);
static_assert(Comparable<Field<double>, int>);

/// Whether `params.set(handle, value)` compiles
template <class H, class T>
concept Settable = requires(iris::Params params, H handle, T value) {
  params.set(handle, value);
};

// A typed parameter handle takes a value of its type, and only for an entry
// that a caller value can produce: a single value.
static_assert(Settable<Field<Int>, int>);
static_assert(Settable<Field<Int>, std::uint32_t>);
static_assert(!Settable<Field<Int>, double>);
static_assert(!Settable<Field<Int>, const char *>);
static_assert(!Settable<Field<Int>, bool>);
static_assert(!Settable<Field<Int>, char>);
static_assert(Settable<Field<double>, int>);
static_assert(Settable<Field<double>, float>);
static_assert(!Settable<Field<double>, std::string_view>);
static_assert(Settable<Field<Number>, double>);
static_assert(!Settable<Field<Number>, Str>);
static_assert(Settable<Field<Str>, const char *>);
static_assert(Settable<Field<Str, 0, maybe>, Str>);
static_assert(!Settable<Field<Str>, int>);
static_assert(Settable<DynField, int>);
static_assert(Settable<DynField, double>);
static_assert(Settable<DynField, std::string_view>);
static_assert(Settable<Field<Int, dynamic_depth>, int>);
static_assert(!Settable<Field<Int, 1>, int>);

} // namespace

// The statements may throw std::bad_alloc, which ends the test as it
// should.
int main() { // NOLINT(bugprone-exception-escape)
  using namespace iris::build;
  // The standard headers may declare ::pow, as MSVC's do, which the
  // using-directive would make ambiguous.
  using iris::build::pow;
  SpecBuilder b;

  // The handles that statements return
  auto n = b.read("n", int_);
  static_assert(std::same_as<decltype(n), Field<Int>>);
  auto f = b.read("f", float_);
  static_assert(std::same_as<decltype(f), Field<double>>);
  auto w = b.read("w", word);
  static_assert(std::same_as<decltype(w), Field<Str>>);
  auto u = b.read("u", num);
  static_assert(std::same_as<decltype(u), Field<Number>>);
  auto row = b.read("row", int_.dim(n));
  static_assert(std::same_as<decltype(row), Field<Int, 1>>);
  auto grid = b.read("grid", alphabet("ab").dim(2).dim(n));
  static_assert(std::same_as<decltype(grid), Field<Int, 2>>);
  auto range = b.read("range", Range{.from = 0, .below = n});
  static_assert(std::same_as<decltype(range), Field<Int>>);
  auto many = b.read("many", int_.dims(std::initializer_list<int>{1, 1}));
  static_assert(std::same_as<decltype(many), Field<Int, dynamic_depth>>);
  auto items = b.list("items", word.until(eol));
  static_assert(std::same_as<decltype(items), Field<Str, 1>>);
  static_assert(std::is_void_v<decltype(b.read(discard, int_))>);
  auto again = b.read(n, float_);
  static_assert(std::same_as<decltype(again), DynField>);

  // let keeps the depth of a scalar expression and forgets a lookup's
  auto sum_of = b.let("s", n + 1);
  static_assert(std::same_as<decltype(sum_of), Field<Int>>);
  auto first = b.let("first", row[0]);
  static_assert(std::same_as<decltype(first), Field<Int, dynamic_depth>>);
  auto loose = b.let("loose", n.dynamic());
  static_assert(std::same_as<decltype(loose), Field<Dynamic, dynamic_depth>>);
  auto p = b.param("p");
  static_assert(std::same_as<decltype(p), Field<Dynamic, dynamic_depth>>);
  auto q = b.param("q", 3);
  static_assert(std::same_as<decltype(q), Field<Int>>);

  // Captures, forward handles and undefined names
  auto token = b.capture("token", untyped);
  static_assert(std::same_as<decltype(token), Field<Str>>);
  auto bounded = b.capture("bounded", Range{.from = 1});
  static_assert(std::same_as<decltype(bounded), Field<Int>>);
  auto opt = b.optional_capture("opt", float_);
  static_assert(std::same_as<decltype(opt), Field<double, 0, maybe>>);
  auto later = b.forward<Field<Int, 1>>("later");
  static_assert(std::same_as<decltype(later), Field<Int, 1>>);
  static_assert(std::same_as<decltype(b.undefined("z")), DynField>);

  // Depth and presence follow the scope
  auto column = b.repeat(n, [](auto &r) {
    static_assert(std::remove_cvref_t<decltype(r)>::depth == 1);
    return r.read("column", int_);
  });
  static_assert(std::same_as<decltype(column), Field<Int, 1>>);
  auto nested = b.repeat(n, [&](auto &r) {
    return r.repeat(n, [](auto &s) { return s.read("cell", word); });
  });
  static_assert(std::same_as<decltype(nested), Field<Str, 2>>);
  auto branch = b.if_(n, [](auto &t) { return t.read("maybe", int_); });
  static_assert(
      std::same_as<decltype(branch.result()), Field<Int, 0, maybe> &>);
  auto otherwise = branch.else_([](auto &e) { return e.read("other", int_); });
  static_assert(std::same_as<decltype(otherwise), Field<Int, 0, maybe>>);
  auto in_arm = b.sections([](auto &arms) {
    return arms.arm("a", [](auto &s) { return s.read("arm", int_); });
  });
  static_assert(std::same_as<decltype(in_arm), Field<Int>>);
  auto in_optional_arm = b.sections([](auto &arms) {
    return arms.optional_arm("b", [](auto &s) { return s.read("opt", int_); });
  });
  static_assert(std::same_as<decltype(in_optional_arm), Field<Int, 0, maybe>>);
  auto named = b.define_type("t", float_);
  static_assert(std::same_as<decltype(named), NamedType<double>>);
  static_assert(std::same_as<decltype(b.param("scale", 3)), Field<Int>>);
  static_assert(std::same_as<decltype(b.param("ratio", 1.5)), Field<double>>);
  static_assert(
      std::same_as<decltype(b.param("free")), Field<Dynamic, dynamic_depth>>);
  static_assert(std::same_as<decltype(b.read("typed", named)), Field<double>>);

  // Expressions
  static_assert(std::same_as<decltype(n + 1), Expr<Int>>);
  static_assert(std::same_as<decltype(n < 1.5), Expr<Int>>);
  static_assert(std::same_as<decltype(-f), Expr<double>>);
  static_assert(std::same_as<decltype(!n), Expr<Int>>);
  static_assert(std::same_as<decltype(row[0]), Expr<Int, Open>>);
  static_assert(std::same_as<decltype(n.dynamic()), Expr<Dynamic>>);
  static_assert(std::same_as<decltype(lit(1.5)), Expr<double>>);
  static_assert(std::same_as<decltype(lit("text")), Expr<Str>>);
  static_assert(std::same_as<decltype(pow(n, 2)), Expr<Int>>);
  static_assert(
      std::same_as<decltype(sum(row, [](const auto &x) { return x; })),
                   Expr<Int>>);
  static_assert(std::same_as<decltype(size(row)), Expr<Int>>);
  static_assert(std::same_as<decltype(defined(n)), Expr<Int>>);
  auto record = b.repeat(1, [](auto &r) { return r.record(); });
  static_assert(std::same_as<decltype(record), Record>);
  static_assert(std::same_as<decltype(record.get(column)), Expr<Int, Open>>);
  static_assert(std::same_as<decltype(record[0]), RecordExpr>);
  static_assert(std::same_as<decltype(record.list()), RecordList>);

  (void)constant_handle;
  return EXIT_SUCCESS;
}
