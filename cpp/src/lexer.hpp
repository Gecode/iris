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
// 599f977b7855dba2a1a5f67db1892722372067d3.

#pragma once

#include "memory.hpp"

#include <cstddef>
#include <cstdint>
#include <string>
#include <string_view>

namespace iris::detail {

/// Token of the reader-spec language
struct Token {
  /// Kind of token
  enum class Kind { Ident, Int, Float, Str, Punct, End };
  Kind kind = Kind::End;
  /// Text, or the characters of a string
  String text;
  /// Value of an integer
  std::int64_t int_value = 0;
  /// Value of a float
  double float_value = 0.0;
  /// Line in the reader spec
  std::size_t line = 0;
  /// Whether no white space precedes the token
  bool glued = false;
};

/// Prefix of the names of Gecode constants
inline constexpr std::string_view gecode_prefix = "gecode.";

/// Test whether `n` is the name of a Gecode constant
inline bool is_gecode_name(std::string_view n) {
  return n.starts_with(gecode_prefix);
}

/**
 * Split reader spec `src` from `origin`, which starts on line `line`, into
 * tokens, ending with a token of kind End
 *
 * Throws Error for a bad string, number, or character.
 */
Seq<Token> lex(std::string_view src, const String &origin,
               std::size_t line = 1);

} // namespace iris::detail
