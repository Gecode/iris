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

#include "parser.hpp"

#include "fail.hpp"
#include "text.hpp"

#include <algorithm>
#include <array>
#include <optional>
#include <string_view>
#include <utility>

namespace iris::detail {

namespace {

/// Return the base type named `t`, or nothing if there is none
std::optional<ReadType> read_type(std::string_view t) {
  if (t == "int")
    return ReadType::Int;
  if (t == "float")
    return ReadType::Float;
  if (t == "num")
    return ReadType::Num;
  if (t == "word")
    return ReadType::Word;
  if (t == "rest")
    return ReadType::Rest;
  return std::nullopt;
}

/// Test whether `s` is a name: a letter or underscore, then letters, digits,
/// and underscores
bool is_name(std::string_view s) {
  return !s.empty() && !is_digit(s[0]) && std::ranges::all_of(s, is_alnum);
}

/// Add the literal text `lit`, if any, as an item to `pat` and clear it
void flush(Pattern &pat, String &lit) {
  if (lit.empty())
    return;
  Pattern::Item it(Pattern::Item::Kind::Lit);
  it.text = std::move(lit);
  pat.items.push_back(std::move(it));
  lit.clear();
}

/// Test whether pattern item `it` ends with a word character
bool ends_word(const Pattern::Item &it) {
  using Kind = Pattern::Item::Kind;
  return (it.kind == Kind::Cap) || (it.kind == Kind::Var) ||
         ((it.kind == Kind::Lit) && is_alnum(it.text.back()));
}

/// Test whether pattern item `it` starts with a word character
bool starts_word(const Pattern::Item &it) {
  using Kind = Pattern::Item::Kind;
  return (it.kind == Kind::Cap) || (it.kind == Kind::Var) ||
         ((it.kind == Kind::Lit) && is_alnum(it.text.front()));
}

/// A line pattern with the text of the type of each capture, which is still
/// to be parsed
struct CompiledPattern {
  Pattern pattern;
  Seq<String> capture_types;
};

/// Compile line pattern `src` on line `line` of the reader spec from
/// `origin`, in the statement or arm on line `owner`
CompiledPattern compile(const String &src, const String &origin,
                        std::size_t line, std::size_t owner) {
  using Item = Pattern::Item;
  CompiledPattern compiled;
  Pattern &pat = compiled.pattern;
  pat.source = src;
  std::size_t p = 0;
  String lit;
  while (p < src.size()) {
    const char c = src[p];
    if (is_blank(c)) {
      flush(pat, lit);
      while ((p < src.size()) && is_blank(src[p]))
        p++;
      pat.items.emplace_back(Item::Kind::Space);
    } else if (c == '{') {
      flush(pat, lit);
      const std::size_t q = src.find('}', p);
      if (q == String::npos)
        fail_in(ErrorCode::SpecPattern, origin, line, owner,
                "unterminated capture in pattern \"" + src + "\"");
      const String cap = src.substr(p + 1, q - p - 1);
      p = q + 1;
      if (cap.starts_with('$')) {
        Item it(Item::Kind::Var);
        it.text = cap.substr(1);
        if (!is_name(it.text))
          fail_in(ErrorCode::SpecPattern, origin, line, owner,
                  "missing or bad name in pattern \"" + src + "\"");
        pat.items.push_back(std::move(it));
        continue;
      }
      Item it(Item::Kind::Cap);
      const std::size_t colon = cap.find(':');
      it.text = cap.substr(0, colon);
      if (!is_name(it.text) && !it.text.empty() && (it.text != "_"))
        fail_in(ErrorCode::SpecPattern, origin, line, owner,
                "capture name '" + it.text + "' in pattern \"" + src +
                    "\" is not a name");
      compiled.capture_types.push_back(
          (colon == String::npos) ? "word" : cap.substr(colon + 1));
      pat.items.push_back(std::move(it));
    } else if (std::string_view(src).substr(p).starts_with("...")) {
      flush(pat, lit);
      if (src.find_first_not_of(" \t", p + 3) != String::npos)
        fail_in(ErrorCode::SpecPattern, origin, line, owner,
                "'...' must end the pattern \"" + src + "\"");
      pat.items.emplace_back(Item::Kind::Any);
      p += 3;
    } else {
      lit += c;
      p++;
    }
  }
  flush(pat, lit);
  // A space is required between two items that would otherwise run
  // together, such as "p cnf" or "{a:int} {b:int}".
  for (std::size_t k = 1; k + 1 < pat.items.size(); k++)
    if (pat.items[k].kind == Item::Kind::Space)
      pat.items[k].required =
          ends_word(pat.items[k - 1]) && starts_word(pat.items[k + 1]);
  return compiled;
}

/// A name bound by a comprehension, at a line of the reader spec
struct BoundName {
  String name;
  std::size_t line;
};

/// A named type, defined by a type statement
struct TypeName {
  String name;
  const Stmt *def;
};

/// A binary operator and its precedence
struct BinaryOp {
  Expr::Op op;
  int precedence;
};

/// Parser for reader specs
class Parser {
public:
  /// Initialize for `tokens` of a reader spec from `origin`
  Parser(Seq<Token> tokens, const String &origin)
      : origin_(origin), toks_(std::move(tokens)) {}

