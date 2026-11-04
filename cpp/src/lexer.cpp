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

#include "lexer.hpp"

#include "fail.hpp"
#include "numbers.hpp"
#include "text.hpp"

#include <algorithm>
#include <array>
#include <utility>

namespace iris::detail {

namespace {

using namespace std::string_view_literals;

/// Splits a reader spec into tokens
class Lexer {
public:
  /// Initialize for `src` from `origin`, which starts on line `line`
  Lexer(std::string_view src, const String &origin, std::size_t line)
      : src_(src), origin_(origin), line_(line) {}

  /// Return the tokens of the whole spec, ending with a token of kind End
  Seq<Token> run() {
    Seq<Token> toks;
    while (skip_space()) {
      Token t;
      t.line = line_;
      t.glued = !space_;
      space_ = false;
      const char c = src_[p_];
      if (c == '"')
        string(t);
      else if (is_digit(c))
        number(t);
      else if (is_alnum(c))
        name(t);
      else
        punctuation(t);
      toks.push_back(std::move(t));
    }
    Token end;
    end.line = line_;
    toks.push_back(std::move(end));
    return toks;
  }

private:
  std::string_view src_;
  const String &origin_;
  std::size_t line_;
  /// Current position
  std::size_t p_ = 0;
  /// Whether white space or a comment precedes the current position
  bool space_ = true;

  [[noreturn]] void fail(ErrorCode code, std::string_view m) const {
    fail_at(code, origin_, line_, m);
  }

  /// Skip white space and comments, and return whether a token follows
  bool skip_space() {
    while (p_ < src_.size()) {
      const char c = src_[p_];
      if (c == '\n') {
        line_++;
        p_++;
      } else if (is_space(c)) {
        p_++;
      } else if (c == '#') {
        while ((p_ < src_.size()) && (src_[p_] != '\n'))
          p_++;
      } else {
        return true;
      }
      space_ = true;
    }
    return false;
  }

  /**
   * Return the character that the escape at the start of `rest` stands for
   *
   * The escape follows a backslash in a string.
   */
  [[nodiscard]] char unescape(std::string_view rest) const {
    switch (rest[0]) {
    case 'n':
      return '\n';
    case 't':
      return '\t';
    case '\\':
    case '"':
      return rest[0];
    case '\n':
      fail(ErrorCode::SpecString, "unterminated string");
    default:
      fail(ErrorCode::SpecString,
           "unknown escape '\\" + String(first_character(rest)) +
               R"msg(' in string (use \n, \t, \\, or \"))msg");
    }
  }

  /// A string, whose text is its characters
  void string(Token &t) {
    p_++;
    // A string ends on its line: a newline, even after a backslash,
    // leaves it unterminated
    while ((p_ < src_.size()) && (src_[p_] != '"')) {
      if (src_[p_] == '\n')
        fail(ErrorCode::SpecString, "unterminated string");
      if (src_[p_] == '\\') {
        if (p_ + 1 >= src_.size())
          fail(ErrorCode::SpecString, "unterminated string");
        t.text += unescape(src_.substr(p_ + 1));
        p_ += 2;
      } else {
        t.text += src_[p_++];
      }
    }
    if (p_ >= src_.size())
      fail(ErrorCode::SpecString, "unterminated string");
    p_++;
    t.kind = Token::Kind::Str;
  }

  /// An integer or a float
  void number(Token &t) {
    // A minus sign before a number is an operator, and a range such as 0..9
    // is two numbers
    std::size_t q = number_end(src_, p_, false, true);
    const std::size_t range = src_.substr(p_, q - p_).find("..");
    if (range != std::string_view::npos)
      q = p_ + range;
    t.text = src_.substr(p_, q - p_);
    p_ = q;
    const bool is_float =
        t.text.find_first_not_of("0123456789") != String::npos;
    t.kind = is_float ? Token::Kind::Float : Token::Kind::Int;
    if (is_float) {
      const std::optional<double> v = parse_float(t.text);
      if (!v)
        fail(ErrorCode::SpecNumber, "bad number " + t.text);
      t.float_value = *v;
    } else {
      const std::optional<std::int64_t> v = parse_int(t.text);
      if (!v)
        fail(ErrorCode::SpecNumber, "bad number " + t.text);
      t.int_value = *v;
    }
  }

  /// Return where the name starting at position `p` ends
  [[nodiscard]] std::size_t name_end(std::size_t p) const {
    while ((p < src_.size()) && is_alnum(src_[p]))
      p++;
    return p;
  }

  /// Test whether the name from `p` to `q` is followed by a dot and a name
  /// that make it the name of a Gecode constant
  [[nodiscard]] bool at_gecode_name(std::size_t p, std::size_t q) const {
    return (src_.substr(p, q - p) == "gecode") && (q + 1 < src_.size()) &&
           (src_[q] == '.') && is_alnum(src_[q + 1]);
  }

  /// A name, or keyword
  void name(Token &t) {
    std::size_t q = name_end(p_);
    // Gecode constants such as gecode.int_max are single names
    if (at_gecode_name(p_, q))
      q = name_end(q + 1);
    t.text = src_.substr(p_, q - p_);
    p_ = q;
    t.kind = Token::Kind::Ident;
  }

  /// The longest operator that comes next
  void punctuation(Token &t) {
    static constexpr std::array operators = {"..<"sv, ".."sv, "=="sv, "!="sv,
                                             "<="sv,  ">="sv, "&&"sv, "||"sv};
    // NUL is punctuation without a use, as for Gecode, whose strchr matches
    // the terminator of its list
    static constexpr std::string_view singles = "{}[]()=<>+-*/%^!,.:\0"sv;
    const std::string_view rest = src_.substr(p_);
    t.kind = Token::Kind::Punct;
    const auto *op = std::ranges::find_if(
        operators, [rest](std::string_view o) { return rest.starts_with(o); });
    if (op != operators.end())
      t.text = *op;
    else if (singles.contains(rest[0]))
      t.text = rest.substr(0, 1);
    else
      fail(ErrorCode::SpecCharacter,
           "unexpected character '" + String(first_character(rest)) + "'");
    p_ += t.text.size();
  }
};

} // namespace

Seq<Token> lex(std::string_view src, const String &origin, std::size_t line) {
  return Lexer(src, origin, line).run();
}

} // namespace iris::detail
