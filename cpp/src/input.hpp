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

// Ported from gecode/instance/eval.hpp at Gecode commit
// 5e7bbd5c26b0702c6fd652356b2289da0710ac3c.

#pragma once

#include "ast.hpp"
#include "text.hpp"

#include <array>
#include <cassert>
#include <cstddef>
#include <cstdint>
#include <limits>
#include <string>
#include <string_view>
#include <utility>

namespace iris::detail {

/// A position in the input, with its line
struct Place {
  /// Position
  std::size_t pos;
  /// Line (1-based)
  std::size_t line;
  /// Start of the line
  std::size_t start;
  /// Return the column (1-based, in bytes, Specification §4.6)
  [[nodiscard]] std::size_t column() const { return pos - start + 1; }
};

/// Instance text with a position and line tracking
class Input {
public:
  /// The text
  std::string_view text;
  /// Where the text comes from
  String origin;
  /// Comment line prefixes
  Seq<String> comments;
  /// Prefixes of lines that end the data
  Seq<String> stops;
  /// Current position
  std::size_t pos = 0;
  /// Current line (1-based)
  std::size_t line = 1;
  /// Start of current line
  std::size_t start = 0;

  /// Initialize for `t` from `o`
  Input(std::string_view t, String o) : text(t), origin(std::move(o)) {
    for (std::size_t c = 0; c < kind_.size(); c++)
      kind_[c] = is_space(static_cast<char>(c)) ? CharKind::Separator
                                                : CharKind::Plain;
  }

  /// Make the characters `cs` tokens by themselves, unless they separate
  /// tokens
  void add_split(std::string_view cs) {
    for (const char c : cs)
      if (kind_of(c) == CharKind::Plain)
        kind_[static_cast<unsigned char>(c)] = CharKind::Split;
  }
  /// Make the characters `cs` separate tokens like white space
  void add_separators(std::string_view cs) {
    for (const char c : cs)
      kind_[static_cast<unsigned char>(c)] = CharKind::Separator;
  }

