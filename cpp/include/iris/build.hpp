#pragma once

/**
 * @file
 * The spec builder: write a reader spec as C++ code, with typed handles.
 *
 * A SpecBuilder collects statements in text order and prints them as Iris
 * 0.1 text, which SpecBuilder::build parses as Spec::parse would. Every
 * statement takes a trailing `std::source_location`, so that an error names
 * the line of host code that wrote the statement. Blocks take a closure
 * that receives the scope of the block; statements must go to the innermost
 * open block. Expressions use the C++ operators whose meaning matches Iris,
 * and the free functions here are function objects, so that argument
 * dependent lookup never prefers std::min or std::ref to them.
 */

#include <iris/data.hpp>
#include <iris/error.hpp>
#include <iris/handle.hpp>
#include <iris/spec.hpp>

#include <concepts>
#include <cstddef>
#include <cstdint>
#include <expected>
#include <functional>
#include <initializer_list>
#include <memory>
#include <optional>
#include <ranges>
#include <source_location>
#include <stdexcept>
#include <string>
#include <string_view>
#include <type_traits>
#include <utility>
#include <vector>

/// The spec builder (`<iris/build.hpp>`)
namespace iris::build {

using iris::Dynamic;
using iris::dynamic_depth;
using iris::DynField;
using iris::Field;
using iris::Number;
using iris::Presence;

template <class V, class S> class Expr;
class Record;
class RecordExpr;
class RecordList;
template <std::size_t D, Presence P> class Scope;
template <std::size_t D, Presence P> class Arms;
template <std::size_t D, Presence P, class R> class If;
class SpecBuilder;

/// The shape of an expression whose depth follows from its scope
struct Scalar {};
/// The shape of an expression that looks a value up, such as `x[i]`, whose
/// depth is not known
struct Open {};

/// What kind of mistake a BuildError reports
enum class BuildErrorKind {
  /// A statement went to a scope that is not the innermost open block, or
  /// a branch continued after later statements
  Scope,
  /// A handle, record or type came from another builder
  Provenance,
  /// A name was defined or declared twice
  Redefinition,
  /// The statements have no spelling in Iris 0.1
  Unprintable,
  /// A capture, forward handle or record was bound wrongly or never
  Binding,
  /// The emitted text is not a valid reader spec: language_error() says why
  Spec,
};

/**
 * A mistake that SpecBuilder::build or SpecBuilder::source reports
 *
 * A builder check names the host call that made the mistake. A language
 * error from parsing the emitted text keeps the text and the error, and
 * names the call that wrote the statement on the error's line.
 */
class BuildError : public std::runtime_error {
public:
  /// Return what kind of mistake this is
  [[nodiscard]] BuildErrorKind kind() const { return kind_; }
  /// Return the message, without the code, line and call site of what()
  [[nodiscard]] const std::string &message() const { return message_; }
  /// Return the language error, for BuildErrorKind::Spec
  [[nodiscard]] const std::optional<Error> &language_error() const {
    return language_error_;
  }
  /// Return the emitted text, for BuildErrorKind::Spec
  [[nodiscard]] const std::optional<std::string> &text() const { return text_; }
  /// Return the line of the emitted text that the language error is on
  [[nodiscard]] std::optional<std::string_view> emitted_line() const;
  /// Return the host call that made the mistake, if it is known
  [[nodiscard]] const std::optional<std::source_location> &call_site() const {
    return call_site_;
  }

private:
  friend struct detail::Access;
  BuildError(BuildErrorKind kind, std::string message,
             std::optional<Error> language_error,
             std::optional<std::string> text,
             std::optional<std::source_location> call_site);

  BuildErrorKind kind_;
  std::string message_;
  std::optional<Error> language_error_;
  std::optional<std::string> text_;
  std::optional<std::source_location> call_site_;
};

/// The internals of the builder. Not part of the public interface.
namespace detail {

using BuilderId = std::uint64_t;
using Site = std::source_location;

enum class ValueTag : std::uint8_t { Int, Float, Str, Num, Dyn };

template <class V> constexpr ValueTag value_tag() {
  if constexpr (std::same_as<V, std::int64_t>)
    return ValueTag::Int;
  else if constexpr (std::same_as<V, double>)
    return ValueTag::Float;
  else if constexpr (std::same_as<V, std::string>)
    return ValueTag::Str;
  else if constexpr (std::same_as<V, Number>)
    return ValueTag::Num;
  else
    return ValueTag::Dyn;
}

/// Return a + b levels, dynamic if either is
constexpr std::size_t plus(std::size_t a, std::size_t b) {
  return (a == dynamic_depth || b == dynamic_depth) ? dynamic_depth : a + b;
}

/// Return D as known levels, or nothing for dynamic_depth
constexpr std::optional<std::size_t> levels_of(std::size_t d) {
  if (d == dynamic_depth)
    return std::nullopt;
  return d;
}

/// Whether a value of depth `a` may be set to a handle of depth `b`
constexpr bool same_depth(std::size_t a, std::size_t b) {
  return a == b || a == dynamic_depth || b == dynamic_depth;
}

enum class OpKind : std::uint8_t {
  Int,
  Float,
  Str,
  Host,
  Name,
  Binding,
  Record,
  RecordList,
  Unary,
  Binary,
  Index,
  Member,
  ListMember,
  Call,
  Comprehension,
};

enum class UnaryOp : std::uint8_t { Minus, Not };

enum class BinaryOp : std::uint8_t {
  Or,
  And,
  Equal,
  NotEqual,
  Less,
  LessEqual,
  Greater,
  GreaterEqual,
  Add,
  Subtract,
  Multiply,
  Divide,
  Remainder,
  Power,
};

/**
 * One operation of an expression in postfix order: a leaf, or an operator
 * that takes the values of the operations before it. Only the fields of
 * its kind are set.
 */
struct Op {
  OpKind kind = OpKind::Int;
  /// The operator of Unary and Binary
  std::uint8_t code = 0;
  /// Whether a Comprehension has a chosen name in `label`
  bool labelled = false;
  /// The value of Int, the argument count of Call
  std::int64_t integer = 0;
  /// The value of Float
  double number = 0;
  /// The binding of Binding and Comprehension; the record of Record,
  /// RecordList and ListMember
  std::uint64_t id = 0;
  /// The builder of Name, Member, Record, RecordList and ListMember
  BuilderId owner = 0;
  /// The name of Host, Name, Member, the function of Call and Comprehension
  std::string_view name;
  /// The chosen name of a Comprehension
  std::string_view label;
  /// The value of Str
  std::shared_ptr<const std::string> text;
};

using Ops = std::vector<Op>;

void append(Ops &to, Ops &&from);

struct AlphabetOption {
  enum class Kind : std::uint8_t { Blank, Strip, Base } kind = Kind::Blank;
  std::string text;
  std::int64_t base = 0;
};

struct TypeSpec {
  enum class Kind : std::uint8_t { Base, Alphabet, Named, Range };
  Kind kind = Kind::Base;
  /// The keyword of Base
  std::string_view base;
  /// The symbols and options of Alphabet
  std::string symbols;
  std::vector<AlphabetOption> options;
  /// The name, builder and declaring statement of Named
  std::string_view named;
  BuilderId owner = 0;
  std::size_t declaration = 0;
  /// The bounds of Range
  std::optional<Ops> lower;
  std::optional<Ops> upper;
  bool inclusive = true;
  /// Whether a Range was given both `to` and `below`
  bool both_upper = false;
};

/// A type with its dimensions and options
struct Declared {
  std::vector<Ops> dimensions;
  TypeSpec type;
  std::optional<Ops> layout;
  std::optional<Ops> offset;
};

struct ListEnd {
  bool eol = true;
  Ops sentinel;
};

struct Piece {
  enum class Kind : std::uint8_t {
    Text,
    Capture,
    Bind,
    Reference,
    Validate,
    Blank,
    Discard,
    DiscardAs,
    RestOfLine,
  };
  Kind kind = Kind::Text;
  std::string text;
  std::string_view name;
  BuilderId owner = 0;
  ValueTag value = ValueTag::Str;
  /// The type of Bind, Validate and DiscardAs; none for `untyped`
  std::optional<Declared> type;
};

/// What a definition defines
struct Target {
  enum class Kind : std::uint8_t { None, Discard, New, Existing };
  Kind kind = Kind::None;
  std::string name;
  BuilderId owner = 0;
};

/// The name a statement defined, and the builder it belongs to
struct HandleName {
  std::string_view name;
  BuilderId owner = 0;
};

struct TypeName {
  std::string_view name;
  BuilderId owner = 0;
  std::size_t declaration = 0;
};

struct Clause {
  std::vector<std::string> until;
  Target store;
  bool skip_other = false;
};

struct State;

enum class Simple : std::uint8_t {
  Format,
  Comment,
  Stop,
  Tokens,
  Separators,
  IgnoreTrailing,
  Expect,
  SkipLine,
  Instances,
  Else,
};

enum class Computed : std::uint8_t { SkipLines, Index, If, ElseIf };

enum class RepeatKind : std::uint8_t { Times, UntilEof, Until };

std::string_view intern(std::string_view name);
std::uint64_t next_binding();

std::shared_ptr<State> new_state();
BuilderId builder_id(const State &state);
std::size_t statement_count(const State &state);
std::size_t open_block(State &state);
void close_block(State &state, Site site);
void check_last(State &state, std::size_t statement, Site site);

void simple(State &state, std::size_t block, Simple kind,
            std::vector<std::string> values, Site site);
std::size_t computed(State &state, std::size_t block, Computed kind, Ops &&ops,
                     Site site);
void check(State &state, std::size_t block, Ops &&condition,
           std::optional<std::string> message, Site site);
void pattern_statement(State &state, std::size_t block, bool find,
                       std::vector<Piece> &&pieces, Site site);
HandleName declaration(State &state, std::size_t block, const Target &target,
                       ValueTag value, Declared &&declared, Site site);
HandleName else_declaration(State &state, std::size_t block, std::size_t after,
                            const Target &target, ValueTag value,
                            Declared &&declared, Site site);
HandleName list(State &state, std::size_t block, const Target &target,
                ValueTag value, Declared &&declared, ListEnd &&end, Site site);
HandleName let(State &state, std::size_t block, const Target &target,
               ValueTag value, std::optional<std::size_t> levels, Ops &&ops,
               Site site);
HandleName param(State &state, std::size_t block, const Target &target,
                 ValueTag value, std::optional<std::size_t> levels,
                 std::optional<Ops> &&fallback, Site site);
TypeName define_type(State &state, std::size_t block, std::string_view name,
                     Declared &&declared, Site site);
std::size_t repeat(State &state, std::size_t block, RepeatKind kind,
                   Ops &&times, std::vector<std::string> terminators,
                   const Target &store, Site site);
void sections(State &state, std::size_t block, std::vector<Clause> clauses,
              Site site);
void arm(State &state, std::size_t block, std::vector<Piece> &&pieces,
         bool optional, bool last, bool body, Site site);
HandleName capture(State &state, std::string_view name, ValueTag value,
                   std::optional<Declared> &&type,
                   std::optional<std::size_t> levels, bool required, Site site);
HandleName forward(State &state, std::string_view name, ValueTag value,
                   std::optional<std::size_t> levels, bool required, Site site);
HandleName undefined(State &state, std::string_view name, Site site);
std::size_t forward_record(State &state, std::string_view each,
                           std::string_view list, Site site);
void name_record(State &state, std::size_t record, bool each,
                 std::string_view name, Site site);
void record_as(State &state, std::size_t record, std::size_t forward,
               BuilderId owner, Site site);
std::expected<std::string, BuildError> try_source(const State &state);
std::expected<Spec, BuildError> try_build(const State &state);

/// The plain type of an argument
template <class T> using Bare = std::decay_t<T>;

template <class T> struct IsField : std::false_type {};
template <class V, std::size_t D, Presence P>
struct IsField<Field<V, D, P>> : std::true_type {};

template <class T> struct IsExpr : std::false_type {};
template <class V, class S> struct IsExpr<Expr<V, S>> : std::true_type {};

/// The integer types that are literals: no bool or character type, and no
/// unsigned type that may not fit in std::int64_t
template <class T>
concept LiteralInteger =
    std::integral<T> && !std::same_as<T, bool> && !std::same_as<T, char> &&
    !std::same_as<T, signed char> && !std::same_as<T, unsigned char> &&
    !std::same_as<T, wchar_t> && !std::same_as<T, char8_t> &&
    !std::same_as<T, char16_t> && !std::same_as<T, char32_t> &&
    (std::is_signed_v<T> || sizeof(T) < sizeof(std::int64_t));

template <class T>
concept LiteralString =
    std::same_as<T, const char *> || std::same_as<T, char *> ||
    std::same_as<T, std::string> || std::same_as<T, std::string_view>;

/// What an operand contributes to an expression; specialized for every
/// operand type
template <class T> struct OperandTraits {};

} // namespace detail

/// An operand of an expression: a handle, an expression, an integer, float
/// or string literal, or a host constant
template <class T>
concept Operand =
    requires { typename detail::OperandTraits<detail::Bare<T>>::Value; };

/// The value type of operand T
template <Operand T>
using ValueOf = detail::OperandTraits<detail::Bare<T>>::Value;

/// The value types that `+`, `-`, `*`, `/`, `%`, `pow`, `!`, `&&` and `||`
/// take
template <class V>
concept Integral = std::same_as<V, std::int64_t> || std::same_as<V, Dynamic>;

/// The value types that unary `-`, `<`, `<=`, `>` and `>=` take
template <class V>
concept Numeric = std::same_as<V, std::int64_t> || std::same_as<V, double> ||
                  std::same_as<V, Number> || std::same_as<V, Dynamic>;

/// The value types of range bounds
template <class V>
concept BoundValue = std::same_as<V, std::int64_t> || std::same_as<V, Number> ||
                     std::same_as<V, Dynamic>;

/// The pairs of value types that `==` and `!=` compare
template <class L, class R>
concept Equatable =
    (Numeric<L> && Numeric<R>) ||
    (std::same_as<L, std::string> && std::same_as<R, std::string>) ||
    std::same_as<L, Dynamic> || std::same_as<R, Dynamic>;

/**
 * An expression with value type V and shape S
 *
 * Operators and the functions of this namespace make expressions from
 * operands. An expression of shape Open looks a value up and can be indexed
 * with `[]` and projected with get(); so can one of value type Dynamic.
 *
 * @tparam V the value type (see iris::HandleValue)
 * @tparam S Scalar or Open
 */
template <class V, class S = Scalar> class Expr {
public:
  /// Element `index` of this array
  template <Operand I>
    requires(std::same_as<S, Open> || std::same_as<V, Dynamic>) &&
            Integral<ValueOf<I>>
  Expr<V, Open> operator[](I &&index) const &;
  /// Element `index` of this array
  template <Operand I>
    requires(std::same_as<S, Open> || std::same_as<V, Dynamic>) &&
            Integral<ValueOf<I>>
  Expr<V, Open> operator[](I &&index) &&;
  /// The field `field` of this record
  template <class F, std::size_t H, Presence P>
    requires std::same_as<S, Open> || std::same_as<V, Dynamic>
  [[nodiscard]] Expr<F, Open> get(const Field<F, H, P> &field) const &;
  /// The field `field` of this record
  template <class F, std::size_t H, Presence P>
    requires std::same_as<S, Open> || std::same_as<V, Dynamic>
  Expr<F, Open> get(const Field<F, H, P> &field) &&;
  /// This expression with its type and depth forgotten
  [[nodiscard]] Expr<Dynamic, Scalar> dynamic() const & {
    return Expr<Dynamic, Scalar>(ops_);
  }
  /// This expression with its type and depth forgotten
  [[nodiscard]] Expr<Dynamic, Scalar> dynamic() && {
    return Expr<Dynamic, Scalar>(std::move(ops_));
  }

private:
  friend struct detail::Access;
  template <class W, class T> friend class Expr;
  explicit Expr(detail::Ops ops) : ops_(std::move(ops)) {}

  detail::Ops ops_;
};

namespace detail {
struct HostName {};
} // namespace detail

/// A host constant (Specification §8.11), which Iris spells with the
/// host's prefix
class Host {
public:
  /// The constant `name`, without the prefix; see the constants in
  /// namespace host
  constexpr Host(detail::HostName /*tag*/, std::string_view name)
      : name_(name) {}
  /// Return the name of the constant, without the prefix
  [[nodiscard]] constexpr std::string_view name() const { return name_; }
  /// This constant with its type forgotten, as an expression; `lit(c)`
  /// keeps the type
  [[nodiscard]] Expr<Dynamic, Scalar> dynamic() const;

private:
  std::string_view name_;
};

/// The host constants
namespace host {
/// `int_max`, the largest integer of a host variable
inline constexpr Host int_max{detail::HostName{}, "int_max"};
/// `int_min`, the smallest integer of a host variable
inline constexpr Host int_min{detail::HostName{}, "int_min"};
} // namespace host

/**
 * The records of a repeat: `job.get(machine)` projects a field of the
 * current record, list() is the list of all of them, and comprehensions
 * iterate over them
 */
class Record {
public:
  /// The field `field` of the current record
  template <class V, std::size_t D, Presence P>
  [[nodiscard]] Expr<V, Open> get(const Field<V, D, P> &field) const;
  /// The records of the repeat `inner` in the current record
  [[nodiscard]] RecordList records(const Record &inner) const;
  /// The list of all records
  [[nodiscard]] RecordList list() const;
  /// Record `index` of the list
  template <Operand I>
    requires Integral<ValueOf<I>>
  RecordExpr operator[](I &&index) const;

private:
  friend struct detail::Access;
  Record(std::size_t id, detail::BuilderId owner) : id_(id), owner_(owner) {}

