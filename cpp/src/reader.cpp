/* -*- mode: C++; c-basic-offset: 2; indent-tabs-mode: nil -*- */
/*
 *  Main authors:
 *     Mikael Zayenz Lagerkvist <lagerkvist@gecode.dev>
 *
 *  Copyright:
 *     Mikael Zayenz Lagerkvist, 2026
 *
 *  This file is part of Gecode, the generic constraint
 *  development environment:
 *     http://www.gecode.dev
 *
 *  Permission is hereby granted, free of charge, to any person obtaining
 *  a copy of this software and associated documentation files (the
 *  "Software"), to deal in the Software without restriction, including
 *  without limitation the rights to use, copy, modify, merge, publish,
 *  distribute, sublicense, and/or sell copies of the Software, and to
 *  permit persons to whom the Software is furnished to do so, subject to
 *  the following conditions:
 *
 *  The above copyright notice and this permission notice shall be
 *  included in all copies or substantial portions of the Software.
 *
 *  THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
 *  EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
 *  MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
 *  NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
 *  LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
 *  OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
 *  WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
 *
 */

// Ported from gecode/instance/reader.cpp at Gecode commit
// 5e7bbd5c26b0702c6fd652356b2289da0710ac3c.

#include "reader.hpp"

#include "eval.hpp"
#include "numbers.hpp"
#include "text.hpp"

#include <algorithm>
#include <climits>
#include <cstdint>
#include <optional>
#include <utility>

namespace iris::detail {

namespace {

/**
 * Return the first column of row `i` of a stored triangle
 *
 * The triangle is upper if `upper` and includes the diagonal if `diag`.
 */
std::size_t triangle_from(bool upper, bool diag, std::size_t i) {
  if (!upper)
    return 0;
  return diag ? i : i + 1;
}
/// Return the column after the last of row `i` of a stored triangle of an
/// `n` by `n` matrix (see triangle_from)
std::size_t triangle_to(bool upper, bool diag, std::size_t n, std::size_t i) {
  if (upper)
    return n;
  return diag ? i + 1 : i;
}

/// Return a description of what read type `t` reads, for messages
std::string describe(ReadType t) {
  switch (t) {
  case ReadType::Int:
    return "int";
  case ReadType::Float:
    return "float";
  case ReadType::Num:
    return "a number";
  case ReadType::Word:
    return "a word";
  case ReadType::Rest:
    return "the rest of the line";
  case ReadType::Symbol:
    return "a symbol";
  }
  std::unreachable();
}

/// Test whether values of read type `t` are strings
bool reads_strings(ReadType t) {
  return (t == ReadType::Word) || (t == ReadType::Rest);
}

/// The bounds of a range type, evaluated
struct Range {
  /// Lower bound, if any
  std::optional<std::int64_t> lo;
  /// Upper bound, included, if any
  std::optional<std::int64_t> hi;
  /// Test whether `v` is in the range
  [[nodiscard]] bool contains(std::int64_t v) const {
    return (!lo || (v >= *lo)) && (!hi || (v <= *hi));
  }
  /// Test whether there are bounds
  [[nodiscard]] bool bounded() const { return lo || hi; }
};

/// A type with its range evaluated
struct Domain {
  /// The type
  const Type &type;
  /// The range
  Range range;
  /// Return a description of the values, for messages
  [[nodiscard]] std::string describe() const {
    std::string d = detail::describe(type.read);
    if (range.bounded())
      d = "an int in " + (range.lo ? std::to_string(*range.lo) : "") + ".." +
          (range.hi ? std::to_string(*range.hi) : "");
    return type.name.empty() ? d : type.name + " (" + d + ")";
  }
};

/// A value captured from a line
struct Capture {
  /// The capture
  const Pattern::Item *item;
  /// The value
  Value value;
  /// Where its text starts in the line
  std::size_t from;
  /// Where its text ends in the line
  std::size_t to;
};

/// Interpreter running a reader spec on an input
class Interpreter : public Evaluator {
public:
  /// Initialize
  Interpreter(const SpecImpl &s, std::string_view text, std::string origin,
              const ParamsImpl &p)
      : Evaluator(s, text, std::move(origin)), params_(p) {}

  /// Add the names of the param statements in `b` to `out`
  static void declared_params(const Block &b, Seq<std::string> &out) {
    for (const std::unique_ptr<Stmt> &s : b) {
      if (s->kind == Stmt::Kind::Param)
        out.push_back(s->name);
      if (s->kind == Stmt::Kind::Instances)
        out.emplace_back("instance");
      declared_params(s->body, out);
      declared_params(s->orelse, out);
      for (const Arm &a : s->arms)
        declared_params(a.body, out);
    }
  }

