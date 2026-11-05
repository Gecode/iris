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

#include <cstddef>
#include <optional>
#include <string_view>

namespace iris::detail {

/*
 * Character classes and number syntax, ASCII only and independent of the
 * global locale
 */

/// Test whether `c` is a digit
constexpr bool is_digit(char c) { return (c >= '0') && (c <= '9'); }
/// Test whether `c` is a letter
constexpr bool is_letter(char c) {
  return ((c >= 'a') && (c <= 'z')) || ((c >= 'A') && (c <= 'Z'));
}
/// Test whether `c` can occur in a name
constexpr bool is_alnum(char c) {
  return is_digit(c) || is_letter(c) || (c == '_');
}
/// Test whether `c` is white space
constexpr bool is_space(char c) {
  return (c == ' ') || (c == '\t') || (c == '\n') || (c == '\r') ||
         (c == '\v') || (c == '\f');
}
/// Test whether `c` is white space within a line
constexpr bool is_blank(char c) {
  return (c == ' ') || (c == '\t') || (c == '\r');
}
/// Test whether `c` is a sign
constexpr bool is_sign(char c) { return (c == '+') || (c == '-'); }
/// Test whether `c` starts the exponent of a float
constexpr bool is_exponent(char c) { return (c == 'e') || (c == 'E'); }

/**
 * Return the offset of the first byte of the first ill-formed UTF-8
 * subsequence of `text`, or nothing if `text` is valid UTF-8.
 *
 * The check is strict: bad continuation bytes, truncated sequences
 * (reported at their first byte), overlong forms, surrogates, and values
 * above U+10FFFF are ill-formed.
 */
std::optional<std::size_t> find_invalid_utf8(std::string_view text);

/// Return the UTF-8 character that starts `text`, which is valid UTF-8 and
/// not empty
std::string_view first_character(std::string_view text);

} // namespace iris::detail
