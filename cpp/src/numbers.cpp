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

// Ported from gecode/instance/spec.cpp and eval.cpp at Gecode commit
// 599f977b7855dba2a1a5f67db1892722372067d3.

#include "numbers.hpp"

#include <algorithm>
#include <charconv>
#include <cmath>
#include <limits>
#include <system_error>

namespace iris::detail {

namespace {

/// Return `t` without a leading '+', which std::from_chars does not accept,
/// or nothing if another sign follows that '+'
std::optional<std::string_view> without_plus(std::string_view t) {
  if (!t.starts_with('+'))
    return t;
  t.remove_prefix(1);
  if (!t.empty() && is_sign(t[0]))
    return std::nullopt;
  return t;
}

/// Test whether the digits before the exponent of float `t` are all zero,
/// so that its exact value is zero
bool is_exact_zero(std::string_view t) {
  const std::string_view significand = t.substr(0, t.find_first_of("eE"));
  return std::ranges::all_of(significand, [](char c) {
    return (c == '0') || (c == '.') || is_sign(c);
  });
}

} // namespace

std::optional<std::int64_t> parse_int(std::string_view t) {
  const std::optional<std::string_view> digits = without_plus(t);
  if (!digits || digits->empty())
    return std::nullopt;
  std::int64_t v = 0;
  const auto [end, ec] =
      std::from_chars(digits->data(), digits->data() + digits->size(), v);
  if ((ec != std::errc()) || (end != digits->data() + digits->size()))
    return std::nullopt;
  return v;
}

std::optional<double> parse_float(std::string_view t) {
  // Only decimal notation: no hexadecimal, infinity, or NaN
  if (t.empty() || (number_end(t, 0, true, true) != t.size()))
    return std::nullopt;
  const std::optional<std::string_view> digits = without_plus(t);
  if (!digits || digits->empty())
    return std::nullopt;
  double v = 0.0;
  const auto [end, ec] =
      std::from_chars(digits->data(), digits->data() + digits->size(), v,
                      std::chars_format::general);
  // Out-of-range values fail, and so do nonzero values that underflow
  if ((ec != std::errc()) || (end != digits->data() + digits->size()))
    return std::nullopt;
  if ((std::fpclassify(v) == FP_SUBNORMAL) || ((v == 0.0) && !is_exact_zero(t)))
    return std::nullopt;
  return v;
}

namespace {

constexpr std::int64_t int_min = std::numeric_limits<std::int64_t>::min();
constexpr std::int64_t int_max = std::numeric_limits<std::int64_t>::max();

} // namespace

std::optional<std::int64_t> checked_add(std::int64_t x, std::int64_t y) {
  if (((y > 0) && (x > int_max - y)) || ((y < 0) && (x < int_min - y)))
    return std::nullopt;
  return x + y;
}

std::optional<std::int64_t> checked_sub(std::int64_t x, std::int64_t y) {
  if (((y < 0) && (x > int_max + y)) || ((y > 0) && (x < int_min + y)))
    return std::nullopt;
  return x - y;
}

std::optional<std::int64_t> checked_mul(std::int64_t x, std::int64_t y) {
  // Sign-aware bounds that never negate the minimum
  bool ok = true;
  if (x > 0)
    ok = (y > 0) ? (x <= int_max / y) : (y >= int_min / x);
  else if (x < 0)
    ok = (y > 0) ? (x >= int_min / y) : ((y == 0) || (y >= int_max / x));
  if (!ok)
    return std::nullopt;
  return x * y;
}

std::optional<std::int64_t> checked_neg(std::int64_t x) {
  if (x == int_min)
    return std::nullopt;
  return -x;
}

std::optional<std::int64_t> checked_div(std::int64_t x, std::int64_t y) {
  if ((x == int_min) && (y == -1))
    return std::nullopt;
  return x / y;
}

std::optional<std::int64_t> checked_rem(std::int64_t x, std::int64_t y) {
  if ((x == int_min) && (y == -1))
    return std::nullopt;
  return x % y;
}

std::optional<std::int64_t> checked_pow(std::int64_t x, std::int64_t y) {
  // Squaring the base only overflows if the result does, as the base is
  // squared only while bits of the exponent remain
  std::int64_t r = 1;
  while (y > 0) {
    if ((y & 1) != 0) {
      const std::optional<std::int64_t> p = checked_mul(r, x);
      if (!p)
        return std::nullopt;
      r = *p;
    }
    y >>= 1;
    if (y > 0) {
      const std::optional<std::int64_t> s = checked_mul(x, x);
      if (!s)
        return std::nullopt;
      x = *s;
    }
  }
  return r;
}

} // namespace iris::detail