  /// Move forward to `p`
  void advance(std::size_t p) {
    while (pos < p) {
      if (text[pos] == '\n') {
        line++;
        start = pos + 1;
      }
      pos++;
    }
  }
  /// Test whether only blanks precede the position on its line
  [[nodiscard]] bool line_start() const {
    for (std::size_t k = start; k < pos; k++)
      if (!is_blank(text[k]))
        return false;
    return true;
  }
  /// Test whether the text at the position starts with `prefix`
  [[nodiscard]] bool starts_with(std::string_view prefix) const {
    if (prefix.empty() || !text.substr(pos).starts_with(prefix))
      return false;
    const std::size_t after = pos + prefix.size();
    return !is_alnum(prefix.back()) || (after >= text.size()) ||
           !is_alnum(text[after]);
  }
  /// Move to the end of the current line
  void to_eol() {
    const std::size_t p = text.find('\n', pos);
    advance(p == std::string_view::npos ? text.size() : p);
  }
  /// Test whether `c` separates tokens
  [[nodiscard]] bool is_separator(char c) const {
    return kind_of(c) == CharKind::Separator;
  }
  /// Test whether `c` forms a token by itself
  [[nodiscard]] bool is_split(char c) const {
    return kind_of(c) == CharKind::Split;
  }
  /// Test whether position `q` is within the current line
  [[nodiscard]] bool in_line(std::size_t q) const {
    return (q < text.size()) && (text[q] != '\n');
  }
  /// Skip white space, comment lines, and everything after a stop line
  void skip() {
    for (;;) {
      while ((pos < text.size()) && is_separator(text[pos])) {
        if (text[pos] == '\n') {
          line++;
          start = pos + 1;
        }
        pos++;
      }
      if ((pos >= text.size()) || !line_start())
        return;
      bool comment = false;
      bool stop = false;
      for (const String &c : comments)
        comment = comment || starts_with(c);
      for (const String &s : stops)
        stop = stop || starts_with(s);
      if (comment) {
        to_eol();
      } else if (stop) {
        advance(text.size());
        return;
      } else {
        return;
      }
    }
  }
  /// Skip to the next data (see skip), and return whether there is any
  bool skip_to_data() {
    skip();
    return pos < text.size();
  }
  /// Skip blanks and separators, and return whether data follows on the
  /// line
  bool skip_to_line_data() {
    while (in_line(pos) && is_separator(text[pos]))
      pos++;
    return in_line(pos);
  }
  /// Return where the token at the position ends
  [[nodiscard]] std::size_t token_end() const {
    std::size_t q = pos;
    if ((q < text.size()) && is_split(text[q]))
      return q + 1;
    while ((q < text.size()) && (kind_of(text[q]) == CharKind::Plain))
      q++;
    return q;
  }
  /// Read the token at the position
  std::string_view token() {
    const std::size_t q = token_end();
    const std::string_view t = text.substr(pos, q - pos);
    advance(q);
    return t;
  }
  /**
   * Read the token at the position as an integer `v`, and return whether it
   * is one
   *
   * Accepts what parse_int accepts, without making a string.
   */
  bool int_token(std::int64_t &v) {
    const std::size_t n = text.size();
    std::size_t p = pos;
    if ((p < n) && is_split(text[p])) {
      if (!is_digit(text[p]))
        return false;
      v = text[p] - '0';
      pos = p + 1;
      return true;
    }
    const bool negative = (p < n) && (text[p] == '-');
    if ((p < n) && is_sign(text[p]))
      p++;
    const std::size_t digits = p;
    // The magnitude, which may be one more than the maximum if negative
    const std::uint64_t limit =
        static_cast<std::uint64_t>(std::numeric_limits<std::int64_t>::max()) +
        (negative ? 1 : 0);
    std::uint64_t m = 0;
    for (;
         (p < n) && is_digit(text[p]) && (kind_of(text[p]) == CharKind::Plain);
         p++) {
      const auto d = static_cast<std::uint64_t>(text[p] - '0');
      if (m > (limit - d) / 10)
        return false;
      m = (10 * m) + d;
    }
    // The token must end after the digits
    if ((p == digits) || ((p < n) && (kind_of(text[p]) == CharKind::Plain)))
      return false;
    v = negative ? static_cast<std::int64_t>(0 - m)
                 : static_cast<std::int64_t>(m);
    // A token holds no newline
    pos = p;
    return true;
  }
  /// Return the next token without consuming it
  std::string_view peek() {
    const Place p0 = here();
    const std::string_view t = skip_to_data() ? token() : std::string_view();
    go_back(p0);
    return t;
  }
  /// Return the line from position `s` on without trailing blanks
  [[nodiscard]] std::string_view line_from(std::size_t s) const {
    std::size_t q = text.find('\n', s);
    if (q == std::string_view::npos)
      q = text.size();
    while ((q > s) && is_blank(text[q - 1]))
      q--;
    return text.substr(s, q - s);
  }
  /// Return the rest of the current line without trailing blanks
  [[nodiscard]] std::string_view rest() const { return line_from(pos); }
  /// Return the line starting at `s` as an error shows it: without trailing
  /// blanks, and cut after 100 bytes and then followed by `...`
  [[nodiscard]] std::string excerpt(std::size_t s) const {
    static constexpr std::size_t shown = 100;
    const std::string_view content = line_from(s);
    if (content.size() <= shown)
      return std::string(content);
    return std::string(content.substr(0, shown)) + "...";
  }
  /// Return the current place
  [[nodiscard]] Place here() const {
    return {.pos = pos, .line = line, .start = start};
  }
  /// Return the place of position `q`, which is not after the current one
  [[nodiscard]] Place place(std::size_t q) const {
    assert(q <= pos);
    // Counted back from the current place, as q is usually close to it
    Place p{.pos = q, .line = line, .start = q};
    for (std::size_t k = q; k < pos; k++)
      if (text[k] == '\n')
        p.line--;
    while ((p.start > 0) && (text[p.start - 1] != '\n'))
      p.start--;
    return p;
  }
  /// Return where the next data starts (see skip), without moving there
  std::size_t next_data() {
    const Place p0 = here();
    skip();
    const std::size_t r = pos;
    go_back(p0);
    return r;
  }

private:
  /// What a character is in tokens
  enum class CharKind : unsigned char {
    Plain,     ///< Part of a token
    Separator, ///< Separates tokens, like white space
    Split      ///< A token by itself
  };
  /// The kind of each character
  std::array<CharKind, 256> kind_{};
  /// Return the kind of `c`
  [[nodiscard]] CharKind kind_of(char c) const {
    return kind_[static_cast<unsigned char>(c)];
  }
  /// Move back to place `p`
  void go_back(const Place &p) {
    pos = p.pos;
    line = p.line;
    start = p.start;
  }
};

} // namespace iris::detail