  std::size_t id_;
  detail::BuilderId owner_;
};

/// A record selected by an expression, such as `jobs[0]`
class RecordExpr {
public:
  /// The field `field` of this record
  template <class V, std::size_t D, Presence P>
  [[nodiscard]] Expr<V, Open> get(const Field<V, D, P> &field) const;
  /// The records of the repeat `inner` in this record
  [[nodiscard]] RecordList records(const Record &inner) const;
  /// Element `index` of this record, which is a list
  template <Operand I>
    requires Integral<ValueOf<I>>
  RecordExpr operator[](I &&index) const;

private:
  friend struct detail::Access;
  explicit RecordExpr(detail::Ops ops) : ops_(std::move(ops)) {}

  detail::Ops ops_;
};

/// A list of records, such as `jobs` or `job.tasks`
class RecordList {
public:
  /// Record `index` of the list
  template <Operand I>
    requires Integral<ValueOf<I>>
  RecordExpr operator[](I &&index) const;

private:
  friend struct detail::Access;
  explicit RecordList(detail::Ops ops) : ops_(std::move(ops)) {}

  detail::Ops ops_;
};

/// A domain with the name that a comprehension gives its element; see over
template <class T> class Over {
public:
  /// The domain, whose element the comprehension calls `name`
  [[nodiscard]] Over named(std::string_view name) const {
    return Over(domain_, detail::intern(name));
  }

private:
  friend struct detail::Access;
  Over(T domain, std::optional<std::string_view> name)
      : domain_(std::move(domain)), name_(name) {}

  T domain_;
  std::optional<std::string_view> name_;
};

template <class V> class ListReading;
class Alphabet;
template <class V> class NamedType;

namespace detail {

/// What a type contributes to a statement; specialized for every type
template <class T> struct TypeTraits {};

} // namespace detail

/// A type: a constant such as int_, a Range, an alphabet, a NamedType, or
/// a Type made from one of these
template <class T>
concept TypeLike =
    requires { typename detail::TypeTraits<std::remove_cvref_t<T>>::Value; };

/// The value type that type T reads
template <TypeLike T>
using TypeValue = detail::TypeTraits<std::remove_cvref_t<T>>::Value;

/// The number of dimensions of type T, or dynamic_depth
template <TypeLike T>
inline constexpr std::size_t type_dims =
    detail::TypeTraits<std::remove_cvref_t<T>>::dims;

/// The end of a list: end of line, as in `list x:int until eol`
struct Eol {};
/// The end of line, which ends a list
inline constexpr Eol eol{};

/**
 * A type with value type V and N dimensions, as in `int_.dim(n).dim(m)`
 *
 * @tparam V the value type
 * @tparam N the number of dimensions, or dynamic_depth
 */
template <class V, std::size_t N> class TypeOf {
public:
  /// This type with one more dimension of `size` elements
  template <Operand E>
  [[nodiscard]] TypeOf<V, detail::plus(N, 1)> dim(E &&extent) const;
  /// This type with a dimension for each size in `sizes`
  template <std::ranges::input_range R>
    requires Operand<std::ranges::range_reference_t<R>>
  [[nodiscard]] TypeOf<V, dynamic_depth> dims(R &&sizes) const;
  /// This type read with the layout `layout`
  template <Operand E> [[nodiscard]] TypeOf<V, N> layout(E &&value) const;
  /// This type with `offset` added to every value read
  template <Operand E> [[nodiscard]] TypeOf<V, N> offset(E &&value) const;
  /// A list of this type that `end` ends: eol, or a sentinel value
  template <class E>
    requires(N == 0) &&
            (std::same_as<std::remove_cvref_t<E>, Eol> || Operand<E>)
  [[nodiscard]] ListReading<V> until(E &&end) const;

private:
  friend struct detail::Access;
  template <class W, std::size_t M> friend class TypeOf;
  explicit TypeOf(detail::Declared declared) : declared_(std::move(declared)) {}

  detail::Declared declared_;
};

/**
 * A base type: one of the constants int_, float_, num, word and rest
 *
 * Its methods give a TypeOf with dimensions or options, or a ListReading.
 */
template <class V> class BaseType {
public:
  /// The base type with the keyword `keyword`; see the constants
  constexpr BaseType(detail::HostName /*tag*/, std::string_view keyword)
      : keyword_(keyword) {}
  /// Return the keyword
  [[nodiscard]] constexpr std::string_view keyword() const { return keyword_; }
  /// This type with a dimension of `size` elements
  template <Operand E> [[nodiscard]] TypeOf<V, 1> dim(E &&extent) const {
    return ty().dim(std::forward<E>(extent));
  }
  /// This type with a dimension for each size in `sizes`
  template <std::ranges::input_range R>
    requires Operand<std::ranges::range_reference_t<R>>
  [[nodiscard]] TypeOf<V, dynamic_depth> dims(R &&sizes) const {
    return ty().dims(std::forward<R>(sizes));
  }
  /// This type read with the layout `layout`
  template <Operand E> [[nodiscard]] TypeOf<V, 0> layout(E &&value) const {
    return ty().layout(std::forward<E>(value));
  }
  /// This type with `offset` added to every value read
  template <Operand E> [[nodiscard]] TypeOf<V, 0> offset(E &&value) const {
    return ty().offset(std::forward<E>(value));
  }
  /// A list of this type that `end` ends: eol, or a sentinel value
  template <class E>
    requires std::same_as<std::remove_cvref_t<E>, Eol> || Operand<E>
  [[nodiscard]] ListReading<V> until(E &&end) const {
    return ty().until(std::forward<E>(end));
  }

private:
  [[nodiscard]] TypeOf<V, 0> ty() const;

  std::string_view keyword_;
};

/// The type `int`, which reads an integer
// NOLINTNEXTLINE(readability-identifier-naming): `int` is a keyword
inline constexpr BaseType<std::int64_t> int_{detail::HostName{}, "int"};
/// The type `float`, which reads a float
// NOLINTNEXTLINE(readability-identifier-naming): `float` is a keyword
inline constexpr BaseType<double> float_{detail::HostName{}, "float"};
/// The type `num`, which reads an integer or a float
inline constexpr BaseType<Number> num{detail::HostName{}, "num"};
/// The type `word`, which reads a word
inline constexpr BaseType<std::string> word{detail::HostName{}, "word"};
/// The type `rest`, which reads the rest of the line
inline constexpr BaseType<std::string> rest{detail::HostName{}, "rest"};

/// A bound of a Range: an operand of value type `std::int64_t`, Number or
/// Dynamic
class Bound {
public:
  /// The bound `value`
  template <Operand E>
    requires BoundValue<ValueOf<E>>
  Bound(E &&value); // NOLINT(google-explicit-constructor)

private:
  friend struct detail::Access;
  detail::Ops ops_;
};

/**
 * An integer range type, given with designated initializers: `{.from = 1,
 * .to = n}` is `1..n`, `{.from = 0, .below = n}` is `0..<n`, and either
 * bound may be left out
 */
struct Range {
  /// The lower bound
  std::optional<Bound> from = std::nullopt;
  /// The inclusive upper bound
  std::optional<Bound> to = std::nullopt;
  /// The exclusive upper bound
  std::optional<Bound> below = std::nullopt;