  /// Run the reader spec and return the data
  DataImpl run() {
    // A misspelt parameter would otherwise be ignored silently
    Seq<std::string> known;
    declared_params(spec.body, known);
    for (const std::string &n : params_.names)
      if (std::ranges::find(known, n) == known.end())
        throw spec_error(ErrorCode::ParamUnknown,
                         "the spec has no parameter '" + n + "'", spec.origin);
    // The whole input must be valid UTF-8 (Specification §4.10)
    if (std::optional<std::size_t> bad = find_invalid_utf8(in.text)) {
      in.advance(*bad);
      throw input_error(ErrorCode::Utf8, "input is not valid UTF-8",
                        Location{.origin = in.origin,
                                 .line = in.line,
                                 .column = in.here().column()});
    }
    exec(spec.body);
    spec_line = 0;
    // Reading stops after the selected instance
    if (!trailing_ && !selected_ && in.skip_to_data())
      throw input_error(ErrorCode::TrailingInput,
                        "unexpected input after the end of the spec: " +
                            quote(in.rest()),
                        Location{.origin = in.origin,
                                 .line = in.line,
                                 .column = in.here().column()});
    DataImpl d;
    for (Entry &e : frames[0])
      if (!is_internal(e.name))
        d.entries.push_back(std::move(e));
    return d;
  }

private:
  /// The parameters
  const ParamsImpl &params_;
  /// Row numbers of the active repeats
  Seq<std::size_t> rows_;
  /// Whether input after the reader spec is allowed
  bool trailing_ = false;
  /// Whether the parameter instance selects an instance
  bool select_ = false;
  /// The value of the parameter instance
  std::string want_;
  /// The number of the instance selected, unless selected by name
  std::optional<std::int64_t> number_;
  /// Number of the instance being read (0 if none)
  std::size_t current_ = 0;
  /// Frame of the instance being read
  std::size_t instance_frame_ = 0;
  /// The first rejection of the instance being read
  std::optional<Error> deferred_;
  /// Whether the selected instance has been read
  bool selected_ = false;
  /// The ranges of the named types, by type statement
  Seq<std::pair<const Stmt *, Range>> type_ranges_;
  /// Where the last token read as a value starts
  std::size_t token_start_ = 0;
  /// Lists of finished repeat iterations, kept for their memory
  Seq<Value> spare_;
  /// Most lists kept in spare
  static constexpr std::size_t max_spare = 8;

  /// Account for `k` units of work that read no input (Specification §10.2)
  void work(std::uint64_t k) { budget.charge_idle(k); }

  /*
   * Reading
   */
  /// Store `t` read as `type` in `v`, and return whether it fits
  static bool convert(ReadType type, std::string_view t, Value &v) {
    if (reads_strings(type)) {
      v = Value::string(t);
      return !t.empty();
    }
    if (type != ReadType::Float)
      if (std::optional<std::int64_t> i = parse_int(t)) {
        v = Value::integer(*i);
        return true;
      }
    if (type != ReadType::Int)
      if (std::optional<double> f = parse_float(t)) {
        v = Value::real(*f);
        return true;
      }
    return false;
  }

  /// Fail unless the string `t` read at position `from` of the input is
  /// valid UTF-8 (Specification §4.10)
  void valid(std::string_view t, std::size_t from) {
    if (std::optional<std::size_t> bad = find_invalid_utf8(t)) {
      // The value is on the current line
      in.pos = from + *bad;
      fail(ErrorCode::Utf8, "string value is not valid UTF-8");
    }
  }

  /// Fail for want of `what` at the end of the input or the token there
  [[noreturn]] void expected(const std::string &what) {
    if (!in.skip_to_data())
      fail(ErrorCode::ExpectedValue,
           "expected " + what + ", found end of input");
    fail(ErrorCode::ExpectedValue,
         "expected " + what + ", found " + quote(in.peek()));
  }
  /**
   * Read one value of `type` into `v`, and return whether there is one
   *
   * At the end of the input or a token that is not of the type, the
   * position stays before it.
   */
  bool scan(ReadType type, Value &v) {
    if (!in.skip_to_data())
      return false;
    token_start_ = in.pos;
    if (type == ReadType::Rest) {
      std::string_view t = in.rest();
      valid(t, token_start_);
      v = Value::string(t);
      in.to_eol();
      return true;
    }
    std::string_view t = in.token();
    if (convert(type, t, v)) {
      if (type == ReadType::Word)
        valid(t, token_start_);
      return true;
    }
    in.pos = token_start_;
    return false;
  }
  /// Read one value of `type`, described by `what`
  Value scalar(ReadType type, const std::string &what) {
    Value v(ValueType::Int);
    if (!scan(type, v))
      expected(what);
    return v;
  }
  /// Read one value of domain `d`, without checking its range
  Value scalar(const Domain &d) {
    Value v(ValueType::Int);
    if (!scan(d.type.read, v))
      expected(d.describe());
    return v;
  }
  /// Read an integer of domain `d`, which reads integers, without checking
  /// its range
  std::int64_t int_scalar(const Domain &d) {
    std::int64_t x = 0;
    if (!in.skip_to_data())
      expected(d.describe());
    token_start_ = in.pos;
    if (!in.int_token(x))
      expected(d.describe());
    return x;
  }
  /// Test whether value `v` is in the range of domain `d`
  static bool fits(const Domain &d, const Value &v) {
    return (v.type != ValueType::Int) || d.range.contains(v.ints[0]);
  }
  /// Fail for the value from position `from` to `to` on the current line,
  /// which is not in domain `d`
  [[noreturn]] void outside(const Domain &d, std::size_t from, std::size_t to) {
    std::string_view t = in.text.substr(from, to - from);
    in.pos = from;
    fail(ErrorCode::ExpectedValue,
         "expected " + d.describe() + ", found " + quote(t));
  }
  /// Read one value of domain `d`
  Value value(const Domain &d) {
    Value v = scalar(d);
    if (!fits(d, v))
      outside(d, token_start_, in.pos);
    return v;
  }
  /// Read one value of domain `d` and append it to the list `v`
  void push(const Domain &d, Value &v) {
    if ((d.type.read == ReadType::Int) && (v.type == ValueType::Int)) {
      std::int64_t x = int_scalar(d);
      if (!d.range.contains(x))
        outside(d, token_start_, in.pos);
      v.ints.push_back(x);
    } else {
      v.push(value(d));
    }
  }