  /// Parse the whole reader spec
  ParsedSpec parse() {
    ParsedSpec s;
    while (peek().kind != Token::Kind::End) {
      owner_line_ = peek().line;
      if (keyword("format")) {
        next();
        if ((peek().kind != Token::Kind::Ident) &&
            (peek().kind != Token::Kind::Str))
          fail(ErrorCode::SpecSyntax,
               "expected a format name, found " + describe(peek()));
        s.name = next().text;
        // Names such as dimacs-cnf: words joined by '-' or '.'
        while (at_name_joiner()) {
          s.name += next().text;
          s.name += next().text;
        }
      } else {
        if (!s.body.empty() && (s.body.back()->kind == Stmt::Kind::Instances))
          fail(ErrorCode::SpecPlacement,
               "'instances' must be the last statement of the spec");
        s.body.push_back(statement());
      }
    }
    check_conflicts(s.body);
    return s;
  }

private:
  /// Where the reader spec comes from
  const String &origin_;
  /// Tokens
  Seq<Token> toks_;
  /// Current position
  std::size_t p_ = 0;
  /// Nesting depth of blocks
  std::size_t depth_ = 0;
  /// Nesting depth of expressions
  std::size_t nesting_ = 0;
  /// Number of repeats parsed
  std::size_t repeats_ = 0;
  /// Whether the next statement is the first of the body of a repeat
  bool repeat_start_ = false;
  /// Whether the statement being parsed may be each
  bool each_ok_ = false;
  /// Line of the innermost statement or sections arm being parsed, which
  /// the errors in it belong to wherever they are found
  std::size_t owner_line_ = 0;
  /// Names bound by the comprehensions being parsed
  Seq<String> bound_;
  /// All names bound by comprehensions
  Seq<BoundName> bound_all_;
  /// The named types visible to the statement being parsed
  Seq<TypeName> types_;
  /// All named types
  Seq<TypeName> types_all_;
  /// Maximal nesting of blocks and expressions
  static constexpr std::size_t max_nesting = 256;

  /// Check that no comprehension binds and no named type is a name of the
  /// spec, as it would hide that name
  void check_conflicts(const Block &body) const {
    Seq<Decl> names;
    declared(body, names);
    const auto is_declared = [&names](const String &n) {
      return std::ranges::any_of(names,
                                 [&n](const Decl &d) { return d.name == n; });
    };
    for (const BoundName &b : bound_all_)
      if (is_declared(b.name))
        fail_at(ErrorCode::SpecNameConflict, origin_, b.line,
                "'" + b.name +
                    "' is bound by a comprehension but is also a name in the "
                    "spec");
    for (const TypeName &t : types_all_)
      if (is_declared(t.name))
        fail_at(ErrorCode::SpecNameConflict, origin_, t.def->line,
                "'" + t.name +
                    "' names a type, so it cannot also name a value");
  }

