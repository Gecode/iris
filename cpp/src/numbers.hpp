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

// Ported from gecode/instance/impl.hpp at Gecode commit
// 599f977b7855dba2a1a5f67db1892722372067d3.

#pragma once

#include "text.hpp"

#include <array>
#include <cfenv>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <string>
#include <string_view>

namespace iris::detail {

/**
 * Return where the number starting at position `p` of `s` ends
 *
 * A number has digits, a sign first if `sign`, and, if `real`, also
 * '.', 'e', 'E', and a sign directly after 'e' or 'E'. Only the
 * characters are scanned: parse_int and parse_float check the syntax.
 */
constexpr std::size_t number_end(std::string_view s, std::size_t p, bool sign,
                                 bool real) {
  std::size_t q = p;
  for (; q < s.size(); q++) {
    const char c = s[q];
    if (is_digit(c))
      continue;
    if (sign && (q == p) && is_sign(c))
      continue;
    if (real && ((c == '.') || is_exponent(c)))
      continue;
    if (real && (q > p) && is_sign(c) && is_exponent(s[q - 1]))
      continue;
    break;
  }
  return q;
}

/// Return the decimal integer `t`, which may have a sign, or nothing if `t`
/// is not one or is out of range
std::optional<std::int64_t> parse_int(std::string_view t);

/**
 * Return the decimal float `t`, which may have a sign, or nothing if `t` is
 * not one or is not representable
 *
 * The value is rounded to nearest, which needs a NearestRounding in scope.
 * A value that overflows, or a nonzero value that rounds to a subnormal or
 * to zero, is not representable (Specification §4.5).
 */
std::optional<double> parse_float(std::string_view t);

/// Room for the spelling of any finite double that write_float writes
using FloatBuffer = std::array<char, 64>;

/**
 * Write the spelling of finite `value` that Rust's `{:?}` gives into
 * `buffer`, and return it as a view of `buffer`
 *
 * It is the shortest text that parse_float reads back as the same number,
 * in scientific notation below 1e-4 and from 1e16 on, and with `.0` on a
 * whole number, so that it reads as a float and not an integer. It
 * allocates nothing, so that Params can format typed parameter values
 * without leaving its memory resource.
 */
std::string_view write_float(double value, FloatBuffer &buffer);

/// Return the spelling of finite `value` that write_float writes. The
/// builder prints float literals with it.
std::string float_text(double value);

/**
 * Round to nearest, ties to even, while this object exists
 *
 * Every conversion to binary64 in a read rounds to nearest, whatever the
 * caller's rounding mode (Specification §4.5 and §6.2). The library's
 * entry points hold one of these. Destruction restores the caller's
 * floating-point environment, exception flags included, and no exception
 * traps in between.
 */
class NearestRounding {
public:
  NearestRounding() {
    std::feholdexcept(&caller_);
    std::fesetround(FE_TONEAREST);
  }
  ~NearestRounding() { std::fesetenv(&caller_); }
  NearestRounding(const NearestRounding &) = delete;
  NearestRounding(NearestRounding &&) = delete;
  NearestRounding &operator=(const NearestRounding &) = delete;
  NearestRounding &operator=(NearestRounding &&) = delete;

private:
  std::fenv_t caller_{};
};

/*
 * Checked integer arithmetic (Specification §8.4): each function returns
 * nothing if the exact result is not a signed 64-bit integer
 */

/// Return `x` + `y`
std::optional<std::int64_t> checked_add(std::int64_t x, std::int64_t y);
/// Return `x` - `y`
std::optional<std::int64_t> checked_sub(std::int64_t x, std::int64_t y);
/// Return `x` * `y`
std::optional<std::int64_t> checked_mul(std::int64_t x, std::int64_t y);
/// Return -`x`
std::optional<std::int64_t> checked_neg(std::int64_t x);
/// Return `x` / `y`, truncated towards zero, for `y` other than 0
std::optional<std::int64_t> checked_div(std::int64_t x, std::int64_t y);
/// Return the remainder of `x` / `y`, with the sign of `x`, for `y` other
/// than 0
std::optional<std::int64_t> checked_rem(std::int64_t x, std::int64_t y);
/// Return `x` to the power of `y` >= 0
std::optional<std::int64_t> checked_pow(std::int64_t x, std::int64_t y);

} // namespace iris::detail