  /// Return the range of type `t`
  Range range(const Type &t) {
    if (t.named != nullptr) {
      for (const auto &[s, r] : type_ranges_)
        if (s == t.named)
          return r;
      fail(ErrorCode::TypeUndefined,
           "the type '" + t.name + "' is not defined yet");
    }
    Range r;
    if (t.lo)
      r.lo = bound(*t.lo);
    if (t.hi) {
      r.hi = bound(*t.hi);
      if (t.exclusive) {
        r.hi = checked_sub(*r.hi, 1);
        if (!r.hi)
          fail(ErrorCode::IntegerOverflow,
               "integer overflow in the upper bound of a range");
      }
    }
    return r;
  }
  /// Return the value of the bound `e` of a range
  std::int64_t bound(const Expr &e) {
    // Ranges are evaluated for every value read, so common bounds such as 0
    // and -n avoid making a value
    if (e.kind == Expr::Kind::Int)
      return e.int_value;
    if ((e.kind == Expr::Kind::Unary) && (e.op == Expr::Op::Neg)) {
      std::optional<std::int64_t> x = checked_neg(bound(*e.args[0]));
      if (!x)
        fail(ErrorCode::IntegerOverflow, "integer overflow in '-'");
      return *x;
    }
    Value tmp(ValueType::Int);
    const Value &v = ref(e, tmp);
    if ((v.depth() > 0) || (v.type != ValueType::Int))
      fail(ErrorCode::RangeBounds, "the bounds of a range must be integers");
    return v.ints[0];
  }

  /// Read an array of domain `d` with dimensions `dims`
  Value array(const Domain &d, const Seq<std::size_t> &dims) {
    // Each factor is at most INT_MAX, so the product cannot overflow
    std::uint64_t size = 1;
    for (std::size_t n : dims) {
      size *= n;
      if (size > INT_MAX)
        fail(ErrorCode::SizeLimit, "array is too large");
    }
    // Rows before an empty dimension cost memory without reading any input
    if (size == 0) {
      std::uint64_t rows = 1;
      for (std::size_t k = 0; (k < dims.size()) && (dims[k] > 0); k++) {
        rows *= dims[k];
        work(rows);
      }
    }
    Value v;
    if (d.type.read == ReadType::Float)
      v.type = ValueType::Float;
    else if (d.type.read == ReadType::Word)
      v.type = ValueType::String;
    // Read the values before making the structure, so that a wrong count
    // fails at the end of the input rather than on allocation
    for (std::uint64_t i = 0; i < size; i++)
      try {
        push(d, v);
      } catch (const ShapeError &e) {
        fail(e, context.back().text);
      }
    v.shape(dims);
    return v;
  }

  /**
   * Read a symmetric `n` by `n` matrix of domain `d` stored in TSPLIB layout
   * `f` with offset `o`
   *
   * The offset applies to the numbers read, not to the cells that only
   * exist after mirroring.
   */
  Value layout(const Domain &d, std::size_t n, std::size_t m,
               const std::string &f, std::int64_t o) {
    if (f == "FULL_MATRIX") {
      Value v = array(d, {n, m});
      offset(v, o);
      return v;
    }
    if (n != m)
      fail(ErrorCode::Layout, "layout " + f + " needs a square matrix");
    // Column-wise upper equals row-wise lower for symmetric matrices
    bool upper = false;
    bool diag = false;
    if ((f == "UPPER_ROW") || (f == "LOWER_COL")) {
      upper = true;
    } else if ((f == "LOWER_ROW") || (f == "UPPER_COL")) {
    } else if ((f == "UPPER_DIAG_ROW") || (f == "LOWER_DIAG_COL")) {
      upper = true;
      diag = true;
    } else if ((f == "LOWER_DIAG_ROW") || (f == "UPPER_DIAG_COL")) {
      diag = true;
    } else {
      fail(ErrorCode::Layout, "unknown matrix layout " + quote(f));
    }
    if (static_cast<std::uint64_t>(n) * n > INT_MAX)
      fail(ErrorCode::SizeLimit, "matrix is too large");
    // Read the stored triangle, then mirror it into a full matrix
    Value t;
    if (d.type.read == ReadType::Float)
      t.type = ValueType::Float;
    for (std::size_t i = 0; i < n; i++)
      for (std::size_t j = triangle_from(upper, diag, i);
           j < triangle_to(upper, diag, n, i); j++)
        push(d, t);
    offset(t, o);
    Value v;
    v.type = t.type;
    v.shape({n, n});
    if (v.type == ValueType::Int)
      v.ints.resize(n * n);
    else
      v.floats.resize(n * n);
    std::size_t k = 0;
    for (std::size_t i = 0; i < n; i++)
      for (std::size_t j = triangle_from(upper, diag, i);
           j < triangle_to(upper, diag, n, i); j++, k++)
        if (v.type == ValueType::Int) {
          v.ints[(i * n) + j] = t.ints[k];
          v.ints[(j * n) + i] = t.ints[k];
        } else {
          v.floats[(i * n) + j] = t.floats[k];
          v.floats[(j * n) + i] = t.floats[k];
        }
    return v;
  }