  [[nodiscard]] const Token &peek() const { return toks_[p_]; }
  const Token &next() {
    const Token &t = toks_[p_];
    if (p_ + 1 < toks_.size())
      p_++;
    return t;
  }
  [[noreturn]] void fail(ErrorCode code, std::string_view m) const {
    fail_on(code, peek().line, m);
  }
  /// Fail with `code` and message `m` on line `line`, in the statement or
  /// arm being parsed
  [[noreturn]] void fail_on(ErrorCode code, std::size_t line,
                            std::string_view m) const {
    fail_in(code, origin_, line, owner_line_, m);
  }
  static String describe(const Token &t) {
    if (t.kind == Token::Kind::End)
      return "end of spec";
    if (t.kind == Token::Kind::Str)
      return "\"" + t.text + "\"";
    return "'" + t.text + "'";
  }
  [[nodiscard]] bool keyword(std::string_view k) const {
    return (peek().kind == Token::Kind::Ident) && (peek().text == k);
  }
  [[nodiscard]] bool punct(std::string_view k) const {
    return (peek().kind == Token::Kind::Punct) && (peek().text == k);
  }
  /// Test whether a '-' or '.' joining the words of a format name such as
  /// dimacs-cnf comes next
  [[nodiscard]] bool at_name_joiner() const {
    if (!(punct("-") || punct(".")) || !peek().glued ||
        (p_ + 1 >= toks_.size()))
      return false;
    const Token &t = toks_[p_ + 1];
    return t.glued &&
           ((t.kind == Token::Kind::Ident) || (t.kind == Token::Kind::Int));
  }
  void expect_punct(std::string_view k) {
    if (!punct(k))
      fail(ErrorCode::SpecSyntax,
           "expected '" + String(k) + "', found " + describe(peek()));
    next();
  }
  void expect_keyword(std::string_view k) {
    if (!keyword(k))
      fail(ErrorCode::SpecSyntax,
           "expected '" + String(k) + "', found " + describe(peek()));
    next();
  }
  String ident(std::string_view what) {
    if (peek().kind != Token::Kind::Ident)
      fail(ErrorCode::SpecSyntax,
           "expected " + String(what) + ", found " + describe(peek()));
    return next().text;
  }
  /// A name that the spec defines, described by `what`
  String new_name(std::string_view what) {
    String n = ident(what);
    if (is_gecode_name(n))
      fail(ErrorCode::SpecName, "'" + n +
                                    "' cannot be defined: names starting "
                                    "with '" +
                                    String(gecode_prefix) +
                                    "' are Gecode constants");
    return n;
  }
  String string(std::string_view what) {
    if (peek().kind != Token::Kind::Str)
      fail(ErrorCode::SpecSyntax, "expected " + String(what) +
                                      " (a string), found " + describe(peek()));
    return next().text;
  }
  Pattern pattern() {
    const std::size_t line = peek().line;
    const String s = string("a line pattern");
    CompiledPattern compiled = compile(s, origin_, line, owner_line_);
    std::size_t k = 0;
    for (Pattern::Item &it : compiled.pattern.items)
      if (it.kind == Pattern::Item::Kind::Cap) {
        it.type = capture_type(compiled.capture_types[k++], line);
        if (it.type.read == ReadType::Symbol)
          fail_on(ErrorCode::SpecPattern, line,
                  "a capture cannot be of an alphabet type");
      }
    return std::move(compiled.pattern);
  }
  /// Return the type `text` of a capture in a pattern on line `line`
  Type capture_type(const String &text, std::size_t line) {
    // Parse the text in place of the tokens of the spec
    Seq<Token> sub = lex(text, origin_, line);
    if (sub.size() == 1)
      fail_on(ErrorCode::SpecPattern, line, "a capture needs a type after ':'");
    std::swap(toks_, sub);
    const std::size_t at = p_;
    p_ = 0;
    Type t = type();
    if (peek().kind != Token::Kind::End)
      fail(ErrorCode::SpecPattern, "unexpected " + describe(peek()) +
                                       " in the capture type '" + text + "'");
    std::swap(toks_, sub);
    p_ = at;
    return t;
  }
  /// Increment nesting counter `n` for `what`, failing beyond the limit
  void enter(std::size_t &n, std::string_view what) {
    if (++n > max_nesting)
      fail(ErrorCode::SpecNesting, String(what) + " are nested too deeply");
  }
  Block block() {
    expect_punct("{");
    enter(depth_, "blocks");
    // The types defined in the block are only visible inside it
    const std::size_t scope = types_.size();
    Block b;
    while (!punct("}")) {
      if (peek().kind == Token::Kind::End)
        fail(ErrorCode::SpecSyntax, "missing '}'");
      b.push_back(statement());
    }
    next();
    while (types_.size() > scope)
      types_.pop_back();
    depth_--;
    return b;
  }
  static Node<Expr> integer(std::int64_t v) {
    auto e = make_node<Expr>(Expr::Kind::Int);
    e->int_value = v;
    return e;
  }

  /// Consume keyword `k` if it comes next, and return whether it did
  bool accept(std::string_view k) {
    if (!keyword(k))
      return false;
    next();
    return true;
  }

  /*
   * Statements: one function per keyword, called after the keyword with a
   * new statement of the keyword's kind
   */

  /// A function completing statement `s`
  using Parse = void (Parser::*)(Stmt &s);

  /// A statement that starts with a keyword
  struct Keyword {
    /// The keyword
    std::string_view keyword;
    /// Kind of statement
    Stmt::Kind kind;
    /// Function parsing the rest of the statement
    Parse parse;
    /// Whether the statement is a setting, which changes how the whole input
    /// is read, so it is at the top level, where it runs once
    bool setting;
  };