  /// This type with a dimension of `size` elements
  template <Operand E>
  [[nodiscard]] TypeOf<std::int64_t, 1> dim(E &&extent) const;
  /// This type read with the layout `layout`
  template <Operand E>
  [[nodiscard]] TypeOf<std::int64_t, 0> layout(E &&value) const;
  /// This type with `offset` added to every value read
  template <Operand E>
  [[nodiscard]] TypeOf<std::int64_t, 0> offset(E &&value) const;
  /// A list of this type that `end` ends: eol, or a sentinel value
  template <class E>
    requires std::same_as<std::remove_cvref_t<E>, Eol> || Operand<E>
  [[nodiscard]] ListReading<std::int64_t> until(E &&end) const;
};

/// An alphabet type, which reads symbols as integers; see alphabet
class Alphabet {
public:
  /// This alphabet, reading the characters `characters` as blanks
  [[nodiscard]] Alphabet blank(std::string_view characters) const;
  /// This alphabet, stripping the characters `characters`
  [[nodiscard]] Alphabet strip(std::string_view characters) const;
  /// This alphabet, numbering its symbols from `value`
  [[nodiscard]] Alphabet base(std::int64_t value) const;
  /// This type with a dimension of `size` elements
  template <Operand E>
  [[nodiscard]] TypeOf<std::int64_t, 1> dim(E &&extent) const;
  /// A list of this type that `end` ends: eol, or a sentinel value
  template <class E>
    requires std::same_as<std::remove_cvref_t<E>, Eol> || Operand<E>
  [[nodiscard]] ListReading<std::int64_t> until(E &&end) const;

private:
  friend struct detail::Access;
  explicit Alphabet(std::string_view symbols) : symbols_(symbols) {}

  std::string symbols_;
  std::vector<detail::AlphabetOption> options_;
};

/// A type that Scope::define_type declared, with value type V
template <class V> class NamedType {
public:
  /// Return the name of the type
  [[nodiscard]] std::string_view name() const { return name_.name; }

private:
  friend struct detail::Access;
  explicit NamedType(detail::TypeName name) : name_(name) {}

  detail::TypeName name_;
};

/// A list reading with value type V, as in `int_.until(eol)`
template <class V> class ListReading {
public:
  /// This list with `offset` added to every value read
  template <Operand E> [[nodiscard]] ListReading<V> offset(E &&value) const;

private:
  friend struct detail::Access;
  ListReading(detail::Declared declared, detail::ListEnd end)
      : declared_(std::move(declared)), end_(std::move(end)) {}

  detail::Declared declared_;
  detail::ListEnd end_;
};

/// The capture type that reads a token without a type, as `{x}` does
struct Untyped {};
/// A capture without a type, as `{x}`
inline constexpr Untyped untyped{};

/// A capture type: a type without dimensions, or untyped
template <class T>
concept CaptureType = std::same_as<std::remove_cvref_t<T>, Untyped> ||
                      (TypeLike<T> && type_dims<T> == 0);

namespace detail {
template <class T> struct CaptureValueOf {
  using Type = TypeValue<T>;
};
template <> struct CaptureValueOf<Untyped> {
  using Type = std::string;
};
} // namespace detail

/// The value type of a capture of type T
template <CaptureType T>
using CaptureValue = detail::CaptureValueOf<std::remove_cvref_t<T>>::Type;

/// The discard `_`: a target that stores nothing, and `{_}` in a pattern
struct Discard {};
/// `_`, which reads a value without storing it
inline constexpr Discard discard{};
/// `{}` in a pattern, which matches a token
struct Blank {};
/// `{}` in a pattern, which matches a token
inline constexpr Blank blank{};
/// `...` in a pattern, which matches the rest of the line
struct RestOfLine {};
/// `...` in a pattern, which matches the rest of the line
inline constexpr RestOfLine rest_of_line{};

/// A part of a pattern that bind, ref, validate or discard_as made
class Part {
private:
  friend struct detail::Access;
  explicit Part(detail::Piece piece) : piece_(std::move(piece)) {}