  /// Add `o` to every number in `v`
  void offset(Value &v, std::int64_t o) {
    for (std::int64_t &x : v.ints) {
      std::optional<std::int64_t> r = checked_add(x, o);
      if (!r)
        fail(ErrorCode::IntegerOverflow,
             "integer overflow when applying offset");
      x = *r;
    }
    for (double &x : v.floats)
      x += static_cast<double>(o);
  }

  /// Return the offset of statement `s` (0 if none)
  std::int64_t offset(const Stmt &s) {
    if (!s.offset)
      return 0;
    Value o = eval(*s.offset);
    if ((o.depth() > 0) || (o.type != ValueType::Int))
      fail(ErrorCode::OffsetType, "offset must be an integer");
    return o.ints[0];
  }

  /// Store value `v` read by statement `s`, unless its name is _
  void store(const Stmt &s, Value &&v) {
    if (s.name != "_")
      set(s.name, std::move(v));
  }

  /// Match pattern `p` against the rest of the current line, consuming it
  bool match(const Pattern &p) {
    Seq<Capture> caps;
    if (!matches(p, caps))
      return false;
    take(caps);
    return true;
  }

  /// Test whether `p` matches the rest of the current line, giving the
  /// captures `caps`
  bool matches(const Pattern &p, Seq<Capture> &caps) {
    in.skip();
    if (in.pos >= in.text.size())
      return false;
    std::string_view l = in.rest();
    std::size_t c = 0;
    for (std::size_t k = 0; k < p.items.size(); k++)
      if (!match_item(p, k, l, c, caps))
        return false;
    while ((c < l.size()) && is_blank(l[c]))
      c++;
    return c == l.size();
  }

  /// Define the values `caps` captured from the current line, and consume
  /// it
  void take(const Seq<Capture> &caps) {
    // Every string captured must be valid, even if it is not kept
    for (const Capture &x : caps)
      if (reads_strings(x.item->type.read))
        valid(x.value.strings[0], in.pos + x.from);
    // The captures can use each other in their ranges
    for (const Capture &x : caps)
      if (!x.item->text.empty() && (x.item->text != "_"))
        set(x.item->text, Value(x.value));
    for (const Capture &x : caps)
      if (x.item->type.ranged()) {
        Domain d{.type = x.item->type, .range = range(x.item->type)};
        if (!fits(d, x.value))
          outside(d, in.pos + x.from, in.pos + x.to);
      }
    in.to_eol();
  }

  /// Match item `k` of `p` at position `c` of line `l`
  bool match_item(const Pattern &p, std::size_t k, std::string_view l,
                  std::size_t &c, Seq<Capture> &caps) {
    using Item = Pattern::Item;
    const Item &it = p.items[k];
    switch (it.kind) {
    case Item::Kind::Space: {
      std::size_t c0 = c;
      while ((c < l.size()) && is_blank(l[c]))
        c++;
      return !it.required || (c > c0) || (c == l.size());
    }
    case Item::Kind::Lit:
      if (!l.substr(c).starts_with(it.text))
        return false;
      c += it.text.size();
      return true;
    case Item::Kind::Any:
      c = l.size();
      return true;
    case Item::Kind::Var: {
      std::string want = variable(it.text);
      if (want.empty() || !l.substr(c).starts_with(want))
        return false;
      c += want.size();
      return true;
    }
    case Item::Kind::Cap: {
      // A word ends before a literal that follows it directly
      char stop = 0;
      if ((k + 1 < p.items.size()) && (p.items[k + 1].kind == Item::Kind::Lit))
        stop = p.items[k + 1].text[0];
      std::size_t c0 = c;
      c = capture_end(it.type.read, l, c, stop);
      std::string_view t = l.substr(c0, c - c0);
      while (!t.empty() && is_blank(t.back()))
        t.remove_suffix(1);
      Capture x{.item = &it,
                .value = Value(ValueType::Int),
                .from = c0,
                .to = c0 + t.size()};
      if (!convert(it.type.read, t, x.value))
        return false;
      caps.push_back(std::move(x));
      return true;
    }
    }
    return false;
  }

  /// Return the value of variable `n` as text, for {$n} in a pattern
  std::string variable(const std::string &n) {
    const Value *v = lookup(n);
    if (v == nullptr)
      fail(ErrorCode::NameUndefined, "'" + n + "' is not defined yet");
    if ((v->depth() == 0) && (v->type == ValueType::String))
      return v->strings[0];
    if ((v->depth() == 0) && (v->type == ValueType::Int))
      return std::to_string(v->ints[0]);
    fail(ErrorCode::PatternReference,
         "'" + n + "' must be a string or an integer");
  }

  /// Return where a capture of `type` starting at `c` of `l` ends
  static std::size_t capture_end(ReadType type, std::string_view l,
                                 std::size_t c, char stop) {
    switch (type) {
    case ReadType::Rest:
      return l.size();
    case ReadType::Word:
      while ((c < l.size()) && !is_blank(l[c]) && (l[c] != stop))
        c++;
      return c;
    default:
      // Numbers end where a following literal starts
      return number_end(l, c, true, type != ReadType::Int);
    }
  }

  void exec(const Block &b) {
    for (const std::unique_ptr<Stmt> &s : b)
      exec(*s);
  }