  /// Return the statement that starts with keyword `k`, or null
  static const Keyword *statement_keyword(std::string_view k) {
    using K = Stmt::Kind;
    static constexpr std::array<Keyword, 20> statements = {{
        {"comment", K::Comment, &Parser::words, true},
        {"stop", K::Stop, &Parser::words, true},
        {"expect", K::Expect, &Parser::words, false},
        {"tokens", K::Tokens, &Parser::chars, true},
        {"separators", K::Separators, &Parser::chars, true},
        {"ignore", K::Trailing, &Parser::trailing, true},
        {"param", K::Param, &Parser::param, false},
        {"list", K::List, &Parser::list, false},
        {"line", K::Line, &Parser::line_pattern, false},
        {"find", K::Find, &Parser::line_pattern, false},
        {"skip", K::Skip, &Parser::skip, false},
        {"index", K::Index, &Parser::index, false},
        {"repeat", K::Repeat, &Parser::repeat, false},
        {"each", K::Each, &Parser::each, false},
        {"instances", K::Instances, &Parser::instances, false},
        {"sections", K::Sections, &Parser::sections, false},
        {"if", K::If, &Parser::conditional, false},
        {"let", K::Let, &Parser::let, false},
        {"check", K::Check, &Parser::check, false},
        {"type", K::Type, &Parser::type_def, false},
    }};
    const auto e = std::ranges::find(statements, k, &Keyword::keyword);
    return (e == statements.end()) ? nullptr : &*e;
  }

  /**
   * Test whether a declaration starts at token `k`
   *
   * A declaration is a name, any dimensions, and a colon, as in
   * `d[n][n]: int`. Its name can be a keyword.
   */
  [[nodiscard]] bool at_declaration(std::size_t k) const {
    if (toks_[k].kind != Token::Kind::Ident)
      return false;
    k++;
    while ((toks_[k].kind == Token::Kind::Punct) && (toks_[k].text == "[")) {
      int open = 0;
      do {
        if (toks_[k].kind == Token::Kind::End)
          return false;
        if (toks_[k].kind == Token::Kind::Punct) {
          if (toks_[k].text == "[")
            open++;
          else if (toks_[k].text == "]")
            open--;
        }
        k++;
      } while (open > 0);
    }
    return (toks_[k].kind == Token::Kind::Punct) && (toks_[k].text == ":");
  }

  /// Parse a statement, which owns the errors found while parsing it
  Node<Stmt> statement() {
    const std::size_t outer = std::exchange(owner_line_, peek().line);
    Node<Stmt> s = statement_contents();
    owner_line_ = outer;
    return s;
  }
  Node<Stmt> statement_contents() {
    const Token &t = peek();
    each_ok_ = repeat_start_;
    repeat_start_ = false;
    if (t.kind != Token::Kind::Ident)
      fail(ErrorCode::SpecSyntax, "expected a statement, found " + describe(t));
    if (at_declaration(p_)) {
      auto s = make_node<Stmt>(Stmt::Kind::Read, t.line);
      declaration(*s);
      return s;
    }
    const Keyword *e = statement_keyword(t.text);
    if (e == nullptr)
      fail(ErrorCode::SpecSyntax, "unknown statement '" + t.text + "'");
    if (e->setting && (depth_ > 0))
      fail(ErrorCode::SpecPlacement,
           "the setting '" + t.text + "' must be at the top level of the spec");
    auto s = make_node<Stmt>(e->kind, t.line);
    next();
    (this->*e->parse)(*s);
    return s;
  }