  detail::Piece piece_;
};

/// A part of a pattern: a capture handle, a Part, discard, blank or
/// rest_of_line
template <class T>
concept PatternPart = detail::IsField<std::remove_cvref_t<T>>::value ||
                      std::same_as<std::remove_cvref_t<T>, Part> ||
                      std::same_as<std::remove_cvref_t<T>, Discard> ||
                      std::same_as<std::remove_cvref_t<T>, Blank> ||
                      std::same_as<std::remove_cvref_t<T>, RestOfLine>;

/**
 * A pattern of a `line`, `find` or sections arm, built at run time
 *
 * pattern() builds one from a string literal checked at compile time; this
 * class builds one from text known only at run time, which build() checks.
 */
class Pattern {
public:
  /// Append the literal text `text`
  Pattern &text(std::string_view text) &;
  /// Append the literal text `text`
  [[nodiscard]] Pattern &&text(std::string_view text) &&;
  /// Append the part `part`
  template <PatternPart T> Pattern &part(const T &part) &;
  /// Append the part `part`
  template <PatternPart T> [[nodiscard]] Pattern &&part(const T &part) &&;

private:
  friend struct detail::Access;
  std::vector<detail::Piece> pieces_;
};

namespace detail {
// Called only where a pattern string is wrong, so that the compiler names
// the mistake: none of them is constexpr.
void pattern_has_an_unpaired_brace();
void pattern_spells_rest_of_line_as_text();
void pattern_placeholders_do_not_match_its_arguments();
} // namespace detail

/**
 * A pattern string with one `{}` for each of the types Parts, checked at
 * compile time
 *
 * Every `{` must be the start of a `{}`, and the text may not contain
 * `...`; use rest_of_line for that.
 */
template <class... Parts> class PatternString {
public:
  /// The pattern string `text`, a string literal
  template <class S>
    requires std::convertible_to<const S &, std::string_view>
  consteval PatternString(const S &text) // NOLINT(google-explicit-constructor)
      : text_(text) {
    std::size_t placeholders = 0;
    for (std::size_t i = 0; i < text_.size(); i++) {
      if (text_[i] == '{') {
        if (i + 1 >= text_.size() || text_[i + 1] != '}')
          detail::pattern_has_an_unpaired_brace();
        placeholders++;
        i++;
      } else if (text_.substr(i, 3) == "...") {
        detail::pattern_spells_rest_of_line_as_text();
      }
    }
    if (placeholders != sizeof...(Parts))
      detail::pattern_placeholders_do_not_match_its_arguments();
  }
  /// Return the text
  [[nodiscard]] constexpr std::string_view text() const { return text_; }

private:
  std::string_view text_;
};

/// The pattern of a statement: a Pattern, or a string literal without
/// placeholders, checked at compile time
class PatternArg {
public:
  /// The pattern `pattern`
  PatternArg(const Pattern &pattern) // NOLINT(google-explicit-constructor)
      : pattern_(&pattern) {}
  /// The pattern of the literal text `text`
  template <class S>
    requires std::convertible_to<const S &, std::string_view>
  consteval PatternArg(const S &text) // NOLINT(google-explicit-constructor)
      : text_(PatternString<>(text).text()) {}

private:
  friend struct detail::Access;
  const Pattern *pattern_ = nullptr;
  std::string_view text_;
};

/// A list of strings: one string, or a braced list or range of them. An
/// empty list stands for none.
class Strings {
public:
  /// No strings
  Strings() = default;
  /// The one string `text`
  Strings(std::string_view text) // NOLINT(google-explicit-constructor)
      : values_{std::string(text)} {}
  /// The one string `text`
  Strings(const char *text) // NOLINT(google-explicit-constructor)
      : Strings(std::string_view(text)) {}
  /// The one string `text`
  Strings(const std::string &text) // NOLINT(google-explicit-constructor)
      : Strings(std::string_view(text)) {}
  /// The strings `texts`
  Strings(std::initializer_list<std::string_view> texts)
      : values_(texts.begin(), texts.end()) {}
  /// The strings of the range `texts`
  template <std::ranges::input_range R>
    requires std::convertible_to<std::ranges::range_reference_t<const R &>,
                                 std::string_view> &&
             (!std::convertible_to<const R &, std::string_view>)
  Strings(const R &texts) { // NOLINT(google-explicit-constructor)
    for (auto &&text : texts)
      values_.emplace_back(std::string_view(text));
  }
  /// Return the strings
  [[nodiscard]] const std::vector<std::string> &values() const {
    return values_;
  }

private:
  std::vector<std::string> values_;
};

/// The target of a `store`: a new name, a handle, or nothing
class StoreTarget {
public:
  /// No store
  StoreTarget() = default;
  /// Store into the new name `name`
  StoreTarget(std::string_view name) // NOLINT(google-explicit-constructor)
      : target_{detail::Target::Kind::New, std::string(name), 0} {}
  /// Store into the new name `name`
  StoreTarget(const char *name) // NOLINT(google-explicit-constructor)
      : StoreTarget(std::string_view(name)) {}
  /// Store into the name of `handle`, as from forward
  template <class V, std::size_t D, Presence P>
  StoreTarget(
      const Field<V, D, P> &handle); // NOLINT(google-explicit-constructor)
  /// `store _`, which cannot be spelled: use a handle from `forward("_")`
  StoreTarget(Discard /*discard*/) // NOLINT(google-explicit-constructor)
      : target_{detail::Target::Kind::Discard, {}, 0} {}

private:
  friend struct detail::Access;
  detail::Target target_;
};

/// The options of Scope::repeat_until
struct UntilOptions {
  /// The target that keeps the terminator line
  StoreTarget store = {}; // NOLINT(readability-redundant-member-init)
};

/// One clause of the options of Scope::sections
struct SectionsOptions {
  /// The lines that end the sections; none for no `until`
  Strings until = {}; // NOLINT(readability-redundant-member-init)
  /// The target that keeps the terminator line; it needs `until`
  StoreTarget store = {}; // NOLINT(readability-redundant-member-init)
  /// Whether lines that start no section are skipped
  bool skip_other = false;
};

/// The options of an arm of Arms
struct ArmOptions {
  /// Whether the sections end after this arm
  bool last = false;
};

namespace detail {

/// Access to the private parts of the public types
struct Access {
  template <class V, std::size_t D, Presence P>
  static Field<V, D, P> field(HandleName name) {
    return Field<V, D, P>(name.name, name.owner);
  }
  template <class V, std::size_t D, Presence P>
  static BuilderId owner(const Field<V, D, P> &field) {
    return field.owner_;
  }
  template <class V, class S> static Expr<V, S> expr(Ops ops) {
    return Expr<V, S>(std::move(ops));
  }
  template <class V, class S> static const Ops &ops(const Expr<V, S> &e) {
    return e.ops_;
  }
  template <class V, class S> static Ops &&ops(Expr<V, S> &&e) {
    return std::move(e).ops_;
  }
  static Record record(std::size_t id, BuilderId owner) { return {id, owner}; }
  static std::size_t record_id(const Record &r) { return r.id_; }
  static BuilderId record_owner(const Record &r) { return r.owner_; }
  static RecordExpr record_expr(Ops ops) { return RecordExpr(std::move(ops)); }
  static RecordList record_list(Ops ops) { return RecordList(std::move(ops)); }
  static const Ops &ops(const RecordExpr &e) { return e.ops_; }
  static const Ops &ops(const RecordList &e) { return e.ops_; }
  template <class T>
  static Over<T> over(T domain, std::optional<std::string_view> name) {
    return Over<T>(std::move(domain), name);
  }
  template <class T> static const T &domain(const Over<T> &o) {
    return o.domain_;
  }
  template <class T>
  static std::optional<std::string_view> label(const Over<T> &o) {
    return o.name_;
  }
  template <class V, std::size_t N> static TypeOf<V, N> type(Declared d) {
    return TypeOf<V, N>(std::move(d));
  }
  template <class V, std::size_t N>
  static const Declared &declared(const TypeOf<V, N> &t) {
    return t.declared_;
  }
  static Alphabet alphabet(std::string_view symbols) {
    return Alphabet(symbols);
  }
  static Declared declared(const Alphabet &a) {
    Declared d;
    d.type.kind = TypeSpec::Kind::Alphabet;
    d.type.symbols = a.symbols_;
    d.type.options = a.options_;
    return d;
  }
  static std::vector<AlphabetOption> &options(Alphabet &a) {
    return a.options_;
  }
  template <class V> static NamedType<V> named(TypeName name) {
    return NamedType<V>(name);
  }
  template <class V> static TypeName type_name(const NamedType<V> &t) {
    return t.name_;
  }
  template <class V>
  static ListReading<V> list(Declared declared, ListEnd end) {
    return ListReading<V>(std::move(declared), std::move(end));
  }
  template <class V> static const Declared &declared(const ListReading<V> &l) {
    return l.declared_;
  }
  template <class V> static const ListEnd &end(const ListReading<V> &l) {
    return l.end_;
  }
  static const Ops &ops(const Bound &b) { return b.ops_; }
  static Ops &bound_ops(Bound &b) { return b.ops_; }
  static Part part(Piece piece) { return Part(std::move(piece)); }
  static const Piece &piece(const Part &p) { return p.piece_; }
  static std::vector<Piece> &pieces(Pattern &p) { return p.pieces_; }
  static std::vector<Piece> pieces(const PatternArg &p) {
    if (p.pattern_ != nullptr)
      return p.pattern_->pieces_;
    std::vector<Piece> pieces;
    if (!p.text_.empty())
      pieces.push_back(Piece{.kind = Piece::Kind::Text,
                             .text = std::string(p.text_),
                             .name = {},
                             .owner = 0,
                             .value = ValueTag::Str,
                             .type = std::nullopt});
    return pieces;
  }
  static const Target &target(const StoreTarget &s) { return s.target_; }
  static Target &target(StoreTarget &s) { return s.target_; }
  static BuildError error(BuildErrorKind kind, std::string message,
                          std::optional<Error> language_error,
                          std::optional<std::string> text,
                          std::optional<std::source_location> call_site) {
    return {kind, std::move(message), std::move(language_error),
            std::move(text), call_site};
  }
  template <class S>
  static S scope(std::shared_ptr<State> state, std::size_t block,
                 std::optional<std::size_t> record) {
    return S(std::move(state), block, record);
  }
  template <class R, class Result>
  static R branch(std::shared_ptr<State> state, std::size_t block,
                  std::optional<std::size_t> record, std::size_t close,
                  Result &&result) {
    return R(std::move(state), block, record, close,
             std::forward<Result>(result));
  }
};

inline Op name_op(std::string_view name, BuilderId owner) {
  Op op;
  op.kind = OpKind::Name;
  op.name = name;
  op.owner = owner;
  return op;
}

inline Op kind_op(OpKind kind) {
  Op op;
  op.kind = kind;
  return op;
}

template <class V, std::size_t H, Presence P>
struct OperandTraits<Field<V, H, P>> {
  using Value = V;
  using Shape = Open;
  static constexpr bool builder = true;
  static constexpr std::size_t depth_in(std::size_t d) {
    return d == 0 ? H : dynamic_depth;
  }
  static Ops ops(const Field<V, H, P> &field) {
    return {name_op(field.name(), Access::owner(field))};
  }
};

template <class V, class S> struct OperandTraits<Expr<V, S>> {
  using Value = V;
  using Shape = S;
  static constexpr bool builder = true;
  static constexpr std::size_t depth_in(std::size_t d) {
    if constexpr (std::same_as<V, Dynamic> || std::same_as<S, Open>)
      return dynamic_depth;
    else
      return d;
  }
  static Ops ops(const Expr<V, S> &e) { return Access::ops(e); }
  static Ops ops(Expr<V, S> &&e) { return Access::ops(std::move(e)); }
};

template <> struct OperandTraits<Host> {
  using Value = std::int64_t;
  using Shape = Scalar;
  static constexpr bool builder = true;
  static constexpr std::size_t depth_in(std::size_t d) { return d; }
  static Ops ops(const Host &host) {
    Op op = kind_op(OpKind::Host);
    op.name = host.name();
    return {op};
  }
};

template <LiteralInteger T> struct OperandTraits<T> {
  using Value = std::int64_t;
  using Shape = Scalar;
  static constexpr bool builder = false;
  static constexpr std::size_t depth_in(std::size_t d) { return d; }
  static Ops ops(T value) {
    Op op = kind_op(OpKind::Int);
    op.integer = static_cast<std::int64_t>(value);
    return {op};
  }
};

template <std::floating_point T> struct OperandTraits<T> {
  using Value = double;
  using Shape = Scalar;
  static constexpr bool builder = false;
  static constexpr std::size_t depth_in(std::size_t d) { return d; }
  static Ops ops(T value) {
    Op op = kind_op(OpKind::Float);
    op.number = static_cast<double>(value);
    return {op};
  }
};

template <LiteralString T> struct OperandTraits<T> {
  using Value = std::string;
  using Shape = Scalar;
  static constexpr bool builder = false;
  static constexpr std::size_t depth_in(std::size_t d) { return d; }
  static Ops ops(std::string_view text) {
    Op op = kind_op(OpKind::Str);
    op.text = std::make_shared<const std::string>(text);
    return {op};
  }
};

/// Return the operations of `operand`
template <Operand T> Ops ops_of(T &&operand) {
  return OperandTraits<Bare<T>>::ops(std::forward<T>(operand));
}

template <class T>
concept BuilderOperand = Operand<T> && OperandTraits<Bare<T>>::builder;

/// The operands of a binary operator: at least one comes from a builder
template <class L, class R>
concept Operands =
    Operand<L> && Operand<R> && (BuilderOperand<L> || BuilderOperand<R>);

/// The depth of operand T in a scope of depth D
template <Operand T, std::size_t D>
inline constexpr std::size_t depth_in = OperandTraits<Bare<T>>::depth_in(D);

/// The shape of operand T: Scalar, or Open for a lookup
template <Operand T> using ShapeOf = OperandTraits<Bare<T>>::Shape;

/// The depth of a handle that `let` defines from operand T in a scope of
/// depth D
template <Operand T, std::size_t D>
inline constexpr std::size_t let_depth =
    (std::same_as<ValueOf<T>, Dynamic> || std::same_as<ShapeOf<T>, Open>)
        ? dynamic_depth
        : D;

/// The levels that a `let` of `ops`, an operand T, defines in a scope of
/// depth D
template <Operand T, std::size_t D>
std::optional<std::size_t> let_levels(const Ops &ops) {
  constexpr bool scalar =
      !std::same_as<ValueOf<T>, Dynamic> &&
      std::same_as<typename OperandTraits<Bare<T>>::Shape, Scalar>;
  if constexpr (scalar) {
    const OpKind last = ops.back().kind;
    if (last == OpKind::Name || last == OpKind::Index || last == OpKind::Member)
      return std::nullopt;
  }
  constexpr std::size_t in = depth_in<T, D>;
  if constexpr (in == dynamic_depth || D == dynamic_depth)
    return std::nullopt;
  else
    return in - D;
}

/// The levels a `param` with a default of operand T defines
template <Operand T>
inline constexpr std::optional<std::size_t> scalar_levels =
    let_depth<T, 0> == dynamic_depth ? std::nullopt
                                     : std::optional<std::size_t>(0);

template <class Out, class L, class R>
Expr<Out, Scalar> binary(L &&left, R &&right, BinaryOp op) {
  Ops ops = ops_of(std::forward<L>(left));
  append(ops, ops_of(std::forward<R>(right)));
  Op last = kind_op(OpKind::Binary);
  last.code = static_cast<std::uint8_t>(op);
  ops.push_back(last);
  return Access::expr<Out, Scalar>(std::move(ops));
}

template <class Out, class T> Expr<Out, Scalar> unary(T &&operand, UnaryOp op) {
  Ops ops = ops_of(std::forward<T>(operand));
  Op last = kind_op(OpKind::Unary);
  last.code = static_cast<std::uint8_t>(op);
  ops.push_back(last);
  return Access::expr<Out, Scalar>(std::move(ops));
}

template <class I> Ops index_ops(Ops ops, I &&index) {
  append(ops, ops_of(std::forward<I>(index)));
  ops.push_back(kind_op(OpKind::Index));
  return ops;
}

template <class V, std::size_t D, Presence P>
Ops member_ops(Ops ops, const Field<V, D, P> &field) {
  Op op = kind_op(OpKind::Member);
  op.name = field.name();
  op.owner = Access::owner(field);
  ops.push_back(op);
  return ops;
}

inline Op record_op(OpKind kind, const Record &record) {
  Op op = kind_op(kind);
  op.id = Access::record_id(record);
  op.owner = Access::record_owner(record);
  return op;
}

/// What a domain of a comprehension contributes; specialized for each
template <class T> struct DomainTraits {};

template <class V, std::size_t H, Presence P>
struct DomainTraits<Field<V, H, P>> {
  using Item = Expr<V, Open>;
  static Ops ops(const Field<V, H, P> &f) { return ops_of(f); }
  static std::optional<std::string_view> label(const Field<V, H, P> &) {
    return std::nullopt;
  }
  static Item item(Ops ops) { return Access::expr<V, Open>(std::move(ops)); }
};

template <class V, class S> struct DomainTraits<Expr<V, S>> {
  using Item = Expr<V, Open>;
  static Ops ops(const Expr<V, S> &e) { return Access::ops(e); }
  static std::optional<std::string_view> label(const Expr<V, S> &) {
    return std::nullopt;
  }
  static Item item(Ops ops) { return Access::expr<V, Open>(std::move(ops)); }
};

template <> struct DomainTraits<Record> {
  using Item = RecordExpr;
  static Ops ops(const Record &r) { return {record_op(OpKind::RecordList, r)}; }
  static std::optional<std::string_view> label(const Record &) {
    return std::nullopt;
  }
  static Item item(Ops ops) { return Access::record_expr(std::move(ops)); }
};

template <> struct DomainTraits<RecordList> {
  using Item = RecordExpr;
  static Ops ops(const RecordList &r) { return Access::ops(r); }
  static std::optional<std::string_view> label(const RecordList &) {
    return std::nullopt;
  }
  static Item item(Ops ops) { return Access::record_expr(std::move(ops)); }
};

template <class T> struct DomainTraits<Over<T>> {
  using Item = DomainTraits<T>::Item;
  static Ops ops(const Over<T> &o) {
    return DomainTraits<T>::ops(Access::domain(o));
  }
  static std::optional<std::string_view> label(const Over<T> &o) {
    return Access::label(o);
  }
  static Item item(Ops ops) { return DomainTraits<T>::item(std::move(ops)); }
};

} // namespace detail

/// A domain of a comprehension: an array handle or expression, a Record, a
/// RecordList, or an Over that names the element
template <class T>
concept Domain =
    requires { typename detail::DomainTraits<detail::Bare<T>>::Item; };

/// The element that a comprehension over domain T passes to its body
template <Domain T> using ItemOf = detail::DomainTraits<detail::Bare<T>>::Item;

/// An expression that `defined` takes: a handle, an expression or records
template <class T>
concept Definable = detail::IsField<detail::Bare<T>>::value ||
                    detail::IsExpr<detail::Bare<T>>::value ||
                    std::same_as<detail::Bare<T>, Record> ||
                    std::same_as<detail::Bare<T>, RecordExpr> ||
                    std::same_as<detail::Bare<T>, RecordList>;

/// The handles, records and expressions that a builder statement takes
/// through [] and dynamic() on an iris::Field
class Indexing {
public:
  /// Element `index` of the entry of `field`
  template <class V, std::size_t D, Presence P, Operand I>
    requires Integral<ValueOf<I>>
  static Expr<V, Open> index(const Field<V, D, P> &field, I &&index) {
    return detail::Access::expr<V, Open>(
        detail::index_ops(detail::ops_of(field), std::forward<I>(index)));
  }
  /// The entry of `field` with its type and depth forgotten
  template <class V, std::size_t D, Presence P>
  static Expr<Dynamic, Scalar> dynamic(const Field<V, D, P> &field) {
    return detail::Access::expr<Dynamic, Scalar>(detail::ops_of(field));
  }
};

/*
 * Operators. Each takes at least one operand from a builder, so they never
 * apply to plain values. Defining `!=` keeps C++20 from rewriting `==`.
 */

/// Addition, `a + b`
template <class L, class R>
  requires detail::Operands<L, R> && Integral<ValueOf<L>> &&
           Integral<ValueOf<R>>
Expr<std::int64_t> operator+(L &&left, R &&right) {
  return detail::binary<std::int64_t>(
      std::forward<L>(left), std::forward<R>(right), detail::BinaryOp::Add);
}
/// Subtraction, `a - b`
template <class L, class R>
  requires detail::Operands<L, R> && Integral<ValueOf<L>> &&
           Integral<ValueOf<R>>
Expr<std::int64_t> operator-(L &&left, R &&right) {
  return detail::binary<std::int64_t>(std::forward<L>(left),
                                      std::forward<R>(right),
                                      detail::BinaryOp::Subtract);
}
/// Multiplication, `a * b`
template <class L, class R>
  requires detail::Operands<L, R> && Integral<ValueOf<L>> &&
           Integral<ValueOf<R>>
Expr<std::int64_t> operator*(L &&left, R &&right) {
  return detail::binary<std::int64_t>(std::forward<L>(left),
                                      std::forward<R>(right),
                                      detail::BinaryOp::Multiply);
}
/// Integer division, `a / b`
template <class L, class R>
  requires detail::Operands<L, R> && Integral<ValueOf<L>> &&
           Integral<ValueOf<R>>
Expr<std::int64_t> operator/(L &&left, R &&right) {
  return detail::binary<std::int64_t>(
      std::forward<L>(left), std::forward<R>(right), detail::BinaryOp::Divide);
}
/// Remainder, `a % b`
template <class L, class R>
  requires detail::Operands<L, R> && Integral<ValueOf<L>> &&
           Integral<ValueOf<R>>
Expr<std::int64_t> operator%(L &&left, R &&right) {
  return detail::binary<std::int64_t>(std::forward<L>(left),
                                      std::forward<R>(right),
                                      detail::BinaryOp::Remainder);
}
/// Conjunction, `a && b`
template <class L, class R>
  requires detail::Operands<L, R> && Integral<ValueOf<L>> &&
           Integral<ValueOf<R>>
Expr<std::int64_t> operator&&(L &&left, R &&right) {
  return detail::binary<std::int64_t>(
      std::forward<L>(left), std::forward<R>(right), detail::BinaryOp::And);
}
/// Disjunction, `a || b`
template <class L, class R>
  requires detail::Operands<L, R> && Integral<ValueOf<L>> &&
           Integral<ValueOf<R>>
Expr<std::int64_t> operator||(L &&left, R &&right) {
  return detail::binary<std::int64_t>(
      std::forward<L>(left), std::forward<R>(right), detail::BinaryOp::Or);
}
/// Equality, `a == b`
template <class L, class R>
  requires detail::Operands<L, R> && Equatable<ValueOf<L>, ValueOf<R>>
Expr<std::int64_t> operator==(L &&left, R &&right) {
  return detail::binary<std::int64_t>(
      std::forward<L>(left), std::forward<R>(right), detail::BinaryOp::Equal);
}
/// Inequality, `a != b`
template <class L, class R>
  requires detail::Operands<L, R> && Equatable<ValueOf<L>, ValueOf<R>>
Expr<std::int64_t> operator!=(L &&left, R &&right) {
  return detail::binary<std::int64_t>(std::forward<L>(left),
                                      std::forward<R>(right),
                                      detail::BinaryOp::NotEqual);
}
/// Comparison, `a < b`
template <class L, class R>
  requires detail::Operands<L, R> && Numeric<ValueOf<L>> && Numeric<ValueOf<R>>
Expr<std::int64_t> operator<(L &&left, R &&right) {
  return detail::binary<std::int64_t>(
      std::forward<L>(left), std::forward<R>(right), detail::BinaryOp::Less);
}
/// Comparison, `a <= b`
template <class L, class R>
  requires detail::Operands<L, R> && Numeric<ValueOf<L>> && Numeric<ValueOf<R>>
Expr<std::int64_t> operator<=(L &&left, R &&right) {
  return detail::binary<std::int64_t>(std::forward<L>(left),
                                      std::forward<R>(right),
                                      detail::BinaryOp::LessEqual);
}
/// Comparison, `a > b`
template <class L, class R>
  requires detail::Operands<L, R> && Numeric<ValueOf<L>> && Numeric<ValueOf<R>>
Expr<std::int64_t> operator>(L &&left, R &&right) {
  return detail::binary<std::int64_t>(
      std::forward<L>(left), std::forward<R>(right), detail::BinaryOp::Greater);
}
/// Comparison, `a >= b`
template <class L, class R>
  requires detail::Operands<L, R> && Numeric<ValueOf<L>> && Numeric<ValueOf<R>>
Expr<std::int64_t> operator>=(L &&left, R &&right) {
  return detail::binary<std::int64_t>(std::forward<L>(left),
                                      std::forward<R>(right),
                                      detail::BinaryOp::GreaterEqual);
}
/// Negation, `-a`, of the same value type
template <class T>
  requires detail::BuilderOperand<T> && Numeric<ValueOf<T>>
Expr<ValueOf<T>> operator-(T &&operand) {
  return detail::unary<ValueOf<T>>(std::forward<T>(operand),
                                   detail::UnaryOp::Minus);
}
/// Logical negation, `!a`
template <class T>
  requires detail::BuilderOperand<T> && Integral<ValueOf<T>>
Expr<std::int64_t> operator!(T &&operand) {
  return detail::unary<std::int64_t>(std::forward<T>(operand),
                                     detail::UnaryOp::Not);
}

namespace detail {

template <class T>
concept Body = std::is_invocable_v<T>;

struct LitFn {
  template <Operand T>
    requires(!BuilderOperand<T> || std::same_as<Bare<T>, Host>)
  Expr<ValueOf<T>> operator()(T &&value) const {
    return Access::expr<ValueOf<T>, Scalar>(ops_of(std::forward<T>(value)));
  }
};

struct PowFn {
  template <class L, class R>
    requires Operand<L> && Operand<R> && Integral<ValueOf<L>> &&
             Integral<ValueOf<R>>
  Expr<std::int64_t> operator()(L &&base, R &&exponent) const {
    return binary<std::int64_t>(std::forward<L>(base),
                                std::forward<R>(exponent), BinaryOp::Power);
  }
};

inline Expr<std::int64_t, Scalar> call(std::string_view function, Ops ops,
                                       std::size_t arguments) {
  Op op = kind_op(OpKind::Call);
  op.name = function;
  op.integer = static_cast<std::int64_t>(arguments);
  ops.push_back(op);
  return Access::expr<std::int64_t, Scalar>(std::move(ops));
}

template <class T, class F>
Expr<std::int64_t, Scalar> comprehension(std::string_view function, T &&domain,
                                         F &&body) {
  using Traits = DomainTraits<Bare<T>>;
  const std::uint64_t binding = next_binding();
  const std::optional<std::string_view> label = Traits::label(domain);
  Ops ops = Traits::ops(std::forward<T>(domain));
  Op element = kind_op(OpKind::Binding);
  element.id = binding;
  append(ops, ops_of(std::invoke(std::forward<F>(body),
                                 Traits::item(Ops{element}))));
  Op op = kind_op(OpKind::Comprehension);
  op.name = function;
  op.id = binding;
  if (label) {
    op.labelled = true;
    op.label = *label;
  }
  ops.push_back(op);
  return Access::expr<std::int64_t, Scalar>(std::move(ops));
}

template <class T, class F>
concept ComprehensionBody = Domain<T> && std::invocable<F, ItemOf<T>> &&
                            Operand<std::invoke_result_t<F, ItemOf<T>>>;

template <class R>
concept OperandRange =
    std::ranges::input_range<R> && Operand<std::ranges::range_reference_t<R>> &&
    (!Operand<R>);

/// A function of Iris over one or more operands: `min`, `max`, `sum`,
/// `distinct`
struct Aggregate {
  std::string_view function;