  void exec(const Stmt &s) {
    spec_line = s.line;
    switch (s.kind) {
    case Stmt::Kind::Comment:
      in.comments.insert(in.comments.end(), s.words.begin(), s.words.end());
      break;
    case Stmt::Kind::Stop:
      in.stops.insert(in.stops.end(), s.words.begin(), s.words.end());
      break;
    case Stmt::Kind::Tokens:
      in.add_split(s.chars);
      break;
    case Stmt::Kind::Separators:
      in.add_separators(s.chars);
      break;
    case Stmt::Kind::Trailing:
      trailing_ = true;
      break;
    case Stmt::Kind::Param:
      param(s);
      break;
    case Stmt::Kind::Read:
      read(s);
      break;
    case Stmt::Kind::List:
      list(s);
      break;
    case Stmt::Kind::Line:
      line(s);
      break;
    case Stmt::Kind::Find:
      find(s);
      break;
    case Stmt::Kind::Skip:
      skip_lines(s);
      break;
    case Stmt::Kind::Expect:
      expect(s);
      break;
    case Stmt::Kind::Index:
      index(s);
      break;
    case Stmt::Kind::Repeat:
      repeat(s);
      break;
    case Stmt::Kind::Sections:
      sections(s);
      break;
    case Stmt::Kind::Instances:
      instances(s);
      break;
    case Stmt::Kind::Type:
      define_type(s);
      break;
    case Stmt::Kind::Let:
      set(s.name, eval(*s.expr));
      break;
    case Stmt::Kind::If:
      exec(truth(eval(*s.expr)) ? s.body : s.orelse);
      break;
    case Stmt::Kind::Check:
      check(s);
      break;
    case Stmt::Kind::Each:
      break;
    }
  }

  /// type T = ...: fix the range of the type
  void define_type(const Stmt &s) {
    Range r = range(s.type);
    for (auto &[t, x] : type_ranges_)
      if (t == &s) {
        x = r;
        return;
      }
    type_ranges_.emplace_back(&s, r);
  }

  /**
   * param x [= default]
   *
   * A given value has the type of the default. Without a default, it is an
   * integer if possible, otherwise a float or a string.
   */
  void param(const Stmt &s) {
    Value d;
    if (s.expr)
      d = eval(*s.expr);
    std::optional<std::string_view> v = params_.get(s.name);
    if (!v) {
      if (!s.expr)
        fail(ErrorCode::ParamMissing,
             "parameter '" + s.name + "' must be given");
      set(s.name, std::move(d));
      return;
    }
    if (s.expr && (d.type == ValueType::String)) {
      set(s.name, Value::string(*v));
      return;
    }
    if (!s.expr || (d.type == ValueType::Int))
      if (std::optional<std::int64_t> i = parse_int(*v)) {
        set(s.name, Value::integer(*i));
        return;
      }
    if (!s.expr || (d.type == ValueType::Float))
      if (std::optional<double> f = parse_float(*v)) {
        set(s.name, Value::real(*f));
        return;
      }
    if (!s.expr) {
      set(s.name, Value::string(*v));
      return;
    }
    fail(ErrorCode::ParamType,
         "parameter '" + s.name + "' must be " +
             (d.type == ValueType::Int ? "an integer" : "a number") + ", not " +
             quote(*v));
  }

  /// x: type and x[d1][d2]...: type
  void read(const Stmt &s) {
    context.push_back(
        Context{.text = s.name, .iteration = 0, .iterations = std::nullopt});
    std::int64_t o = offset(s);
    Domain d{.type = s.type, .range = range(s.type)};
    if (s.dims.empty()) {
      Value v = value(d);
      offset(v, o);
      store(s, std::move(v));
    } else {
      Seq<std::size_t> dims;
      for (const std::unique_ptr<Expr> &e : s.dims)
        dims.push_back(count(*e, "array size"));
      if (s.type.read == ReadType::Symbol) {
        store(s, symbols(s.type, dims[0], dims[1]));
      } else if (s.expr) {
        Value f = eval(*s.expr);
        if ((f.depth() > 0) || (f.type != ValueType::String))
          fail(ErrorCode::Layout, "matrix layout must be a string");
        store(s, layout(d, dims[0], dims[1], f.strings[0], o));
      } else {
        Value v = array(d, dims);
        offset(v, o);
        store(s, std::move(v));
      }
    }
    context.pop_back();
  }