  /// comment, stop, and expect: one or more strings
  void words(Stmt &s) {
    s.words.push_back(string(s.kind == Stmt::Kind::Expect ? "a literal token"
                                                          : "a line prefix"));
    while (peek().kind == Token::Kind::Str)
      s.words.push_back(next().text);
  }
  /// tokens and separators: a string of characters
  void chars(Stmt &s) {
    s.chars = string(s.kind == Stmt::Kind::Tokens ? "single-character tokens"
                                                  : "separator characters");
  }
  /// ignore trailing
  void trailing(Stmt & /*s*/) { expect_keyword("trailing"); }
  /// A name for a value that is kept, so not _
  String defined(std::string_view what) {
    String n = new_name(what);
    if (n == "_")
      fail(ErrorCode::SpecName, "'_' cannot be defined here");
    return n;
  }
  /// param x [= default]
  void param(Stmt &s) {
    s.name = defined("a parameter name");
    if (punct("=")) {
      next();
      s.expr = expr();
    }
  }
  /**
   * Consume option `k` of a statement if it comes next, and return whether
   * it did
   *
   * A declaration of a value with the same name ends the statement.
   */
  bool option(std::string_view k) { return !at_declaration(p_) && accept(k); }
  /// Test whether keyword `k` can end a range without an upper bound
  static bool ends_range(std::string_view k) {
    return (statement_keyword(k) != nullptr) || (k == "layout") ||
           (k == "offset") || (k == "until");
  }
  /// Test whether the upper bound of a range comes next
  [[nodiscard]] bool at_upper_bound() const {
    const Token &t = peek();
    switch (t.kind) {
    case Token::Kind::Ident:
      return !ends_range(t.text) && !at_declaration(p_);
    case Token::Kind::Punct:
      return punct("(") || punct("-") || punct("!");
    case Token::Kind::End:
      return false;
    case Token::Kind::Int:
    case Token::Kind::Float:
    case Token::Kind::Str:
      return true;
    }
    std::unreachable();
  }
  /// Return the type statement of the named type `n`, or null
  [[nodiscard]] const Stmt *named_type(std::string_view n) const {
    const auto t =
        std::ranges::find(types_.rbegin(), types_.rend(), n, &TypeName::name);
    return (t == types_.rend()) ? nullptr : t->def;
  }
  /// Test whether `n` is the name of a base type
  static bool is_base_type(std::string_view n) {
    return read_type(n).has_value() || (n == "alphabet");
  }
  /**
   * A type
   *
   * A base type, a named type, an alphabet type, or a range lo..hi or
   * lo..<hi, where one of the bounds can be left out (but not the upper
   * bound of lo..<hi).
   */
  Type type() {
    Type t;
    if (peek().kind == Token::Kind::Ident) {
      const String &n = peek().text;
      if (const std::optional<ReadType> r = read_type(n)) {
        next();
        t.read = *r;
        return t;
      }
      if (n == "alphabet") {
        next();
        alphabet(t);
        return t;
      }
      if (const Stmt *d = named_type(n)) {
        next();
        Type u = d->type.use(d);
        u.name = d->name;
        return u;
      }
    }
    if (!punct("..") && !punct("..<")) {
      Node<Expr> lo = expr();
      if (!punct("..") && !punct("..<")) {
        if (lo->kind == Expr::Kind::Name)
          fail(ErrorCode::SpecUnknownType, "unknown type '" + lo->s + "'");
        fail(ErrorCode::SpecSyntax,
             "expected '..' or '..<' after the lower bound of a range, found " +
                 describe(peek()));
      }
      t.lo = std::move(lo);
    }
    t.exclusive = punct("..<");
    next();
    if (at_upper_bound())
      t.hi = expr();
    if (t.exclusive && !t.hi)
      fail(ErrorCode::SpecSyntax, "'..<' needs an upper bound");
    if (!t.lo && !t.hi)
      fail(ErrorCode::SpecSyntax, "a range needs a lower or an upper bound");
    return t;
  }
  /// alphabet "..." [blank "..."] [strip "..."] [base n], after alphabet
  void alphabet(Type &t) {
    t.read = ReadType::Symbol;
    t.alphabet = string("an alphabet");
    for (;;) {
      if (option("blank")) {
        t.blank = string("blank symbols");
      } else if (option("strip")) {
        t.strip = string("symbols to strip");
      } else if (option("base")) {
        const bool negative = punct("-");
        if (negative)
          next();
        if (peek().kind != Token::Kind::Int)
          fail(ErrorCode::SpecSyntax,
               "expected an integer, found " + describe(peek()));
        const std::int64_t base = next().int_value;
        t.base = negative ? -base : base;
      } else {
        break;
      }
    }
  }
  /// Test whether type `t` reads numbers
  static bool numeric(const Type &t) {
    return (t.read == ReadType::Int) || (t.read == ReadType::Float) ||
           (t.read == ReadType::Num);
  }
  /// Optional offset of a declaration or list statement
  void offset(Stmt &s) {
    if (!option("offset"))
      return;
    if (!numeric(s.type))
      fail(ErrorCode::SpecType, "'offset' needs numbers");
    s.offset = expr();
  }
  /// x[d1][d2]...: type [layout e] [offset e]
  void declaration(Stmt &s) {
    s.name = new_name("a name");
    while (punct("[")) {
      next();
      s.dims.push_back(expr());
      expect_punct("]");
    }
    expect_punct(":");
    s.type = type();
    if ((s.type.read == ReadType::Rest) && !s.dims.empty())
      fail(ErrorCode::SpecType, "'rest' values cannot be arrays");
    if ((s.type.read == ReadType::Symbol) && (s.dims.size() != 2))
      fail(ErrorCode::SpecType,
           "an alphabet type needs a two-dimensional array");
    if (option("layout")) {
      if ((s.dims.size() != 2) || !numeric(s.type))
        fail(ErrorCode::SpecType,
             "'layout' needs a two-dimensional array of numbers");
      s.expr = expr();
    }
    offset(s);
  }
  /// list x: type until (e | eol) [offset e]
  void list(Stmt &s) {
    s.name = new_name("a name");
    expect_punct(":");
    s.type = type();
    if ((s.type.read == ReadType::Rest) || (s.type.read == ReadType::Symbol))
      fail(ErrorCode::SpecType,
           "a list cannot be of 'rest' or of an alphabet type");
    expect_keyword("until");
    if (accept("eol"))
      s.until_eol = true;
    else
      s.sentinel = expr();
    offset(s);
  }
  /// type t = type
  void type_def(Stmt &s) {
    s.name = defined("a type name");
    if (is_base_type(s.name))
      fail(ErrorCode::SpecNameConflict, "'" + s.name + "' is a base type");
    if (named_type(s.name) != nullptr)
      fail(ErrorCode::SpecNameConflict,
           "the type '" + s.name + "' is already defined");
    expect_punct("=");
    s.type = type();
    const TypeName t{.name = s.name, .def = &s};
    types_.push_back(t);
    types_all_.push_back(t);
  }
  /// line and find: a pattern
  void line_pattern(Stmt &s) { s.pattern = pattern(); }
  /// skip line, or skip e lines
  void skip(Stmt &s) {
    if (accept("line")) {
      s.expr = integer(1);
    } else {
      s.expr = expr();
      expect_keyword("lines");
    }
  }
  /// index e
  void index(Stmt &s) { s.expr = expr(); }
  /// Terminating tokens: "a" "b" ... [store x]
  void until(Stmt &s) {
    s.has_until = true;
    s.words.push_back(string("a terminating token"));
    while (peek().kind == Token::Kind::Str)
      s.words.push_back(next().text);
    if (accept("store"))
      s.store = new_name("a name");
  }
  /// repeat (e | until eof | until tokens) [as name] { ... }
  void repeat(Stmt &s) {
    s.id = repeats_++;
    if (accept("until")) {
      if (accept("eof"))
        s.until_eof = true;
      else
        until(s);
    } else {
      s.expr = expr();
    }
    if (accept("as"))
      s.list = new_name("a name for the records");
    repeat_start_ = true;
    s.body = block();
    repeat_start_ = false;
    if (!s.body.empty() && (s.body[0]->kind == Stmt::Kind::Each))
      s.each = s.body[0]->name;
  }
  /// each x, first in the body of a repeat
  void each(Stmt &s) {
    if (!each_ok_)
      fail(ErrorCode::SpecPlacement,
           "'each' must be the first statement of a repeat, and comes at most "
           "once");
    s.name = new_name("a name for each record");
  }
  /// instances { ... }, at the top level only
  void instances(Stmt &s) {
    if (depth_ > 0)
      fail(ErrorCode::SpecPlacement,
           "'instances' must be at the top level of the spec");
    s.body = block();
  }
  /// sections [until tokens] [skip other] { "pattern" [optional] [last]
  /// [{...}] ... }
  void sections(Stmt &s) {
    for (;;) {
      if (accept("until")) {
        until(s);
      } else if (accept("skip")) {
        expect_keyword("other");
        s.skip_other = true;
      } else {
        break;
      }
    }
    expect_punct("{");
    while (!punct("}")) {
      if (peek().kind == Token::Kind::End)
        fail(ErrorCode::SpecSyntax, "missing '}'");
      // The arm owns the errors found while parsing it
      Arm a;
      a.line = peek().line;
      const std::size_t outer = std::exchange(owner_line_, a.line);
      a.pattern = pattern();
      a.optional = accept("optional");
      a.last = accept("last");
      if (punct("{"))
        a.body = block();
      owner_line_ = outer;
      s.arms.push_back(std::move(a));
    }
    next();
  }
  /// if e { ... } [else (if ... | { ... })]
  void conditional(Stmt &s) {
    s.expr = expr();
    s.body = block();
    if (!accept("else"))
      return;
    if (keyword("if")) {
      // A chain of else if nests like blocks
      enter(depth_, "blocks");
      s.orelse.push_back(statement());
      depth_--;
    } else {
      s.orelse = block();
    }
  }
  /// let x = e
  void let(Stmt &s) {
    s.name = defined("a name");
    expect_punct("=");
    s.expr = expr();
  }
  /// check e ["message"]
  void check(Stmt &s) {
    s.expr = expr();
    if (peek().kind == Token::Kind::Str)
      s.message = next().text;
  }

