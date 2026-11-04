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
// 5e7bbd5c26b0702c6fd652356b2289da0710ac3c.

#pragma once

#include "text.hpp"

#include <cstddef>
#include <cstdint>
#include <optional>
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
 * The value is rounded to nearest. A value that overflows, or a nonzero
 * value that rounds to a subnormal or to zero, is not representable
 * (Specification §4.5).
 */
std::optional<double> parse_float(std::string_view t);

} // namespace iris::detail