  /// list x: type until (eol | terminator)
  void list(const Stmt &s) {
    context.push_back(
        Context{.text = s.name, .iteration = 0, .iterations = std::nullopt});
    Value v(ValueType::Int);
    if (!spare_.empty()) {
      v = std::move(spare_.back());
      spare_.pop_back();
    }
    v.clear_list(s.type.read == ReadType::Float  ? ValueType::Float
                 : s.type.read == ReadType::Word ? ValueType::String
                                                 : ValueType::Int);
    Domain d{.type = s.type, .range = range(s.type)};
    try {
      if (s.until_eol)
        list_line(d, v);
      else
        list_until(s, d, v);
    } catch (const ShapeError &e) {
      fail(e, s.name);
    }
    v.end_list();
    offset(v, offset(s));
    context.pop_back();
    store(s, std::move(v));
  }
  /// Read the values of domain `d` on the rest of the line into `v`
  void list_line(const Domain &d, Value &v) {
    // The rest of the current line, or the next line if the current one is
    // used up (skip_to_data() moves there)
    if (!in.skip_to_line_data() && !in.skip_to_data())
      fail(ErrorCode::ExpectedValue,
           "expected a line of values, found end of input");
    while (in.skip_to_line_data())
      push(d, v);
  }
  /// Test whether the scalars `x` and `y` are numbers that are equal
  static bool same_number(const Value &x, const Value &y) {
    if ((x.type == ValueType::Int) && (y.type == ValueType::Int))
      return x.ints[0] == y.ints[0];
    if ((x.type == ValueType::String) || (y.type == ValueType::String))
      return false;
    return x.as_double() == y.as_double();
  }
  /// Read integers of domain `d` into `v` up to the terminator `stop`
  void ints_until(const Domain &d, std::int64_t stop, Value &v) {
    for (;;) {
      // The terminator need not be in the range
      std::int64_t x = int_scalar(d);
      if (x == stop)
        return;
      if (!d.range.contains(x))
        outside(d, token_start_, in.pos);
      v.ints.push_back(x);
    }
  }
  /// Read values of domain `d` into `v` up to the terminator of `s`
  void list_until(const Stmt &s, const Domain &d, Value &v) {
    if ((d.type.read == ReadType::Int) &&
        (s.sentinel->kind == Expr::Kind::Int)) {
      ints_until(d, s.sentinel->int_value, v);
      return;
    }
    Value stop = eval(*s.sentinel);
    if (stop.depth() > 0)
      fail(ErrorCode::ListTerminator, "list terminator must be a single value");
    // Words end at the token spelling an integer terminator
    if ((s.type.read == ReadType::Word) && (stop.type == ValueType::Int))
      stop = Value::string(std::to_string(stop.ints[0]));
    if ((s.type.read == ReadType::Word) && (stop.type == ValueType::Float))
      fail(ErrorCode::ListTerminator,
           "a list of words needs a string or integer terminator");
    if ((d.type.read == ReadType::Int) && (stop.type == ValueType::Int)) {
      ints_until(d, stop.ints[0], v);
      return;
    }
    for (;;) {
      // A string terminator is compared before the token is read as a
      // value, so "EOF" can end a list of numbers
      if (stop.type == ValueType::String) {
        if (!in.skip_to_data())
          fail(ErrorCode::ExpectedToken,
               "expected " + quote(stop.strings[0]) + ", found end of input");
        if (in.peek() == stop.strings[0]) {
          (void)in.token();
          return;
        }
      }
      // The terminator need not be in the range
      Value x = scalar(d);
      if (same_number(x, stop))
        return;
      if (!fits(d, x))
        outside(d, token_start_, in.pos);
      v.push(x);
    }
  }

  /// line "pattern": the rest of the current line, or the next line if the
  /// current one is used up, must match
  void line(const Stmt &s) {
    if (match(s.pattern))
      return;
    fail(ErrorCode::LineMatch,
         "expected a line matching \"" + s.pattern.source + "\"" +
             (in.pos >= in.text.size() ? ", found end of input" : ""));
  }
  /// find "pattern": skip lines until one matches
  void find(const Stmt &s) {
    while (!match(s.pattern)) {
      if (!in.skip_to_data())
        fail(ErrorCode::FindMatch,
             "no line matching \"" + s.pattern.source + "\" found");
      in.to_eol();
    }
  }
  /// skip e lines
  void skip_lines(const Stmt &s) {
    std::size_t n = count(*s.expr, "number of lines");
    for (std::size_t k = 0; k < n; k++) {
      if (!in.skip_to_data())
        fail(ErrorCode::SkipEof, "cannot skip a line at end of input");
      in.to_eol();
    }
  }
  /// expect "a" "b" ...: literal tokens
  void expect(const Stmt &s) {
    for (const std::string &w : s.words) {
      if (!in.skip_to_data())
        fail(ErrorCode::ExpectedToken,
             "expected " + quote(w) + ", found end of input");
      std::size_t p0 = in.pos;
      std::string_view t = in.token();
      if (t != w) {
        in.pos = p0;
        fail(ErrorCode::ExpectedToken,
             "expected " + quote(w) + ", found " + quote(t));
      }
    }
  }
  /// index b: the row number of the innermost repeat, counted from b
  void index(const Stmt &s) {
    if (rows_.empty())
      fail(ErrorCode::IndexContext, "'index' must be inside a repeat");
    // The base is at most INT_MAX, so the sum cannot overflow
    std::int64_t want = static_cast<std::int64_t>(rows_.back()) +
                        static_cast<std::int64_t>(count(*s.expr, "index base"));
    in.skip();
    std::size_t p0 = in.pos;
    Value v = scalar(ReadType::Int, describe(ReadType::Int));
    if (v.ints[0] != want) {
      in.pos = p0;
      fail(ErrorCode::IndexMismatch, "expected index " + std::to_string(want) +
                                         ", found " +
                                         std::to_string(v.ints[0]));
    }
  }

  /// Test whether the next token ends statement `s`, and consume it
  bool until(const Stmt &s) {
    if (!s.has_until || !in.skip_to_data())
      return false;
    std::string_view t = in.peek();
    for (const std::string &w : s.words)
      if (t == w) {
        (void)in.token();
        if (!s.store.empty())
          set(s.store, Value::string(w));
        return true;
      }
    return false;
  }
  /// Describe the terminating tokens of `s`, for messages
  static std::string until_text(const Stmt &s) {
    std::string r;
    for (std::size_t k = 0; k < s.words.size(); k++)
      r += (k > 0 ? " or " : "") + quote(s.words[k]);
    return r;
  }