  /*
   * Expressions, by precedence climbing
   */

  Node<Expr> expr() { return binary(0); }
  /// Return the binary operator `t` with its precedence, or nothing if `t`
  /// is none
  static std::optional<BinaryOp> binary_op(const Token &t) {
    struct Entry {
      std::string_view text;
      BinaryOp op;
    };
    using O = Expr::Op;
    static constexpr std::array<Entry, 13> ops = {{
        {"||", {O::Or, 1}},
        {"&&", {O::And, 2}},
        {"==", {O::Eq, 3}},
        {"!=", {O::Ne, 3}},
        {"<", {O::Lt, 4}},
        {">", {O::Gt, 4}},
        {"<=", {O::Le, 4}},
        {">=", {O::Ge, 4}},
        {"+", {O::Add, 5}},
        {"-", {O::Sub, 5}},
        {"*", {O::Mul, 6}},
        {"/", {O::Div, 6}},
        {"%", {O::Mod, 6}},
    }};
    if (t.kind != Token::Kind::Punct)
      return std::nullopt;
    const auto o = std::ranges::find(ops, t.text, &Entry::text);
    if (o == ops.end())
      return std::nullopt;
    return o->op;
  }
  /// Binary operators of at least precedence `min`, left-associative
  Node<Expr> binary(int min) {
    Node<Expr> lhs = unary();
    // Each operator adds a level above lhs
    std::size_t levels = 0;
    for (;;) {
      const std::optional<BinaryOp> op = binary_op(peek());
      if (!op || (op->precedence < min))
        break;
      enter(nesting_, "expressions");
      levels++;
      auto e = make_node<Expr>(Expr::Kind::Binary);
      e->op = op->op;
      e->s = next().text;
      e->args.push_back(std::move(lhs));
      e->args.push_back(binary(op->precedence + 1));
      lhs = std::move(e);
    }
    nesting_ -= levels;
    return lhs;
  }
  Node<Expr> unary() {
    enter(nesting_, "expressions");
    Node<Expr> e;
    if (punct("-") || punct("!")) {
      e = make_node<Expr>(Expr::Kind::Unary);
      e->op = punct("-") ? Expr::Op::Neg : Expr::Op::Not;
      e->s = next().text;
      e->args.push_back(unary());
    } else {
      e = power();
    }
    nesting_--;
    return e;
  }
  /// a ^ b, which binds tighter than unary operators and groups to the
  /// right; b is a unary expression, so 2^-1 is 2^(-1)
  Node<Expr> power() {
    Node<Expr> e = postfix();
    if (!punct("^"))
      return e;
    auto x = make_node<Expr>(Expr::Kind::Binary);
    x->op = Expr::Op::Pow;
    x->s = next().text;
    x->args.push_back(std::move(e));
    x->args.push_back(unary());
    return x;
  }
  /// Indices and fields a[i][j].f...
  Node<Expr> postfix() {
    Node<Expr> e = primary();
    // Each index or field adds a level above e
    std::size_t levels = 0;
    while (punct("[") || punct(".")) {
      const bool field = punct(".");
      next();
      enter(nesting_, "expressions");
      levels++;
      auto x = make_node<Expr>(field ? Expr::Kind::Field : Expr::Kind::Index);
      x->args.push_back(std::move(e));
      if (field) {
        x->s = ident("a field name");
      } else {
        x->args.push_back(expr());
        expect_punct("]");
      }
      e = std::move(x);
    }
    nesting_ -= levels;
    return e;
  }

