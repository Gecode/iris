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

// Ported from gecode/instance/spec.cpp at Gecode commit
// 5e7bbd5c26b0702c6fd652356b2289da0710ac3c.

#include "numbers.hpp"

#include <algorithm>
#include <charconv>
#include <cmath>
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

} // namespace iris::detail