  /**
   * Test whether the instance being read may not be the selected one
   *
   * When selecting by name, the instance is the selected one if it has the
   * name, and may not be while the name is not defined yet.
   */
  [[nodiscard]] bool may_be_skipped() const {
    if (current_ == 0)
      return false;
    if (!select_)
      return current_ != 1;
    if (number_)
      return std::cmp_not_equal(current_, *number_);
    return instance_name(frames[instance_frame_]) != want_;
  }

  /**
   * Reject the instance being read with error `e`
   *
   * Reading stops, unless the instance may not be the selected one: then
   * the first rejection is recorded, and reading continues (see instances).
   */
  void reject(Error e) {
    if (!may_be_skipped())
      throw std::move(e);
    if (!deferred_)
      deferred_ = std::move(e);
  }

  /// check e "message"
  void check(const Stmt &s) {
    std::optional<Error> e;
    // holds() sets e when the check fails
    if (!holds(s, e) && e)
      reject(std::move(*e));
  }

  /// Read instance `n` with the body `b` into a frame, and return it
  Seq<Entry> instance(const Block &b, std::size_t n) {
    current_ = n;
    deferred_.reset();
    context.push_back(
        Context{.text = "instance", .iteration = n, .iterations = {}});
    frames.emplace_back();
    instance_frame_ = frames.size() - 1;
    exec(b);
    Seq<Entry> f = std::move(frames.back());
    frames.pop_back();
    context.pop_back();
    current_ = 0;
    return f;
  }

  /// Return the name of the instance in frame `f`, or empty
  static std::string instance_name(const Seq<Entry> &f) {
    for (const Entry &e : f)
      if ((e.name == "name") && (e.value.depth() == 0)) {
        if (e.value.type == ValueType::String)
          return e.value.strings[0];
        if (e.value.type == ValueType::Int)
          return std::to_string(e.value.ints[0]);
      }
    return {};
  }

  /**
   * Read the instances of a file and keep the selected one
   *
   * Each instance is read into a frame of its own, which is dropped unless
   * the instance is the one wanted. The parameter `instance` selects an
   * instance by number (from 1) if it is a number, and by the value of
   * `name` otherwise. Without it, the file must contain a single instance.
   * Reading stops after the selected instance.
   *
   * A rejection (see reject) of an instance that is not the selected one
   * does not stop reading, so that an invalid instance does not hide the
   * ones after it. Other errors do, as the input cannot be read on.
   */
  void instances(const Stmt &s) {
    std::optional<std::string_view> w = params_.get("instance");
    select_ = w.has_value();
    if (select_) {
      want_ = *w;
      number_ = parse_int(want_);
      if (number_ && (*number_ < 1))
        fail(ErrorCode::InstanceNumber, "instance numbers start at 1");
    }
    std::size_t n = 0;
    while (in.skip_to_data()) {
      n++;
      std::size_t start = in.pos;
      Seq<Entry> f = instance(s.body, n);
      spec_line = s.line;
      if (in.pos == start)
        fail(ErrorCode::NoProgress, "an instance must read some input");
      bool chosen = (n == 1);
      if (select_)
        chosen =
            number_ ? std::cmp_equal(*number_, n) : (instance_name(f) == want_);
      if (chosen) {
        // An instance selected by name may have been rejected before its
        // name was known
        if (deferred_)
          throw std::move(*deferred_);
        for (Entry &e : f)
          set(e.name, std::move(e.value));
        if (select_) {
          selected_ = true;
          return;
        }
      }
    }
    if (n == 0)
      fail(ErrorCode::InstanceEmpty, "the file contains no instance");
    if (select_)
      fail(ErrorCode::InstanceNotFound,
           "there is no instance " + quote(want_) + " (the file has " +
               std::to_string(n) + " instance" + (n == 1 ? "" : "s") + ")");
    if (n > 1)
      fail(ErrorCode::InstanceSelection,
           "the file has " + std::to_string(n) +
               " instances; select one with the parameter instance (a "
               "number from 1, or a name)");
  }

  /**
   * Repeat the body of `s`
   *
   * Each iteration runs in a frame of its own. Afterwards, the value of
   * every name the body defines is added as a row to the name's column, and
   * the frame is dropped, so that only one iteration's values exist at a
   * time besides the columns.
   */
  void repeat(const Stmt &s) {
    Seq<Decl> names;
    declared(s.body, names);
    std::optional<std::size_t> n;
    if (s.expr)
      n = count(*s.expr, "repeat count");
    Seq<Column> columns;
    for (const Decl &d : names)
      columns.emplace_back(d.type, d.depth);
    // Where each record starts, for errors in checks
    Value starts;
    // The frame of an iteration, kept between iterations for its memory
    Seq<Entry> frame;
    for (std::size_t r = 0; !n || (r < *n); r++) {
      if (s.until_eof && !in.skip_to_data())
        break;
      if (until(s))
        break;
      if (s.has_until && !in.skip_to_data())
        fail(ErrorCode::RepeatTerminator,
             "expected " + until_text(s) + " before end of input");
      context.push_back(
          Context{.text = "repeat", .iteration = r + 1, .iterations = n});
      frames.push_back(std::move(frame));
      rows_.push_back(r);
      starts.ints.push_back(static_cast<std::int64_t>(in.next_data()));
      std::size_t start = in.pos;
      exec(s.body);
      if (in.pos == start) {
        if (!n)
          fail(ErrorCode::NoProgress,
               "the body of a repeat until read no input");
        work(1);
      }
      rows_.pop_back();
      Seq<Entry> &f = frames.back();
      for (std::size_t i = 0; i < names.size(); i++) {
        Entry *e = nullptr;
        for (Entry &x : f)
          if (x.name == names[i].name)
            e = &x;
        try {
          if (e == nullptr)
            columns[i].skip();
          else
            columns[i].add(e->value);
        } catch (const ShapeError &err) {
          fail(err.code, "'" + names[i].name + "' " + err.msg);
        }
      }
      // A list read in each iteration then needs no new memory
      for (Entry &e : f)
        if ((e.value.depth() > 0) && (spare_.size() < max_spare))
          spare_.push_back(std::move(e.value));
      frame = std::move(frames.back());
      frame.clear();
      frames.pop_back();
      context.pop_back();
      spec_line = s.line;
    }
    for (std::size_t i = 0; i < names.size(); i++)
      try {
        set(names[i].name, columns[i].finish());
      } catch (const ShapeError &err) {
        fail(err.code, "'" + names[i].name + "' " + err.msg);
      }
    starts.end_list();
    set(record_starts(s.id), std::move(starts));
  }