  /// Check that call `e` on reader-spec line `line` names a function with
  /// suitable arguments
  void check_call(const Expr &e, std::size_t line) const {
    const String &f = e.s;
    const std::size_t n = e.args.size();
    bool ok = false;
    if (f == "defined")
      ok = (n == 1) && (e.args[0]->kind == Expr::Kind::Name);
    else if ((f == "count") || (f == "size"))
      ok = (n == 1);
    else if ((f == "min") || (f == "max") || (f == "sum") || (f == "distinct"))
      ok = (n >= 1);
    else if ((f == "all") || (f == "any"))
      fail_on(ErrorCode::SpecFunction, line,
              "'" + f + "' needs the form " + f + "(x in a: e)");
    else
      fail_on(ErrorCode::SpecFunction, line, "unknown function '" + f + "'");
    if (!ok)
      fail_on(ErrorCode::SpecFunction, line,
              "wrong arguments for function '" + f + "'");
  }
  /// Return the value of the Gecode constant `n` on reader-spec line `line`
  [[nodiscard]] std::int64_t gecode_constant(const String &n,
                                             std::size_t line) const {
    struct Constant {
      std::string_view name;
      std::int64_t value;
    };
    // Gecode's Int::Limits::max and min, fixed in Iris 0.1
    static constexpr std::array<Constant, 2> constants = {{
        {"gecode.int_max", 2147483646},
        {"gecode.int_min", -2147483646},
    }};
    String known;
    for (const Constant &c : constants) {
      if (n == c.name)
        return c.value;
      known += (known.empty() ? "" : ", ") + String(c.name);
    }
    fail_on(ErrorCode::SpecConstant, line,
            "unknown Gecode constant '" + n + "' (known: " + known + ")");
  }
  /// Test whether a comprehension for function `f` comes next, after "f("
  [[nodiscard]] bool at_comprehension(std::string_view f) const {
    if ((f != "all") && (f != "any") && (f != "sum") && (f != "count"))
      return false;
    if ((peek().kind != Token::Kind::Ident) || (p_ + 1 >= toks_.size()))
      return false;
    const Token &t = toks_[p_ + 1];
    return (t.kind == Token::Kind::Ident) && (t.text == "in");
  }
  /// f(x in a: e), after "f("
  Node<Expr> comprehension(const String &f) {
    auto e = make_node<Expr>(Expr::Kind::Comprehension);
    e->s = f;
    BoundName b{.name = {}, .line = peek().line};
    b.name = new_name("a name");
    if (b.name == "_")
      fail(ErrorCode::SpecName, "'_' cannot be bound");
    if (std::ranges::contains(bound_, b.name))
      fail(ErrorCode::SpecNameConflict,
           "'" + b.name + "' is already bound by an enclosing comprehension");
    e->var = b.name;
    expect_keyword("in");
    // The name is bound in the body only
    e->args.push_back(expr());
    expect_punct(":");
    bound_.push_back(b.name);
    e->args.push_back(expr());
    bound_.pop_back();
    expect_punct(")");
    bound_all_.push_back(std::move(b));
    return e;
  }
  Node<Expr> primary() {
    const Token &t = peek();
    if (t.kind == Token::Kind::Int)
      return integer(next().int_value);
    if (t.kind == Token::Kind::Float) {
      auto e = make_node<Expr>(Expr::Kind::Float);
      e->float_value = next().float_value;
      return e;
    }
    if (t.kind == Token::Kind::Str) {
      auto e = make_node<Expr>(Expr::Kind::Str);
      e->s = next().text;
      return e;
    }
    if (punct("(")) {
      next();
      Node<Expr> e = expr();
      expect_punct(")");
      return e;
    }
    if ((t.kind == Token::Kind::Ident) && is_gecode_name(t.text)) {
      const std::size_t line = t.line;
      return integer(gecode_constant(next().text, line));
    }
    if (t.kind == Token::Kind::Ident) {
      const std::size_t line = t.line;
      String n = next().text;
      if (punct("(")) {
        next();
        if (at_comprehension(n))
          return comprehension(n);
        auto e = make_node<Expr>(Expr::Kind::Call);
        e->s = std::move(n);
        if (!punct(")")) {
          e->args.push_back(expr());
          while (punct(",")) {
            next();
            e->args.push_back(expr());
          }
        }
        expect_punct(")");
        check_call(*e, line);
        return e;
      }
      auto e = make_node<Expr>(Expr::Kind::Name);
      e->s = std::move(n);
      return e;
    }
    fail(ErrorCode::SpecSyntax, "expected an expression, found " + describe(t));
  }
};

/// Return the type of the values that `t` reads
ValueType value_type(ReadType t) {
  switch (t) {
  case ReadType::Float:
    return ValueType::Float;
  case ReadType::Word:
  case ReadType::Rest:
    return ValueType::String;
  case ReadType::Int:
  case ReadType::Num:
  case ReadType::Symbol:
    return ValueType::Int;
  }
  std::unreachable();
}

/// Add name `n` with type `t` and depth `d` to `out`
void declare(Seq<Decl> &out, const String &n, ValueType t, std::size_t d) {
  if (n.empty() || (n == "_"))
    return;
  if (std::ranges::contains(out, n, &Decl::name))
    return;
  out.push_back(Decl{.name = n, .type = t, .depth = d});
}

/// Add the captures of `p` to `out`
void declare(Seq<Decl> &out, const Pattern &p) {
  for (const Pattern::Item &it : p.items)
    if (it.kind == Pattern::Item::Kind::Cap)
      declare(out, it.text, value_type(it.type.read), 0);
}

} // namespace

void declared(const Block &b, Seq<Decl> &out) {
  for (const Node<Stmt> &s : b) {
    switch (s->kind) {
    case Stmt::Kind::Read:
      declare(out, s->name, value_type(s->type.read), s->dims.size());
      break;
    case Stmt::Kind::List:
      declare(out, s->name, value_type(s->type.read), 1);
      break;
    case Stmt::Kind::Let:
    case Stmt::Kind::Param:
      // Usually a single value; the type is only known when running
      declare(out, s->name, ValueType::Int, 0);
      break;
    case Stmt::Kind::Line:
    case Stmt::Kind::Find:
      declare(out, s->pattern);
      break;
    case Stmt::Kind::Repeat: {
      // Values of a nested repeat are one level deeper
      declare(out, s->store, ValueType::String, 0);
      declare(out, record_starts(s->id), ValueType::Int, 1);
      Seq<Decl> inner;
      declared(s->body, inner);
      for (const Decl &d : inner)
        declare(out, d.name, d.type, d.depth + 1);
      break;
    }
    case Stmt::Kind::Sections:
      declare(out, s->store, ValueType::String, 0);
      for (const Arm &a : s->arms) {
        declare(out, a.pattern);
        declared(a.body, out);
      }
      break;
    case Stmt::Kind::If:
      declared(s->body, out);
      declared(s->orelse, out);
      break;
    case Stmt::Kind::Instances:
      declared(s->body, out);
      break;
    default:
      break;
    }
  }
}

ParsedSpec parse_tokens(Seq<Token> tokens, const String &origin) {
  return Parser(std::move(tokens), origin).parse();
}

} // namespace iris::detail
