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

/*
 * The syntax tree of a reader spec, shared by the implementation files.
 * Not part of the public interface.
 */

#pragma once

#include "call_sites.hpp"
#include "memory.hpp"

#include <iris/data.hpp>

#include <cstddef>
#include <cstdint>
#include <memory>
#include <optional>
#include <string>
#include <utility>
#include <vector>

namespace iris::detail {

using iris::ValueType;

struct Stmt;

/**
 * Expression
 *
 * An index expression `a[i]` has the operands `a` and `i`, a field `r.f`
 * the operand `r` and the name `f` in `s`. A comprehension such as
 * `all(x in a: e)` has the function in `s`, the bound name in `var`, and
 * the operands `a` and `e`.
 *
 * The parser finds the expressions that are records or lists of records of
 * a repeat (see Stmt::list and Stmt::each): `records` is the repeat.
 */
struct Expr {
  /// Kind of expression
  enum class Kind {
    Int,
    Float,
    Str,
    Name,
    Binary,
    Unary,
    Call,
    Index,
    Field,
    Comprehension
  };
  /// Operator of unary and binary expressions
  enum class Op {
    Or,
    And,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Neg,
    Not
  };
  Kind kind;
  Op op = Op::Or;
  /// Integer literal
  std::int64_t int_value = 0;
  /// Float literal
  double float_value = 0.0;
  /// String literal, name, operator, or function
  String s;
  /// Name bound by a comprehension
  String var;
  /// Index of a record name among the records of its check, if it is one
  std::optional<std::size_t> record;
  /// The repeat whose records or lists of records this is, or null
  const Stmt *records = nullptr;
  /// Whether this is a list of records of `records`, rather than a record
  bool record_list = false;
  /// For a comprehension, the repeat whose records are the elements of its
  /// array, or null
  const Stmt *elements = nullptr;
  /// Operands or arguments
  Seq<Node<Expr>> args;
  /// Initialize with kind `k`
  explicit Expr(Kind k) : kind(k) {}
};

/**
 * A record name of a check
 *
 * A name defined inside repeats that do not enclose the check, or the name
 * of each record of such a repeat (see Stmt::each), so that the check holds
 * for each of its records.
 */
struct Record {
  /// The name of the values
  String name;
  /// The name in messages: the name, or the list of records
  String label;
  /// The repeats between the check and the definition, outermost first
  Seq<const Stmt *> repeats;
  /// Name of the record starts of the innermost of these repeats
  String starts;
};

/**
 * Return the name of the record starts of the repeat numbered `id`
 *
 * A repeat stores where each of its records starts in the input under this
 * name, which is not a spec name, so that a failed check can point at a
 * record. Such names are not part of the data read.
 */
inline String record_starts(std::size_t id) { return "#" + to_string(id); }
/// Test whether `n` is a name used internally (see record_starts)
inline bool is_internal(const String &n) { return !n.empty() && (n[0] == '#'); }

/// What a declaration or a capture reads
enum class ReadType {
  Int,   ///< An integer
  Float, ///< A float
  Num,   ///< An integer if possible, otherwise a float
  Word,  ///< A token as a string
  Rest,  ///< The rest of the line as a string
  Symbol ///< A character of an alphabet, as an integer
};

/**
 * Type of the values that a declaration or a capture reads
 *
 * An integer type can be a range `lo..hi`, or `lo..<hi` without `hi`, where
 * one of the bounds can be left out. An alphabet type reads one character
 * for each value. A use of a named type (see Stmt::Kind::Type) copies its
 * definition except for the bounds, which are evaluated where it is
 * defined.
 */
struct Type {
  /// What the type reads
  ReadType read = ReadType::Int;
  /// Lower bound of a range, or null
  Node<Expr> lo;
  /// Upper bound of a range, or null
  Node<Expr> hi;
  /// Whether the upper bound is excluded
  bool exclusive = false;
  /// The type statement defining a named type, or null
  const Stmt *named = nullptr;
  /// Name of a named type
  String name;
  /// Symbols of an alphabet type
  String alphabet;
  /// Symbols that stand for 0
  String blank;
  /// Symbols to skip
  String strip;
  /// Value of the first symbol
  std::int64_t base = 1;
  /// Test whether the type has a range, possibly through its name
  [[nodiscard]] bool ranged() const {
    return (lo != nullptr) || (hi != nullptr) || (named != nullptr);
  }
  /// Return a use of the type defined by type statement `d`
  [[nodiscard]] Type use(const Stmt *d) const {
    Type t;
    t.read = read;
    t.named = d;
    t.alphabet = alphabet;
    t.blank = blank;
    t.strip = strip;
    t.base = base;
    return t;
  }
};

/// Line pattern such as "DIMENSION : {n:int}"
struct Pattern {
  /// Pattern item
  struct Item {
    /// Kind of item
    enum class Kind { Lit, Space, Cap, Any, Var };
    Kind kind;
    /// Literal text, capture name, or variable name
    String text;
    /// What a capture reads
    Type type;
    /// Whether a space item needs at least one blank
    bool required = false;
    /// Initialize with kind `k`
    explicit Item(Kind k) : kind(k) {}
  };
  /// Source text
  String source;
  /// Items
  Seq<Item> items;
};

/// Sequence of statements
using Block = Seq<Node<Stmt>>;

/// Arm of a sections statement
struct Arm {
  /// Line pattern
  Pattern pattern;
  /// Reader-spec line of the pattern, where the arm starts
  std::size_t line = 0;
  /// Statements run after the line matched
  Block body;
  /// Whether the input may lack a line matching the arm
  bool optional = false;
  /// Whether the sections statement ends after this arm
  bool last = false;
};

/// Statement
struct Stmt {
  /// Kind of statement
  enum class Kind {
    Comment,
    Stop,
    Tokens,
    Separators,
    Trailing,
    Param,
    Read,
    List,
    Line,
    Find,
    Skip,
    Expect,
    Index,
    Repeat,
    Sections,
    If,
    Let,
    Check,
    Type,
    Instances,
    Each
  };
  Kind kind;
  /// Line in the reader spec
  std::size_t line;
  /// What read and list statements read, or the type a type statement
  /// defines
  Type type;
  /// Name defined
  String name;
  /// Characters for tokens and separators statements
  String chars;
  /// Message of a check statement
  String message;
  /// Name for the terminating token found
  String store;
  /// Name of the list of records of a repeat
  String list;
  /// Name of each record of a repeat, inside and after it
  String each;
  /// Array dimensions
  Seq<Node<Expr>> dims;
  /// Count, condition, layout, value, or default
  Node<Expr> expr;
  /// List terminator
  Node<Expr> sentinel;
  /// Added to every number read
  Node<Expr> offset;
  /// Prefixes, literal tokens, or terminating tokens
  Seq<String> words;
  /// List ends at end of line
  bool until_eol = false;
  /// Repeat until end of input
  bool until_eof = false;
  /// Repeat or sections end at a token from `words`
  bool has_until = false;
  /// Sections skip unknown lines
  bool skip_other = false;
  /// Pattern for line and find
  Pattern pattern;
  /// Body, then-branch
  Block body;
  /// Else-branch
  Block orelse;
  /// Arms of sections
  Seq<Arm> arms;
  /// Number of a repeat, in the order of the spec
  std::size_t id = 0;
  /// Record names of a check
  Seq<Record> records;
  /// Initialize with kind `k` at reader-spec line `l`
  Stmt(Kind k, std::size_t l) : kind(k), line(l) {}
};

/// A name that a block may define, with the type and depth of its values
struct Decl {
  /// Name
  String name;
  /// Type of the leaves
  ValueType type;
  /// Depth
  std::size_t depth;
};
/// Add the names that block `b` may define to `out`
void declared(const Block &b, Seq<Decl> &out);

/**
 * Resolve the names in the expressions of `b`, the body of a reader spec
 * from `origin`
 *
 * Finds the expressions that are records or lists of records of a named
 * repeat (see Expr) and the record names of the checks (see Record), and
 * reports their misuse.
 */
void resolve(Block &b, const String &origin);

/// Implementation of Spec
struct SpecImpl {
  /// The resource that every operation with the spec allocates from
  std::pmr::memory_resource *memory;
  /// Where the reader spec comes from
  String origin;
  /// Format name
  String name;
  /// Statements
  Block body;
  /// The call site of each line, for a spec from a spec builder
  CallSites call_sites;
};

} // namespace iris::detail