  template <Operand... A>
    requires(sizeof...(A) >= 1)
  Expr<std::int64_t> operator()(A &&...arguments) const {
    Ops ops;
    (append(ops, ops_of(std::forward<A>(arguments))), ...);
    return call(function, std::move(ops), sizeof...(A));
  }
  template <OperandRange R> Expr<std::int64_t> operator()(R &&arguments) const {
    Ops ops;
    std::size_t n = 0;
    for (auto &&argument : std::forward<R>(arguments)) {
      append(ops, ops_of(std::forward<decltype(argument)>(argument)));
      n++;
    }
    return call(function, std::move(ops), n);
  }
};

struct MinFn : Aggregate {};
struct MaxFn : Aggregate {};
struct DistinctFn : Aggregate {};

struct SumFn : Aggregate {
  using Aggregate::operator();
  template <class T, class F>
    requires ComprehensionBody<T, F> && (!Operand<F>)
  Expr<std::int64_t> operator()(T &&domain, F &&body) const {
    return comprehension("sum", std::forward<T>(domain), std::forward<F>(body));
  }
};

struct CountFn {
  template <Domain T> Expr<std::int64_t> operator()(T &&domain) const {
    return call("count", DomainTraits<Bare<T>>::ops(std::forward<T>(domain)),
                1);
  }
  template <class T, class F>
    requires ComprehensionBody<T, F>
  Expr<std::int64_t> operator()(T &&domain, F &&body) const {
    return comprehension("count", std::forward<T>(domain),
                         std::forward<F>(body));
  }
};

struct SizeFn {
  template <Domain T> Expr<std::int64_t> operator()(T &&domain) const {
    return call("size", DomainTraits<Bare<T>>::ops(std::forward<T>(domain)), 1);
  }
};

struct QuantifierFn {
  std::string_view function;
  template <class T, class F>
    requires ComprehensionBody<T, F>
  Expr<std::int64_t> operator()(T &&domain, F &&body) const {
    return comprehension(function, std::forward<T>(domain),
                         std::forward<F>(body));
  }
};

struct OverFn {
  template <Domain T> Over<Bare<T>> operator()(T &&domain) const {
    return Access::over<Bare<T>>(std::forward<T>(domain), std::nullopt);
  }
};

struct DefinedFn {
  template <Definable T> Expr<std::int64_t> operator()(const T &name) const {
    if constexpr (std::same_as<T, Record>)
      return call("defined", DomainTraits<Record>::ops(name), 1);
    else if constexpr (std::same_as<T, RecordExpr> ||
                       std::same_as<T, RecordList>)
      return call("defined", Access::ops(name), 1);
    else
      return call("defined", ops_of(name), 1);
  }
};

template <class T> std::optional<Declared> capture_declared(const T &type) {
  if constexpr (std::same_as<T, Untyped>)
    return std::nullopt;
  else
    return TypeTraits<T>::declared(type);
}

template <PatternPart T> Piece piece_of(const T &part) {
  if constexpr (IsField<T>::value)
    return Piece{.kind = Piece::Kind::Capture,
                 .text = {},
                 .name = part.name(),
                 .owner = Access::owner(part),
                 .value = ValueTag::Str,
                 .type = std::nullopt};
  else if constexpr (std::same_as<T, Part>)
    return Access::piece(part);
  else {
    Piece piece;
    if constexpr (std::same_as<T, Discard>)
      piece.kind = Piece::Kind::Discard;
    else if constexpr (std::same_as<T, Blank>)
      piece.kind = Piece::Kind::Blank;
    else
      piece.kind = Piece::Kind::RestOfLine;
    return piece;
  }
}

void append_text(std::vector<Piece> &pieces, std::string_view text);

struct PatternFn {
  template <PatternPart... Parts>
  Pattern operator()(PatternString<std::type_identity_t<Parts>...> text,
                     const Parts &...parts) const {
    std::vector<Piece> given;
    given.reserve(sizeof...(Parts));
    (given.push_back(piece_of(parts)), ...);
    Pattern result;
    std::vector<Piece> &pieces = Access::pieces(result);
    const std::string_view source = text.text();
    std::size_t start = 0;
    std::size_t next = 0;
    for (std::size_t i = 0; i + 1 < source.size(); i++)
      if (source[i] == '{') {
        append_text(pieces, source.substr(start, i - start));
        pieces.push_back(std::move(given[next++]));
        start = i + 2;
        i++;
      }
    append_text(pieces, source.substr(start));
    return result;
  }
};

struct BindFn {
  template <class V, std::size_t D, Presence P, CaptureType T>
  Part operator()(const Field<V, D, P> &handle, const T &type) const {
    return Access::part(
        Piece{.kind = Piece::Kind::Bind,
              .text = {},
              .name = handle.name(),
              .owner = Access::owner(handle),
              .value = value_tag<CaptureValue<T>>(),
              .type = capture_declared<std::remove_cvref_t<T>>(type)});
  }
};

struct RefFn {
  template <class V, std::size_t D, Presence P>
  Part operator()(const Field<V, D, P> &handle) const {
    return Access::part(Piece{.kind = Piece::Kind::Reference,
                              .text = {},
                              .name = handle.name(),
                              .owner = Access::owner(handle),
                              .value = ValueTag::Str,
                              .type = std::nullopt});
  }
};

struct TypedPartFn {
  Piece::Kind kind;
  template <CaptureType T> Part operator()(const T &type) const {
    return Access::part(
        Piece{.kind = kind,
              .text = {},
              .name = {},
              .owner = 0,
              .value = value_tag<CaptureValue<T>>(),
              .type = capture_declared<std::remove_cvref_t<T>>(type)});
  }
};

struct AlphabetFn {
  Alphabet operator()(std::string_view symbols) const {
    return Access::alphabet(symbols);
  }
};

struct TyFn {
  template <TypeLike T>
  TypeOf<TypeValue<T>, type_dims<T>> operator()(const T &type) const {
    return Access::type<TypeValue<T>, type_dims<T>>(
        TypeTraits<std::remove_cvref_t<T>>::declared(type));
  }
};

template <class V> struct TypeTraits<BaseType<V>> {
  using Value = V;
  static constexpr std::size_t dims = 0;
  static Declared declared(const BaseType<V> &type) {
    Declared d;
    d.type.base = type.keyword();
    return d;
  }
};

template <class V, std::size_t N> struct TypeTraits<TypeOf<V, N>> {
  using Value = V;
  static constexpr std::size_t dims = N;
  static Declared declared(const TypeOf<V, N> &type) {
    return Access::declared(type);
  }
};

template <> struct TypeTraits<Range> {
  using Value = std::int64_t;
  static constexpr std::size_t dims = 0;
  static Declared declared(const Range &range) {
    Declared d;
    d.type.kind = TypeSpec::Kind::Range;
    if (range.from)
      d.type.lower = Access::ops(*range.from);
    if (range.to) {
      d.type.upper = Access::ops(*range.to);
      d.type.inclusive = true;
    }
    if (range.below) {
      d.type.both_upper = range.to.has_value();
      d.type.upper = Access::ops(*range.below);
      d.type.inclusive = false;
    }
    return d;
  }
};

template <> struct TypeTraits<Alphabet> {
  using Value = std::int64_t;
  static constexpr std::size_t dims = 0;
  static Declared declared(const Alphabet &alphabet) {
    return Access::declared(alphabet);
  }
};

template <class V> struct TypeTraits<NamedType<V>> {
  using Value = V;
  static constexpr std::size_t dims = 0;
  static Declared declared(const NamedType<V> &type) {
    const TypeName name = Access::type_name(type);
    Declared d;
    d.type.kind = TypeSpec::Kind::Named;
    d.type.named = name.name;
    d.type.owner = name.owner;
    d.type.declaration = name.declaration;
    return d;
  }
};

template <TypeLike T> Declared declared_of(const T &type) {
  return TypeTraits<std::remove_cvref_t<T>>::declared(type);
}

template <class E> ListEnd list_end(E &&end) {
  if constexpr (std::same_as<std::remove_cvref_t<E>, Eol>)
    return ListEnd{.eol = true, .sentinel = {}};
  else
    return ListEnd{.eol = false, .sentinel = ops_of(std::forward<E>(end))};
}

/// What a definition's target gives: a new name gives a typed handle, an
/// existing handle a DynField, and discard nothing
template <class T> struct TargetTraits {};

template <LiteralString T> struct TargetTraits<T> {
  template <class V, std::size_t D, Presence P> using Out = Field<V, D, P>;
  static Target target(std::string_view name) {
    return Target{Target::Kind::New, std::string(name), 0};
  }
};

template <class A, std::size_t B, Presence C>
struct TargetTraits<Field<A, B, C>> {
  template <class V, std::size_t D, Presence P> using Out = DynField;
  static Target target(const Field<A, B, C> &handle) {
    return Target{Target::Kind::Existing, std::string(handle.name()),
                  Access::owner(handle)};
  }
};

template <> struct TargetTraits<Discard> {
  template <class V, std::size_t D, Presence P> using Out = void;
  static Target target(Discard /*discard*/) {
    return Target{Target::Kind::Discard, {}, 0};
  }
};

template <class T, class V, std::size_t D, Presence P>
using TargetOut = TargetTraits<Bare<T>>::template Out<V, D, P>;

template <class Out> Out make_handle(HandleName name) {
  if constexpr (!std::is_void_v<Out>)
    return Access::field<typename Out::Value, Out::depth, Out::presence>(name);
}

/// The result of a body, held by an If; nothing for void
template <class R> struct Held {
  R value;
};
template <> struct Held<void> {};

/// Nothing to do after a block
struct NoAfter {
  void operator()() const {}
};

/// Run `body` on the scope S of a new block, close the block, run `after`,
/// and return the result of the body
template <class S, class F, class A = NoAfter>
std::invoke_result_t<F, S &> enter(const std::shared_ptr<State> &state,
                                   std::optional<std::size_t> record, F &&body,
                                   Site site, A after = {}) {
  S scope = Access::scope<S>(state, open_block(*state), record);
  using R = std::invoke_result_t<F, S &>;
  if constexpr (std::is_void_v<R>) {
    std::invoke(std::forward<F>(body), scope);
    close_block(*state, site);
    after();
  } else {
    decltype(auto) result = std::invoke(std::forward<F>(body), scope);
    close_block(*state, site);
    after();
    return std::forward<R>(result);
  }
}

} // namespace detail

/// A literal or host constant as an expression, as in `lit(1)` or
/// `lit(host::int_max)`
inline constexpr detail::LitFn lit{};
/// `pow(a, b)`, which Iris spells `a ^ b`. Where the C library's `::pow` is
/// declared, as `<math.h>` and MSVC's standard headers declare it,
/// `using namespace iris::build;` makes an unqualified `pow` ambiguous, so
/// add `using iris::build::pow;` beside it.
inline constexpr detail::PowFn pow{};
/// `min(a, b, ...)`, or `min(row)` over an array; also takes a range of
/// operands
inline constexpr detail::MinFn min{{"min"}};
/// `max(a, b, ...)`, or `max(row)` over an array; also takes a range of
/// operands
inline constexpr detail::MaxFn max{{"max"}};
/// `distinct(a, b, ...)`, or `distinct(row)` over an array; also takes a
/// range of operands
inline constexpr detail::DistinctFn distinct{{"distinct"}};
/// `sum(a, b, ...)`, `sum(row)`, or the comprehension `sum(domain, body)`
/// that adds the body over the elements of a domain
inline constexpr detail::SumFn sum{{"sum"}};
/// `count(domain)`, the number of defined elements, or the comprehension
/// `count(domain, body)` that counts the elements for which the body holds
inline constexpr detail::CountFn count{};
/// `size(domain)`, the number of elements
inline constexpr detail::SizeFn size{};
/// The comprehension `all(domain, body)`: whether the body holds for every
/// element
inline constexpr detail::QuantifierFn all{"all"};
/// The comprehension `any(domain, body)`: whether the body holds for some
/// element
inline constexpr detail::QuantifierFn any{"any"};
/// A domain whose element the comprehension names, as in
/// `sum(over(row).named("x"), body)`
inline constexpr detail::OverFn over{};
/// `defined(x)`: whether a name, element or field is defined
inline constexpr detail::DefinedFn defined{};
/// A pattern from a string literal with one `{}` for each part, checked at
/// compile time, as in `pattern("p cnf {} {}", vars, clauses)`
inline constexpr detail::PatternFn pattern{};
/// `{x:type}` in a pattern, which defines the handle's name
inline constexpr detail::BindFn bind{};
/// `{$x}` in a pattern, which matches the value of the handle's entry
inline constexpr detail::RefFn ref{};
/// `{:type}` in a pattern, which checks a token without storing it
inline constexpr detail::TypedPartFn validate{detail::Piece::Kind::Validate};
/// `{_:type}` in a pattern, which reads a token of a type and discards it
inline constexpr detail::TypedPartFn discard_as{detail::Piece::Kind::DiscardAs};
/// The alphabet type with the symbols `symbols`, as in `alphabet("01")`
inline constexpr detail::AlphabetFn alphabet{};
/// A type as a TypeOf, which has every method of a type, as in
/// `ty(byte).until(eol)`
inline constexpr detail::TyFn ty{};

/**
 * The branch of an `if`: else_if, else_ and else_read_if continue it, each
 * directly after the branch before
 *
 * @tparam R the result of the body of the `if`
 */
template <std::size_t D, Presence P, class R> class If {
public:
  /// Return the result of the body of the `if`
  template <class T = R>
    requires(!std::is_void_v<T>)
  [[nodiscard]] T &result() {
    return held_.value;
  }

  /// `else if condition { body }`; returns the result of the body
  template <Operand C, class F>
    requires std::invocable<F, Scope<D, Presence::Maybe> &>
  std::invoke_result_t<F, Scope<D, Presence::Maybe> &>
  else_if(C &&condition, F &&body,
          std::source_location site = std::source_location::current()) {
    detail::check_last(*state_, close_, site);
    detail::computed(*state_, block_, detail::Computed::ElseIf,
                     detail::ops_of(std::forward<C>(condition)), site);
    return detail::enter<Scope<D, Presence::Maybe>>(
        state_, record_, std::forward<F>(body), site,
        [this] { close_ = detail::statement_count(*state_) - 1; });
  }

  /// `else { body }`; returns the result of the body
  template <class F>
    requires std::invocable<F, Scope<D, Presence::Maybe> &>
  std::invoke_result_t<F, Scope<D, Presence::Maybe> &>
  else_( // NOLINT(readability-identifier-naming): `else` is a keyword
      F &&body, std::source_location site = std::source_location::current()) {
    detail::check_last(*state_, close_, site);
    detail::simple(*state_, block_, detail::Simple::Else, {}, site);
    return detail::enter<Scope<D, Presence::Maybe>>(
        state_, record_, std::forward<F>(body), site);
  }

  /// `else if:type`, a declaration of the name `if` as the else branch;
  /// `target` must be "if"
  template <class T, TypeLike Ty>
  detail::TargetOut<T, TypeValue<Ty>, detail::plus(D, type_dims<Ty>),
                    Presence::Maybe>
  else_read_if(const T &target, const Ty &type,
               std::source_location site = std::source_location::current()) {
    using Out =
        detail::TargetOut<T, TypeValue<Ty>, detail::plus(D, type_dims<Ty>),
                          Presence::Maybe>;
    return detail::make_handle<Out>(detail::else_declaration(
        *state_, block_, close_,
        detail::TargetTraits<detail::Bare<T>>::target(target),
        detail::value_tag<TypeValue<Ty>>(), detail::declared_of(type), site));
  }

private:
  friend struct detail::Access;
  template <class Result>
  If(std::shared_ptr<detail::State> state, std::size_t block,
     std::optional<std::size_t> record, std::size_t close, Result &&result)
      : state_(std::move(state)), block_(block), record_(record), close_(close),
        held_{std::forward<Result>(result)} {}
  If(std::shared_ptr<detail::State> state, std::size_t block,
     std::optional<std::size_t> record, std::size_t close)
      : state_(std::move(state)), block_(block), record_(record),
        close_(close) {}

  std::shared_ptr<detail::State> state_;
  std::size_t block_;
  std::optional<std::size_t> record_;
  std::size_t close_;
  [[no_unique_address]] detail::Held<R> held_;
};

/**
 * The scope of a block, at depth D (the number of enclosing repeats) and
 * presence P (whether the statements run in every record)
 *
 * The statements of a block go to its scope, which its closure receives.
 * A scope is a handle to its block: a statement written through a scope
 * whose block is not the innermost open one is a builder error. Every
 * statement takes a trailing `std::source_location`, so that errors name
 * the call.
 *
 * @tparam D the depth, or dynamic_depth
 * @tparam P Presence::Maybe inside a branch or an optional arm
 */
template <std::size_t D = 0, Presence P = Presence::Required> class Scope {
public:
  /// The scope of a statement nested in this one
  static constexpr std::size_t depth = D;
  /// Whether the statements of this scope run in every record
  static constexpr Presence presence = P;

  /// `target: type`, a declaration, as in `read("n", int_)` or
  /// `read("x", int_.dim(n))`. A name gives a typed handle, a handle
  /// redefines its name and gives a DynField, and discard gives nothing.
  template <class T, TypeLike Ty>
  detail::TargetOut<T, TypeValue<Ty>, detail::plus(D, type_dims<Ty>), P>
  read(const T &target, const Ty &type,
       std::source_location site = std::source_location::current()) {
    using Out =
        detail::TargetOut<T, TypeValue<Ty>, detail::plus(D, type_dims<Ty>), P>;
    return detail::make_handle<Out>(detail::declaration(
        *state_, block_, detail::TargetTraits<detail::Bare<T>>::target(target),
        detail::value_tag<TypeValue<Ty>>(), detail::declared_of(type), site));
  }

  /// `list target: type until end`, as in `list("x", int_.until(0))`
  template <class T, class V>
  detail::TargetOut<T, V, detail::plus(D, 1), P>
  list(const T &target, const ListReading<V> &reading,
       std::source_location site = std::source_location::current()) {
    using Out = detail::TargetOut<T, V, detail::plus(D, 1), P>;
    detail::Declared declared = detail::Access::declared(reading);
    detail::ListEnd end = detail::Access::end(reading);
    return detail::make_handle<Out>(detail::list(
        *state_, block_, detail::TargetTraits<detail::Bare<T>>::target(target),
        detail::value_tag<V>(), std::move(declared), std::move(end), site));
  }

  /// `let target = value`. A scalar expression keeps the depth of the
  /// scope; a lookup or a Dynamic value gives dynamic_depth.
  template <class T, Operand E>
  detail::TargetOut<T, ValueOf<E>, detail::let_depth<E, D>, P>
  let(const T &target, E &&value,
      std::source_location site = std::source_location::current()) {
    using Out = detail::TargetOut<T, ValueOf<E>, detail::let_depth<E, D>, P>;
    detail::Ops ops = detail::ops_of(std::forward<E>(value));
    const std::optional<std::size_t> levels = detail::let_levels<E, D>(ops);
    return detail::make_handle<Out>(detail::let(
        *state_, block_, detail::TargetTraits<detail::Bare<T>>::target(target),
        detail::value_tag<ValueOf<E>>(), levels, std::move(ops), site));
  }

  /// `let handle = value`, keeping the handle's type and depth: the value
  /// must have both
  template <class V, std::size_t H, Presence HP, Operand E>
    requires std::same_as<ValueOf<E>, V> &&
             (detail::same_depth(detail::depth_in<E, D>, H))
  void set(const Field<V, H, HP> &handle, E &&value,
           std::source_location site = std::source_location::current()) {
    detail::Ops ops = detail::ops_of(std::forward<E>(value));
    const std::optional<std::size_t> levels = detail::let_levels<E, D>(ops);
    detail::let(*state_, block_,
                detail::TargetTraits<Field<V, H, HP>>::target(handle),
                detail::value_tag<V>(), levels, std::move(ops), site);
  }

  /// `param target`, a parameter without a default
  template <class T>
  detail::TargetOut<T, Dynamic, dynamic_depth, P>
  param(const T &target,
        std::source_location site = std::source_location::current()) {
    using Out = detail::TargetOut<T, Dynamic, dynamic_depth, P>;
    return detail::make_handle<Out>(detail::param(
        *state_, block_, detail::TargetTraits<detail::Bare<T>>::target(target),
        detail::ValueTag::Dyn, std::nullopt, std::nullopt, site));
  }

  /// `param target = fallback`
  template <class T, Operand E>
  detail::TargetOut<T, ValueOf<E>, detail::let_depth<E, D>, P>
  param(const T &target, E &&fallback,
        std::source_location site = std::source_location::current()) {
    using Out = detail::TargetOut<T, ValueOf<E>, detail::let_depth<E, D>, P>;
    return detail::make_handle<Out>(detail::param(
        *state_, block_, detail::TargetTraits<detail::Bare<T>>::target(target),
        detail::value_tag<ValueOf<E>>(), detail::scalar_levels<E>,
        detail::ops_of(std::forward<E>(fallback)), site));
  }

  /// `check condition`
  template <Operand E>
  void check(E &&condition,
             std::source_location site = std::source_location::current()) {
    detail::check(*state_, block_, detail::ops_of(std::forward<E>(condition)),
                  std::nullopt, site);
  }

  /// `check condition "message"`
  template <Operand E>
  void check(E &&condition, std::string_view message,
             std::source_location site = std::source_location::current()) {
    detail::check(*state_, block_, detail::ops_of(std::forward<E>(condition)),
                  std::string(message), site);
  }

  /// `expect "word" ...`
  void expect(const Strings &words,
              std::source_location site = std::source_location::current()) {
    detail::simple(*state_, block_, detail::Simple::Expect, words.values(),
                   site);
  }

  /// `line "pattern"`
  void line(PatternArg text,
            std::source_location site = std::source_location::current()) {
    detail::pattern_statement(*state_, block_, false,
                              detail::Access::pieces(text), site);
  }

  /// `find "pattern"`
  void find(PatternArg text,
            std::source_location site = std::source_location::current()) {
    detail::pattern_statement(*state_, block_, true,
                              detail::Access::pieces(text), site);
  }

  /// `skip line`
  void skip_line(std::source_location site = std::source_location::current()) {
    detail::simple(*state_, block_, detail::Simple::SkipLine, {}, site);
  }

  /// `skip count lines`
  template <Operand E>
  void skip_lines(E &&lines,
                  std::source_location site = std::source_location::current()) {
    detail::computed(*state_, block_, detail::Computed::SkipLines,
                     detail::ops_of(std::forward<E>(lines)), site);
  }

  /// `index position`
  template <Operand E>
  void index(E &&position,
             std::source_location site = std::source_location::current()) {
    detail::computed(*state_, block_, detail::Computed::Index,
                     detail::ops_of(std::forward<E>(position)), site);
  }

  /// `type name = type`, a named type for declarations and captures
  template <TypeLike Ty>
    requires(type_dims<Ty> == 0)
  NamedType<TypeValue<Ty>>
  define_type(std::string_view name, const Ty &type,
              std::source_location site = std::source_location::current()) {
    return detail::Access::named<TypeValue<Ty>>(detail::define_type(
        *state_, block_, name, detail::declared_of(type), site));
  }

  /// `repeat count { body }`; returns the result of the body
  template <Operand C, class F>
    requires std::invocable<F, Scope<detail::plus(D, 1), P> &>
  std::invoke_result_t<F, Scope<detail::plus(D, 1), P> &>
  repeat(C &&times, F &&body,
         std::source_location site = std::source_location::current()) {
    const std::size_t record =
        detail::repeat(*state_, block_, detail::RepeatKind::Times,
                       detail::ops_of(std::forward<C>(times)), {}, {}, site);
    return detail::enter<Scope<detail::plus(D, 1), P>>(
        state_, record, std::forward<F>(body), site);
  }

  /// `repeat until eof { body }`; returns the result of the body
  template <class F>
    requires std::invocable<F, Scope<detail::plus(D, 1), P> &>
  std::invoke_result_t<F, Scope<detail::plus(D, 1), P> &> repeat_until_eof(
      F &&body, std::source_location site = std::source_location::current()) {
    const std::size_t record = detail::repeat(
        *state_, block_, detail::RepeatKind::UntilEof, {}, {}, {}, site);
    return detail::enter<Scope<detail::plus(D, 1), P>>(
        state_, record, std::forward<F>(body), site);
  }

  /// `repeat until "line" ... { body }`; returns the result of the body
  template <class F>
    requires std::invocable<F, Scope<detail::plus(D, 1), P> &>
  std::invoke_result_t<F, Scope<detail::plus(D, 1), P> &>
  repeat_until(const Strings &terminators, F &&body,
               std::source_location site = std::source_location::current()) {
    return repeat_until(terminators, UntilOptions{}, std::forward<F>(body),
                        site);
  }

  /// `repeat until "line" ... store target { body }`; returns the result of
  /// the body
  template <class F>
    requires std::invocable<F, Scope<detail::plus(D, 1), P> &>
  std::invoke_result_t<F, Scope<detail::plus(D, 1), P> &>
  repeat_until(const Strings &terminators, const UntilOptions &options,
               F &&body,
               std::source_location site = std::source_location::current()) {
    const std::size_t record = detail::repeat(
        *state_, block_, detail::RepeatKind::Until, {}, terminators.values(),
        detail::Access::target(options.store), site);
    return detail::enter<Scope<detail::plus(D, 1), P>>(
        state_, record, std::forward<F>(body), site);
  }

  /// `sections { arms }`; returns the result of the body, which receives
  /// the Arms
  template <class F>
    requires std::invocable<F, Arms<D, P> &>
  std::invoke_result_t<F, Arms<D, P> &>
  sections(F &&body,
           std::source_location site = std::source_location::current()) {
    return sections({}, std::forward<F>(body), site);
  }

  /// `sections until ... store ... skip other { arms }`
  template <class F>
    requires std::invocable<F, Arms<D, P> &>
  std::invoke_result_t<F, Arms<D, P> &>
  sections(const SectionsOptions &options, F &&body,
           std::source_location site = std::source_location::current()) {
    return sections({options}, std::forward<F>(body), site);
  }

  /// `sections` with several clauses in order, as in
  /// `{{.until = "END"}, {.until = "x"}}`
  template <class F>
    requires std::invocable<F, Arms<D, P> &>
  std::invoke_result_t<F, Arms<D, P> &>
  sections(std::initializer_list<SectionsOptions> clauses, F &&body,
           std::source_location site = std::source_location::current()) {
    std::vector<detail::Clause> list;
    for (const SectionsOptions &clause : clauses)
      list.push_back(detail::Clause{clause.until.values(),
                                    detail::Access::target(clause.store),
                                    clause.skip_other});
    detail::sections(*state_, block_, std::move(list), site);
    return detail::enter<Arms<D, P>>(state_, record_, std::forward<F>(body),
                                     site);
  }

  /// `if condition { body }`; else_if, else_ and else_read_if on the
  /// result continue it
  template <Operand C, class F>
    requires std::invocable<F, Scope<D, Presence::Maybe> &>
  If<D, P, std::invoke_result_t<F, Scope<D, Presence::Maybe> &>>
  if_( // NOLINT(readability-identifier-naming): `if` is a keyword
      C &&condition, F &&body,
      std::source_location site = std::source_location::current()) {
    using S = Scope<D, Presence::Maybe>;
    using R = std::invoke_result_t<F, S &>;
    detail::computed(*state_, block_, detail::Computed::If,
                     detail::ops_of(std::forward<C>(condition)), site);
    if constexpr (std::is_void_v<R>) {
      detail::enter<S>(state_, record_, std::forward<F>(body), site);
      return detail::Access::branch<If<D, P, R>>(
          state_, block_, record_, detail::statement_count(*state_) - 1,
          detail::Held<void>{});
    } else {
      decltype(auto) result =
          detail::enter<S>(state_, record_, std::forward<F>(body), site);
      return detail::Access::branch<If<D, P, R>>(
          state_, block_, record_, detail::statement_count(*state_) - 1,
          std::forward<R>(result));
    }
  }

  /// A capture for patterns, `{name:type}`, in the scope of this block
  template <CaptureType Ty>
  Field<CaptureValue<Ty>, D, P>
  capture(std::string_view name, const Ty &type,
          std::source_location site = std::source_location::current()) {
    return declare_capture<Ty, P>(name, type, site);
  }

  /// A capture for patterns whose entry may be undefined
  template <CaptureType Ty>
  Field<CaptureValue<Ty>, D, Presence::Maybe> optional_capture(
      std::string_view name, const Ty &type,
      std::source_location site = std::source_location::current()) {
    return declare_capture<Ty, Presence::Maybe>(name, type, site);
  }

  /// A handle of type H for a name that a later statement defines, as in
  /// `forward<Field<std::int64_t>>("n")`
  template <class H>
    requires detail::IsField<H>::value
  H forward(std::string_view name,
            std::source_location site = std::source_location::current()) {
    return detail::Access::field<typename H::Value, H::depth, H::presence>(
        detail::forward(*state_, name, detail::value_tag<typename H::Value>(),
                        detail::levels_of(H::depth),
                        H::presence == Presence::Required, site));
  }

  /// A handle for a name that nothing defines, for `defined(x)`
  DynField
  undefined(std::string_view name,
            std::source_location site = std::source_location::current()) {
    return detail::Access::field<Dynamic, dynamic_depth, Presence::Maybe>(
        detail::undefined(*state_, name, site));
  }

  /// A record handle for a repeat that comes later, with the names `each`
  /// and `list`; record_as in the repeat binds it
  Record
  forward_record(std::string_view each, std::string_view list,
                 std::source_location site = std::source_location::current()) {
    return detail::Access::record(
        detail::forward_record(*state_, each, list, site),
        detail::builder_id(*state_));
  }

  /// The records of the innermost repeat
  [[nodiscard]] Record record() const
    requires(D != 0)
  {
    return detail::Access::record(own_record(), detail::builder_id(*state_));
  }

  /// Name the current record of the innermost repeat, `each name`
  void each_name(std::string_view name,
                 std::source_location site = std::source_location::current())
    requires(D != 0)
  {
    detail::name_record(*state_, own_record(), true, name, site);
  }

  /// Name the list of the innermost repeat, `as name`
  void as_name(std::string_view name,
               std::source_location site = std::source_location::current())
    requires(D != 0)
  {
    detail::name_record(*state_, own_record(), false, name, site);
  }

  /// Bind a record from forward_record to the innermost repeat
  void record_as(const Record &record,
                 std::source_location site = std::source_location::current())
    requires(D != 0)
  {
    detail::record_as(*state_, own_record(), detail::Access::record_id(record),
                      detail::Access::record_owner(record), site);
  }

private:
  friend struct detail::Access;
  friend class SpecBuilder;

  /// The scope of the block `block` of `state`
  Scope(std::shared_ptr<detail::State> state, std::size_t block,
        std::optional<std::size_t> record)
      : state_(std::move(state)), block_(block), record_(record) {}

  /// The builder's state, which its scopes share
  std::shared_ptr<detail::State> state_;
  /// The block of this scope
  std::size_t block_;
  /// The records of the innermost repeat
  std::optional<std::size_t> record_;

  /// The records of the innermost repeat, which a scope of depth D > 0
  /// always has
  [[nodiscard]] std::size_t own_record() const { return record_.value_or(0); }

  template <class Ty, Presence Q>
  Field<CaptureValue<Ty>, D, Q> declare_capture(std::string_view name,
                                                const Ty &type,
                                                std::source_location site) {
    return detail::Access::field<CaptureValue<Ty>, D, Q>(
        detail::capture(*state_, name, detail::value_tag<CaptureValue<Ty>>(),
                        detail::capture_declared<std::remove_cvref_t<Ty>>(type),
                        detail::levels_of(D), Q == Presence::Required, site));
  }
};

/**
 * The arms of a `sections` block: each arm is a pattern that starts a
 * section, with or without a body
 */
template <std::size_t D = 0, Presence P = Presence::Required> class Arms {
public:
  /// An arm without a body
  void arm(PatternArg text, ArmOptions options = {},
           std::source_location site = std::source_location::current()) {
    add(text, false, options, false, site);
  }
  /// An arm with a body; returns the result of the body
  template <class F>
    requires std::invocable<F, Scope<D, P> &>
  std::invoke_result_t<F, Scope<D, P> &>
  arm(PatternArg text, F &&body,
      std::source_location site = std::source_location::current()) {
    return arm(text, ArmOptions{}, std::forward<F>(body), site);
  }
  /// An arm with options and a body; returns the result of the body
  template <class F>
    requires std::invocable<F, Scope<D, P> &>
  std::invoke_result_t<F, Scope<D, P> &>
  arm(PatternArg text, ArmOptions options, F &&body,
      std::source_location site = std::source_location::current()) {
    add(text, false, options, true, site);
    return detail::enter<Scope<D, P>>(state_, record_, std::forward<F>(body),
                                      site);
  }
  /// An `optional` arm without a body
  void
  optional_arm(PatternArg text, ArmOptions options = {},
               std::source_location site = std::source_location::current()) {
    add(text, true, options, false, site);
  }
  /// An `optional` arm with a body, whose statements may not run; returns
  /// the result of the body
  template <class F>
    requires std::invocable<F, Scope<D, Presence::Maybe> &>
  std::invoke_result_t<F, Scope<D, Presence::Maybe> &>
  optional_arm(PatternArg text, F &&body,
               std::source_location site = std::source_location::current()) {
    return optional_arm(text, ArmOptions{}, std::forward<F>(body), site);
  }
  /// An `optional` arm with options and a body; returns the result of the
  /// body
  template <class F>
    requires std::invocable<F, Scope<D, Presence::Maybe> &>
  std::invoke_result_t<F, Scope<D, Presence::Maybe> &>
  optional_arm(PatternArg text, ArmOptions options, F &&body,
               std::source_location site = std::source_location::current()) {
    add(text, true, options, true, site);
    return detail::enter<Scope<D, Presence::Maybe>>(
        state_, record_, std::forward<F>(body), site);
  }

private:
  friend struct detail::Access;
  Arms(std::shared_ptr<detail::State> state, std::size_t block,
       std::optional<std::size_t> record)
      : state_(std::move(state)), block_(block), record_(record) {}

  void add(const PatternArg &text, bool optional, ArmOptions options, bool body,
           std::source_location site) {
    detail::arm(*state_, block_, detail::Access::pieces(text), optional,
                options.last, body, site);
  }

  std::shared_ptr<detail::State> state_;
  std::size_t block_;
  std::optional<std::size_t> record_;
};

/**
 * A reader spec under construction: the scope of the top level, with the
 * settings that only the top level has
 *
 * build() prints the statements and parses the text; source() returns the
 * text. Both report builder checks first.
 */
class SpecBuilder : public Scope<0, Presence::Required> {
public:
  /// A builder without a `format` statement
  SpecBuilder();
  /// A builder that starts with `format name`
  explicit SpecBuilder(
      std::string_view format,
      std::source_location site = std::source_location::current());

