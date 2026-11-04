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

// Ported from the dzn output of gecode/instance/data.cpp at Gecode commit
// 5e7bbd5c26b0702c6fd652356b2289da0710ac3c.

#include <iris/data.hpp>

#include "text.hpp"
#include "value.hpp"

#include <algorithm>
#include <array>
#include <bit>
#include <cassert>
#include <cmath>
#include <compare>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <string>
#include <string_view>
#include <utility>

namespace iris {

namespace {

using detail::Value;

/**
 * A natural number below 2^1280, enough for the scaled values of
 * append_float
 *
 * Its limbs are 32-bit, least significant first; limbs from size_ on are
 * zero.
 */
class Natural {
public:
  explicit Natural(std::uint64_t v)
      : limbs_{static_cast<std::uint32_t>(v),
               static_cast<std::uint32_t>(v >> 32U)} {
    trim(2);
  }

  /// Multiply by 2^`bits`
  void shift(std::size_t bits) {
    if (size_ == 0)
      return;
    const std::size_t words = bits / 32;
    const std::size_t rest = bits % 32;
    const std::size_t n = size_ + words + 1;
    assert(n <= capacity);
    // From the top, so that each limb is read before it is overwritten
    for (std::size_t i = n; i-- > words;) {
      const std::size_t from = i - words;
      const std::uint64_t high = (from < size_) ? limbs_[from] : 0;
      const std::uint64_t low = (from > 0) ? limbs_[from - 1] : 0;
      limbs_[i] =
          static_cast<std::uint32_t>((((high << 32U) | low) << rest) >> 32U);
    }
    std::fill_n(limbs_.begin(), words, 0);
    trim(n);
  }
  /// Multiply by `factor`
  void multiply(std::uint32_t factor) {
    std::uint64_t carry = 0;
    for (std::size_t i = 0; i < size_; i++) {
      carry += std::uint64_t{limbs_[i]} * factor;
      limbs_[i] = static_cast<std::uint32_t>(carry);
      carry >>= 32U;
    }
    if (carry != 0) {
      assert(size_ < capacity);
      limbs_[size_++] = static_cast<std::uint32_t>(carry);
    }
  }
  /// Multiply by 10^`k`
  void multiply_power_of_ten(std::size_t k) {
    static constexpr std::array<std::uint32_t, 10> powers = {
        1,      10,      100,      1000,      10000,
        100000, 1000000, 10000000, 100000000, 1000000000};
    for (; k >= 9; k -= 9)
      multiply(powers[9]);
    multiply(powers[k]);
  }
  /// Subtract `x`, which is at most this number
  void subtract(const Natural &x) {
    std::uint64_t borrow = 0;
    for (std::size_t i = 0; i < size_; i++) {
      const std::uint64_t y = std::uint64_t{x.limbs_[i]} + borrow;
      borrow = (limbs_[i] < y) ? 1 : 0;
      limbs_[i] = static_cast<std::uint32_t>((borrow << 32U) + limbs_[i] - y);
    }
    trim(size_);
  }
  friend Natural operator+(const Natural &x, const Natural &y) {
    Natural r(0);
    const std::size_t n = std::max(x.size_, y.size_);
    std::uint64_t carry = 0;
    for (std::size_t i = 0; i < n; i++) {
      carry += std::uint64_t{x.limbs_[i]} + y.limbs_[i];
      r.limbs_[i] = static_cast<std::uint32_t>(carry);
      carry >>= 32U;
    }
    assert((carry == 0) || (n < capacity));
    if (carry != 0)
      r.limbs_[n] = static_cast<std::uint32_t>(carry);
    r.trim(n + 1);
    return r;
  }
  friend std::strong_ordering operator<=>(const Natural &x, const Natural &y) {
    if (x.size_ != y.size_)
      return x.size_ <=> y.size_;
    for (std::size_t i = x.size_; i-- > 0;)
      if (x.limbs_[i] != y.limbs_[i])
        return x.limbs_[i] <=> y.limbs_[i];
    return std::strong_ordering::equal;
  }

private:
  static constexpr std::size_t capacity = 40;
  std::array<std::uint32_t, capacity> limbs_{};
  std::size_t size_ = 0;

