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
 * The values a read produces, shared by the implementation files. Not part
 * of the public interface.
 */

#pragma once

#include "ast.hpp"

#include <iris/error.hpp>

#include <cstddef>
#include <cstdint>
#include <optional>
#include <span>
#include <string>
#include <string_view>
#include <utility>

namespace iris::detail {

/// Error about the shape or type of a value, located by the caller
struct ShapeError {
  ErrorCode code;
  /// Message
  String msg;
};

/**
 * A value: scalar or nested (possibly ragged) array
 *
 * The leaves are stored in a flat array of the value's type. The structure
 * of a value of depth d is given by d levels of cumulative offsets: level 0
 * is {0, n} for the outermost list of n elements, and level k has one offset
 * more than there are lists at depth k, so that list j of level k holds the
 * lists or leaves levels[k][j] to levels[k][j+1] of the level below. The
 * last offset of the last level is the number of leaves.
 */
class Value {
public:
  /// Type of the leaves
  ValueType type;
  /// Offsets per level; there are as many levels as the depth
  Seq<Seq<std::size_t>> levels;
  /// Integer leaves
  Seq<std::int64_t> ints;
  /// Float leaves
  Seq<double> floats;
  /// String leaves
  Seq<String> strings;

  /// The empty integer list
  ///
  /// The level is built in place, as an initializer list would allocate a
  /// copy through the allocator, which compilers cannot elide.
  Value() : type(ValueType::Int) { levels.emplace_back(2, 0); }
  /// Scalar of type `t` whose leaf is still to be added
  explicit Value(ValueType t) : type(t) {}
  /// Integer scalar
  static Value integer(std::int64_t v) {
    Value x(ValueType::Int);
    x.ints.push_back(v);
    return x;
  }
  /// Float scalar
  static Value real(double v) {
    Value x(ValueType::Float);
    x.floats.push_back(v);
    return x;
  }
  /// String scalar
  static Value string(std::string_view v) {
    Value x(ValueType::String);
    x.strings.emplace_back(v);
    return x;
  }

  /// Return the nesting depth (0 for scalars)
  [[nodiscard]] std::size_t depth() const { return levels.size(); }
  /// Test whether numeric scalar
  [[nodiscard]] bool is_number() const {
    return levels.empty() && (type != ValueType::String);
  }
  /// Return number of leaves
  [[nodiscard]] std::size_t leaves() const {
    switch (type) {
    case ValueType::Int:
      return ints.size();
    case ValueType::Float:
      return floats.size();
    case ValueType::String:
      return strings.size();
    }
    std::unreachable();
  }
  /// Return length of outermost list
  [[nodiscard]] std::size_t size() const {
    return levels.empty() ? 0 : levels[0][1];
  }
  /// Return the length of list `pos` at level `level`
  [[nodiscard]] std::size_t list_size(std::size_t level,
                                      std::size_t pos) const {
    return levels[level][pos + 1] - levels[level][pos];
  }
  /// Return value of numeric scalar or leaf `i` as a double
  [[nodiscard]] double as_double(std::size_t i = 0) const {
    return type == ValueType::Int ? static_cast<double>(ints[i]) : floats[i];
  }
  /// Convert integer leaves to floats
  void to_float();
  /// Append the value of scalar `s`, adapting the type
  void push(const Value &s);
  /// Make the outermost list hold all leaves, after pushing them
  void end_list() { levels[0][1] = leaves(); }
  /// Make this the empty list of type `t`, keeping the memory for reuse
  void clear_list(ValueType t);
  /**
   * Make this an array with the dimensions `dims` around its leaves, which
   * must number their product
   *
   * The structure grows row by row, so that it never takes more memory
   * than the rows built so far.
   */
  void shape(const Seq<std::size_t> &dims);

  /// A list or leaf of a value: list `pos` at level `level`, or leaf `pos`
  /// if the level is the depth
  struct Position {
    std::size_t level;
    std::size_t pos;
  };
  /// Find the part that the indices `path` select, or nothing if they are
  /// out of range
  [[nodiscard]] std::optional<Position>
  locate(std::span<const std::size_t> path) const;
  /// Return the leaves of the part at `at`, from the first to after the last
  [[nodiscard]] std::pair<std::size_t, std::size_t> range(Position at) const;
  /// Return the part at `at` as a value of its own
  [[nodiscard]] Value part(Position at) const;
  /**
   * Return element `i` of the outermost list
   *
   * The element is a scalar for a one-dimensional array and a row otherwise.
   */
  [[nodiscard]] Value element(std::size_t i) const {
    return part({.level = 1, .pos = i});
  }
};

/**
 * Array built row by row from the values of a repeated statement
 *
 * Row r is the value the name had in iteration r. For example, the rows
 * [1,2], [], and [3] give a value of depth 2 with the levels {0,3} and
 * {0,2,2,3} and leaves 1,2,3. A missing row (from an iteration that did not
 * define the name) becomes an empty list, which needs rows that are lists.
 *
 * Depth and type come from the rows added. A column without rows takes them
 * from the statement defining the name.
 */
class Column {
public:
  /// Empty column for rows of type `t` and depth `depth`
  Column(ValueType t, std::size_t depth);
  /// Add a row `v`; throws ShapeError with a phrase completing the name
  void add(Value &v);
  /// Add a missing row
  void skip();
  /// Return the array
  Value finish();

private:
  /// Make the levels of a column of rows of depth `d`
  void start_levels();

  /// The array
  Value c_;
  /// Number of rows
  std::size_t rows_ = 0;
  /// Missing rows before the first row added
  std::size_t pending_ = 0;
  /// Whether a row has been added
  bool started_ = false;
  /// Whether a row has been missing
  bool missing_ = false;
  /// Depth of the rows
  std::size_t d_;
};

/// A named value
struct Entry {
  String name;
  Value value;
};

/// Implementation of Data
struct DataImpl {
  /// The entries in definition order
  Seq<Entry> entries;
  /// Return the value of entry `n`, or null
  [[nodiscard]] const Value *find(std::string_view n) const {
    for (const Entry &e : entries)
      if (e.name == n)
        return &e.value;
    return nullptr;
  }
};

/// Implementation of Params
struct ParamsImpl {
  Seq<String> names;
  Seq<String> values;
  /// Return the value of `name`, or nothing if it is not set
  [[nodiscard]] std::optional<std::string_view>
  get(std::string_view name) const {
    for (std::size_t i = 0; i < names.size(); i++)
      if (names[i] == name)
        return values[i];
    return std::nullopt;
  }
};

} // namespace iris::detail