  /// `format name`
  void format(std::string_view name,
              std::source_location site = std::source_location::current());
  /// `comment "marker" ...`
  void comment(const Strings &markers,
               std::source_location site = std::source_location::current());
  /// `stop "marker" ...`
  void stop(const Strings &markers,
            std::source_location site = std::source_location::current());
  /// `tokens "characters"`
  void tokens(std::string_view characters,
              std::source_location site = std::source_location::current());
  /// `separators "characters"`
  void separators(std::string_view characters,
                  std::source_location site = std::source_location::current());
  /// `ignore trailing`
  void
  ignore_trailing(std::source_location site = std::source_location::current());

  /// `instances { body }`; returns the result of the body
  template <class F>
    requires std::invocable<F, Scope<0, Presence::Required> &>
  std::invoke_result_t<F, Scope<0, Presence::Required> &>
  instances(F &&body,
            std::source_location site = std::source_location::current()) {
    detail::simple(*state_, block_, detail::Simple::Instances, {}, site);
    return detail::enter<Scope<0, Presence::Required>>(
        state_, std::nullopt, std::forward<F>(body), site);
  }

  /// Return the text of the spec. Throws BuildError for a builder check.
  [[nodiscard]] std::string source() const;
  /// Return the text of the spec, or the BuildError of a builder check
  [[nodiscard]] std::expected<std::string, BuildError> try_source() const;
  /// Print and parse the spec. Throws BuildError for a builder check or a
  /// language error.
  [[nodiscard]] Spec build() const;
  /// Print and parse the spec, or return the BuildError
  [[nodiscard]] std::expected<Spec, BuildError> try_build() const;
};

/*
 * Definitions of the members declared above
 */

template <class V, class S>
template <Operand I>
  requires(std::same_as<S, Open> || std::same_as<V, Dynamic>) &&
          Integral<ValueOf<I>>
Expr<V, Open> Expr<V, S>::operator[](I &&index) const & {
  return Expr<V, Open>(detail::index_ops(ops_, std::forward<I>(index)));
}

template <class V, class S>
template <Operand I>
  requires(std::same_as<S, Open> || std::same_as<V, Dynamic>) &&
          Integral<ValueOf<I>>
Expr<V, Open> Expr<V, S>::operator[](I &&index) && {
  return Expr<V, Open>(
      detail::index_ops(std::move(ops_), std::forward<I>(index)));
}

template <class V, class S>
template <class F, std::size_t H, Presence P>
  requires std::same_as<S, Open> || std::same_as<V, Dynamic>
Expr<F, Open> Expr<V, S>::get(const Field<F, H, P> &field) const & {
  return Expr<F, Open>(detail::member_ops(ops_, field));
}

template <class V, class S>
template <class F, std::size_t H, Presence P>
  requires std::same_as<S, Open> || std::same_as<V, Dynamic>
Expr<F, Open> Expr<V, S>::get(const Field<F, H, P> &field) && {
  return Expr<F, Open>(detail::member_ops(std::move(ops_), field));
}

inline Expr<Dynamic, Scalar> Host::dynamic() const {
  return detail::Access::expr<Dynamic, Scalar>(detail::ops_of(*this));
}

template <class V, std::size_t D, Presence P>
Expr<V, Open> Record::get(const Field<V, D, P> &field) const {
  return detail::Access::expr<V, Open>(detail::member_ops(
      {detail::record_op(detail::OpKind::Record, *this)}, field));
}

inline RecordList Record::records(const Record &inner) const {
  return detail::Access::record_list(
      {detail::record_op(detail::OpKind::Record, *this),
       detail::record_op(detail::OpKind::ListMember, inner)});
}

inline RecordList Record::list() const {
  return detail::Access::record_list(
      {detail::record_op(detail::OpKind::RecordList, *this)});
}

template <Operand I>
  requires Integral<ValueOf<I>>
RecordExpr Record::operator[](I &&index) const {
  return list()[std::forward<I>(index)];
}

template <class V, std::size_t D, Presence P>
Expr<V, Open> RecordExpr::get(const Field<V, D, P> &field) const {
  return detail::Access::expr<V, Open>(detail::member_ops(ops_, field));
}

inline RecordList RecordExpr::records(const Record &inner) const {
  detail::Ops ops = ops_;
  ops.push_back(detail::record_op(detail::OpKind::ListMember, inner));
  return detail::Access::record_list(std::move(ops));
}

template <Operand I>
  requires Integral<ValueOf<I>>
RecordExpr RecordExpr::operator[](I &&index) const {
  return RecordExpr(detail::index_ops(ops_, std::forward<I>(index)));
}

template <Operand I>
  requires Integral<ValueOf<I>>
RecordExpr RecordList::operator[](I &&index) const {
  return detail::Access::record_expr(
      detail::index_ops(ops_, std::forward<I>(index)));
}

template <class V, std::size_t N>
template <Operand E>
TypeOf<V, detail::plus(N, 1)> TypeOf<V, N>::dim(E &&extent) const {
  detail::Declared declared = declared_;
  declared.dimensions.push_back(detail::ops_of(std::forward<E>(extent)));
  return TypeOf<V, detail::plus(N, 1)>(std::move(declared));
}

template <class V, std::size_t N>
template <std::ranges::input_range R>
  requires Operand<std::ranges::range_reference_t<R>>
TypeOf<V, dynamic_depth> TypeOf<V, N>::dims(R &&sizes) const {
  detail::Declared declared = declared_;
  for (auto &&extent : std::forward<R>(sizes))
    declared.dimensions.push_back(
        detail::ops_of(std::forward<decltype(extent)>(extent)));
  return TypeOf<V, dynamic_depth>(std::move(declared));
}

template <class V, std::size_t N>
template <Operand E>
TypeOf<V, N> TypeOf<V, N>::layout(E &&value) const {
  detail::Declared declared = declared_;
  declared.layout = detail::ops_of(std::forward<E>(value));
  return TypeOf(std::move(declared));
}

template <class V, std::size_t N>
template <Operand E>
TypeOf<V, N> TypeOf<V, N>::offset(E &&value) const {
  detail::Declared declared = declared_;
  declared.offset = detail::ops_of(std::forward<E>(value));
  return TypeOf(std::move(declared));
}

template <class V, std::size_t N>
template <class E>
  requires(N == 0) && (std::same_as<std::remove_cvref_t<E>, Eol> || Operand<E>)
ListReading<V> TypeOf<V, N>::until(E &&end) const {
  return detail::Access::list<V>(declared_,
                                 detail::list_end(std::forward<E>(end)));
}

template <class V> TypeOf<V, 0> BaseType<V>::ty() const {
  return detail::Access::type<V, 0>(
      detail::TypeTraits<BaseType<V>>::declared(*this));
}

template <Operand E>
  requires BoundValue<ValueOf<E>>
Bound::Bound(E &&value) : ops_(detail::ops_of(std::forward<E>(value))) {}

template <Operand E> TypeOf<std::int64_t, 1> Range::dim(E &&extent) const {
  return ty(*this).dim(std::forward<E>(extent));
}

template <Operand E> TypeOf<std::int64_t, 0> Range::layout(E &&value) const {
  return ty(*this).layout(std::forward<E>(value));
}

template <Operand E> TypeOf<std::int64_t, 0> Range::offset(E &&value) const {
  return ty(*this).offset(std::forward<E>(value));
}

template <class E>
  requires std::same_as<std::remove_cvref_t<E>, Eol> || Operand<E>
ListReading<std::int64_t> Range::until(E &&end) const {
  return ty(*this).until(std::forward<E>(end));
}

template <Operand E> TypeOf<std::int64_t, 1> Alphabet::dim(E &&extent) const {
  return ty(*this).dim(std::forward<E>(extent));
}

template <class E>
  requires std::same_as<std::remove_cvref_t<E>, Eol> || Operand<E>
ListReading<std::int64_t> Alphabet::until(E &&end) const {
  return ty(*this).until(std::forward<E>(end));
}

template <class V>
template <Operand E>
ListReading<V> ListReading<V>::offset(E &&value) const {
  detail::Declared declared = declared_;
  declared.offset = detail::ops_of(std::forward<E>(value));
  return ListReading(std::move(declared), end_);
}

template <PatternPart T> Pattern &Pattern::part(const T &part) & {
  pieces_.push_back(detail::piece_of(part));
  return *this;
}

template <PatternPart T> Pattern &&Pattern::part(const T &part) && {
  pieces_.push_back(detail::piece_of(part));
  return std::move(*this);
}

template <class V, std::size_t D, Presence P>
StoreTarget::StoreTarget(const Field<V, D, P> &handle)
    : target_{detail::Target::Kind::Existing, std::string(handle.name()),
              detail::Access::owner(handle)} {}

} // namespace iris::build

namespace iris {
// The operators must be found for an iris::Field too, whose associated
// namespace is iris.
using build::operator+;
using build::operator-;
using build::operator*;
using build::operator/;
using build::operator%;
using build::operator&&;
using build::operator||;
using build::operator==;
using build::operator!=;
using build::operator<;
using build::operator<=;
using build::operator>;
using build::operator>=;
using build::operator!;
} // namespace iris