  /// Set size_ to the limbs in use among the first `n`
  void trim(std::size_t n) {
    size_ = std::min(n, capacity);
    while ((size_ > 0) && (limbs_[size_ - 1] == 0))
      size_--;
  }
};

/**
 * Append the MiniZinc spelling of float `f` to `out` (Specification §B.3)
 *
 * The digits come from exact integer arithmetic, so they do not depend on
 * the floating-point rounding mode. Before each digit the value of f is
 * r / s and the gaps to the midpoints with its neighbours are plus / s and
 * minus / s, in units of that digit.
 */
void append_float(std::string &out, double f) {
  if (f == 0.0) {
    out += std::signbit(f) ? "-0.0" : "0.0";
    return;
  }
  if (std::signbit(f))
    out += '-';
  // f is mantissa * 2^exponent
  const auto bits = std::bit_cast<std::uint64_t>(f);
  const auto biased = static_cast<int>((bits >> 52U) & 0x7ffU);
  const std::uint64_t fraction = bits & ((std::uint64_t{1} << 52U) - 1);
  const std::uint64_t mantissa =
      (biased == 0) ? fraction : (fraction | (std::uint64_t{1} << 52U));
  const int exponent = (biased == 0) ? -1074 : biased - 1075;
  // Decimals within the gaps read back as f; at the boundary, f wins the
  // tie if its mantissa is even
  const bool even = (mantissa % 2) == 0;
  // Above the smallest normal power of two the gap below is half that
  // above. The values are in units of 2^(exponent - scale): f is then
  // mantissa * 2^scale, and the half gaps 2^(scale - 1) above and 1 below.
  const int scale = ((fraction == 0) && (biased > 1)) ? 2 : 1;
  Natural r(mantissa << static_cast<unsigned>(scale));
  Natural s(1);
  Natural plus(std::uint64_t{1} << static_cast<unsigned>(scale - 1));
  Natural minus(1);
  if (exponent >= scale) {
    const auto unit = static_cast<std::size_t>(exponent - scale);
    r.shift(unit);
    plus.shift(unit);
    minus.shift(unit);
  } else {
    s.shift(static_cast<std::size_t>(scale - exponent));
  }
  // The decimal exponent k with 10^k <= f < 10^(k + 1), from an estimate
  // with floor(log2 f) * 78913 / 2^18 that the loops correct
  int k = ((63 - std::countl_zero(mantissa) + exponent) * 78913) >> 18;
  if (k >= 0) {
    s.multiply_power_of_ten(static_cast<std::size_t>(k));
  } else {
    r.multiply_power_of_ten(static_cast<std::size_t>(-k));
    plus.multiply_power_of_ten(static_cast<std::size_t>(-k));
    minus.multiply_power_of_ten(static_cast<std::size_t>(-k));
  }
  Natural ten_s = s;
  ten_s.multiply(10);
  while (r >= ten_s) {
    s = ten_s;
    ten_s.multiply(10);
    k++;
  }
  while (r < s) {
    r.multiply(10);
    plus.multiply(10);
    minus.multiply(10);
    k--;
  }
  // Integral floats start with all integer digits, to give 20.0 rather
  // than 2e+01
  const int start = ((k >= 0) && (k <= 16)) ? k + 1 : 1;
  std::array<char, 17> digits{};
  int precision = 0;
  for (;;) {
    int digit = 0;
    while (r >= s) {
      r.subtract(s);
      digit++;
    }
    digits[static_cast<std::size_t>(precision++)] =
        static_cast<char>('0' + digit);
    if (precision >= start) {
      // Round the exact value half to even, then test whether the decimal
      // reads back as f
      const std::strong_ordering half = (r + r) <=> s;
      const bool up = (half > 0) || ((half == 0) && (digit % 2 == 1));
      const std::strong_ordering inside = up ? (r + plus) <=> s : minus <=> r;
      if ((inside > 0) || ((inside == 0) && even) || (precision == 17)) {
        if (up) {
          int i = precision - 1;
          for (; (i >= 0) && (digits[static_cast<std::size_t>(i)] == '9'); i--)
            digits[static_cast<std::size_t>(i)] = '0';
          if (i >= 0) {
            digits[static_cast<std::size_t>(i)]++;
          } else {
            digits[0] = '1';
            k++;
          }
        }
        break;
      }
    }
    r.multiply(10);
    plus.multiply(10);
    minus.multiply(10);
  }
  // The significant digits, without trailing zeros
  auto length = static_cast<std::size_t>(precision);
  while ((length > 1) && (digits[length - 1] == '0'))
    length--;
  const std::string_view significant(digits.data(), length);
  if ((k < -4) || (k >= precision)) {
    out += significant[0];
    if (length > 1) {
      out += '.';
      out += significant.substr(1);
    }
    out += (k < 0) ? "e-" : "e+";
    const int magnitude = std::abs(k);
    if (magnitude < 10)
      out += '0';
    out += std::to_string(magnitude);
  } else if (k >= 0) {
    const auto integer = static_cast<std::size_t>(k) + 1;
    if (length <= integer) {
      out += significant;
      out.append(integer - length, '0');
      out += ".0";
    } else {
      out += significant.substr(0, integer);
      out += '.';
      out += significant.substr(integer);
    }
  } else {
    out += "0.";
    out.append(static_cast<std::size_t>(-k - 1), '0');
    out += significant;
  }
}

/// Append string `s` to `out` as a MiniZinc string (Specification §B.4)
void append_string(std::string &out, std::string_view s) {
  static constexpr std::string_view hex = "0123456789abcdef";
  out += '"';
  for (const char c : s) {
    const auto byte = static_cast<unsigned char>(c);
    if ((c == '"') || (c == '\\')) {
      out += '\\';
      out += c;
    } else if (c == '\n') {
      out += "\\n";
    } else if (c == '\t') {
      out += "\\t";
    } else if (byte < 0x20) {
      out += "\\x";
      out += hex[byte >> 4];
      out += hex[byte & 0xf];
    } else {
      out += c;
    }
  }
  out += '"';
}

/// Append leaf `i` of `v` to `out` (Specification §B.2 to §B.4)
void append_leaf(std::string &out, const Value &v, std::size_t i) {
  switch (v.type) {
  case ValueType::Int:
    out += std::to_string(v.ints[i]);
    return;
  case ValueType::Float:
    append_float(out, v.floats[i]);
    return;
  case ValueType::String:
    append_string(out, v.strings[i]);
    return;
  }
  std::unreachable();
}

/// Append the leaves of `v` from `from` to before `to` to `out`, separated
/// by commas
void append_leaves(std::string &out, const Value &v, std::size_t from,
                   std::size_t to) {
  for (std::size_t i = from; i < to; i++) {
    if (i > from)
      out += ", ";
    append_leaf(out, v, i);
  }
}

/// Append name `n` to `out` as a MiniZinc identifier, quoted if it is not
/// one (Specification §B.2)
void append_name(std::string &out, const std::string &n) {
  static constexpr std::array<std::string_view, 51> keywords = {
      "ann",        "annotation", "any",    "array",     "bool",     "case",
      "constraint", "default",    "diff",   "div",       "else",     "elseif",
      "endif",      "enum",       "false",  "float",     "function", "if",
      "in",         "include",    "int",    "intersect", "let",      "list",
      "maximize",   "minimize",   "mod",    "not",       "of",       "op",
      "opt",        "output",     "par",    "predicate", "record",   "satisfy",
      "set",        "solve",      "string", "subset",    "superset", "symdiff",
      "test",       "then",       "true",   "tuple",     "type",     "union",
      "var",        "where",      "xor"};
  // MiniZinc identifiers start with a letter
  if (!n.empty() && detail::is_letter(n[0]) &&
      (std::ranges::find(keywords, n) == keywords.end())) {
    out += n;
    return;
  }
  out += '\'';
  out += n;
  out += '\'';
}

/// Return the length of the first list at `level` of `v`, or 0 if there is
/// none
std::size_t extent(const Value &v, std::size_t level) {
  const detail::Seq<std::size_t> &offsets = v.levels[level];
  return offsets.size() > 1 ? offsets[1] - offsets[0] : 0;
}

/// Test whether all lists at each level of `v` have the same length
bool rectangular(const Value &v) {
  for (std::size_t level = 1; level < v.depth(); level++)
    for (std::size_t pos = 0; pos + 1 < v.levels[level].size(); pos++)
      if (v.list_size(level, pos) != extent(v, level))
        return false;
  return true;
}

/**
 * Append rectangular array `v`, of depth 1 to 6, to `out` (Specification
 * §B.5)
 *
 * Depth 1 is a list and depth 2 a matrix, unless it is empty. Other depths
 * need MiniZinc's arrayNd with the extent of each level.
 */
void append_rectangular(std::string &out, const Value &v) {
  const std::size_t leaves = v.leaves();
  if (v.depth() == 1) {
    out += '[';
    append_leaves(out, v, 0, leaves);
    out += ']';
    return;
  }
  const std::size_t columns = extent(v, 1);
  if ((v.depth() == 2) && (v.size() > 0) && (columns > 0)) {
    out += "[| ";
    for (std::size_t row = 0; row < leaves; row += columns) {
      if (row > 0)
        out += "\n  | ";
      append_leaves(out, v, row, row + columns);
    }
    out += " |]";
    return;
  }
  out += "array" + std::to_string(v.depth()) + "d(";
  for (std::size_t level = 0; level < v.depth(); level++)
    out += "1.." + std::to_string(extent(v, level)) + ", ";
  out += '[';
  append_leaves(out, v, 0, leaves);
  out += "])";
}

/**
 * Append array `v` to `out` as a MiniZinc array of one-tuples of its
 * lists (Specification §B.6)
 *
 * An explicit stack of the lists being written, one per level, keeps deep
 * arrays off the call stack.
 */
void append_tuples(std::string &out, const Value &v) {
  // A list being written: its children are from `from` to before `end`,
  // and `next` is the first child not yet written
  struct Open {
    std::size_t from;
    std::size_t next;
    std::size_t end;
  };
  detail::Seq<Open> open;
  const auto start = [&](std::size_t level, std::size_t pos) {
    const std::size_t from = v.levels[level][pos];
    out += '[';
    open.push_back(
        {.from = from, .next = from, .end = v.levels[level][pos + 1]});
  };
  start(0, 0);
  while (!open.empty()) {
    const std::size_t level = open.size() - 1;
    Open &o = open.back();
    const bool leaves = level + 1 == v.depth();
    if (!leaves && (o.next < o.end)) {
      if (o.next > o.from)
        out += ", ";
      out += '(';
      const std::size_t child = o.next++;
      start(level + 1, child);
      continue;
    }
    if (leaves)
      append_leaves(out, v, o.from, o.end);
    out += ']';
    open.pop_back();
    // Close the tuple around a child list
    if (!open.empty())
      out += ",)";
  }
}

/// Append the comment with the MiniZinc type of the tuples of array `v`
/// named `n` to `out` (Specification §B.6)
void append_tuple_type(std::string &out, const std::string &n, const Value &v) {
  out += "% ";
  out += n;
  out += ": ";
  for (std::size_t level = 1; level < v.depth(); level++)
    out += "array[int] of tuple(";
  out += "array[int] of ";
  out += type_name(v.type);
  out.append(v.depth() - 1, ')');
  out += '\n';
}

/// Append the assignment of entry `e` to `out` (Specification §B.8)
void append_entry(std::string &out, const detail::Entry &e) {
  const Value &v = e.value;
  // MiniZinc has array constructors up to array6d; other arrays are tuples
  const bool tuples = (v.depth() > 6) || !rectangular(v);
  if (tuples)
    append_tuple_type(out, e.name, v);
  append_name(out, e.name);
  out += " = ";
  if (v.depth() == 0)
    append_leaf(out, v, 0);
  else if (tuples)
    append_tuples(out, v);
  else
    append_rectangular(out, v);
  out += ";\n";
}

} // namespace

std::string Data::dzn() const {
  std::string out;
  for (const detail::Entry &e : impl_->entries)
    append_entry(out, e);
  return out;
}

} // namespace iris