  /**
   * sections: read lines in any order, each by the first arm that matches
   * it
   *
   * An arm that is not optional must match a line, and no arm may match two
   * lines.
   */
  void sections(const Stmt &s) {
    // The line on which each arm matched, or 0
    Seq<std::size_t> seen(s.arms.size(), 0);
    std::string end = "the end of the input";
    Place end_at = in.here();
    for (;;) {
      spec_line = s.line;
      if (!in.skip_to_data()) {
        end_at = in.here();
        break;
      }
      Place at = in.here();
      if (until(s)) {
        end = until_text(s);
        end_at = at;
        break;
      }
      std::optional<std::size_t> found;
      Seq<Capture> caps;
      for (std::size_t k = 0; !found && (k < s.arms.size()); k++) {
        caps.clear();
        if (matches(s.arms[k].pattern, caps))
          found = k;
      }
      if (!found) {
        if (s.skip_other) {
          in.to_eol();
          continue;
        }
        std::string expected;
        for (std::size_t k = 0; (k < s.arms.size()) && (k < 6); k++) {
          expected += (k > 0) ? ", \"" : "\"";
          expected += s.arms[k].pattern.source;
          expected += '"';
        }
        if (s.arms.size() > 6)
          expected += ", ...";
        fail(ErrorCode::SectionsUnexpected,
             "unexpected line; expected one of " + expected);
      }
      const Arm &a = s.arms[*found];
      if (seen[*found] > 0) {
        std::string m = "a second line matching \"" + a.pattern.source + "\"";
        reject(located(ErrorCode::SectionsRepeated,
                       m + " (the first is on line " +
                           std::to_string(seen[*found]) + ")",
                       at));
      } else {
        seen[*found] = at.line;
      }
      take(caps);
      context.push_back(Context{.text = "section \"" + a.pattern.source + "\"",
                                .iteration = 0,
                                .iterations = std::nullopt});
      exec(a.body);
      context.pop_back();
      if (a.last) {
        end = "the line matching \"" + a.pattern.source + "\"";
        end_at = at;
        break;
      }
    }
    for (std::size_t k = 0; k < s.arms.size(); k++)
      if (!s.arms[k].optional && (seen[k] == 0)) {
        std::string m = "expected a line matching \"";
        m += s.arms[k].pattern.source;
        m += "\" before ";
        m += end;
        Error e = located(ErrorCode::LineMatch, std::move(m), end_at);
        // At the end of the input, no later instance can be hidden
        if (!in.skip_to_data())
          throw std::move(e);
        reject(std::move(e));
        return;
      }
  }

  /// Read an `r` by `c` array of the symbols of alphabet type `t`
  Value symbols(const Type &t, std::size_t r, std::size_t c) {
    if (static_cast<std::uint64_t>(r) * c > INT_MAX)
      fail(ErrorCode::SizeLimit, "array is too large");
    if (c == 0)
      work(r);
    Value g;
    for (std::size_t i = 0; i < r; i++)
      for (std::size_t j = 0; j < c; j++) {
        char ch = 0;
        for (;;) {
          in.skip();
          if (in.pos >= in.text.size())
            fail(ErrorCode::Symbol, "expected a symbol for row " +
                                        std::to_string(i + 1) + ", column " +
                                        std::to_string(j + 1) +
                                        ", found end of input");
          ch = in.text[in.pos];
          if (!t.strip.contains(ch))
            break;
          in.advance(in.pos + 1);
        }
        std::size_t k = t.alphabet.find(ch);
        if (t.blank.contains(ch)) {
          g.ints.push_back(0);
        } else if (k != std::string::npos) {
          std::optional<std::int64_t> v =
              checked_add(static_cast<std::int64_t>(k), t.base);
          if (!v)
            fail(ErrorCode::IntegerOverflow,
                 "integer overflow in the value of a symbol");
          g.ints.push_back(*v);
        } else {
          fail(ErrorCode::Symbol,
               "unexpected symbol " + quote(std::string(1, ch)) +
                   ", expected one of " + quote(t.alphabet + t.blank));
        }
        in.advance(in.pos + 1);
      }
    // The structure is made after reading, as for arrays
    g.shape({r, c});
    return g;
  }
};

} // namespace

DataImpl read(const SpecImpl &spec, std::string_view text, std::string origin,
              const ParamsImpl &params) {
  Interpreter i(spec, text, std::move(origin), params);
  return i.run();
}

} // namespace iris::detail
