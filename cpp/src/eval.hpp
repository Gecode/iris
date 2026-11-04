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
#include "budget.hpp"
#include "fail.hpp"
#include "input.hpp"
#include "value.hpp"

#include <iris/error.hpp>

#include <cstddef>
#include <cstdint>
#include <optional>
#include <string>
#include <string_view>

namespace iris::detail {

/// Return `s` in single quotes, with newlines and tabs escaped
std::string quote(std::string_view s);

/**
 * Records or a list of records of a named repeat
 *
 * The indices `path` select them in the record starts of the repeat (see
 * record_starts), and in each name defined in the repeat.
 */
struct RecordRef {
  /// The repeat, or null
  const Stmt *repeat = nullptr;
  /// The indices
  Seq<std::size_t> path;
};

/// A part of the explanation of a failed check, such as "record 3 of
/// 'jobs'", with the position of the record in the input if known
struct Witness {
  /// The text
  std::string text;
  /// The index of the element or record, from 0
  std::size_t index;
  /// The position, or nothing
  std::optional<std::size_t> pos;
};

/// A name bound by a comprehension, visible while the binding exists
class Binding {
public:
  /// The name
  const std::string &name;
  /// The value, unless a record
  const Value *value = nullptr;
  /// The record, if a record of a named repeat
  const RecordRef *record = nullptr;
  /// The binding of the enclosing comprehension, or null
  const Binding *outer;
  /// Bind `n` inside the bindings starting at `b`
  Binding(const Binding *&b, const std::string &n)
      : name(n), outer(b), innermost_(b) {
    innermost_ = this;
  }
  Binding(const Binding &) = delete;
  Binding(Binding &&) = delete;
  Binding &operator=(const Binding &) = delete;
  Binding &operator=(Binding &&) = delete;
  /// Remove the binding
  ~Binding() { innermost_ = outer; }

private:
  /// The innermost binding, which this binding is while it exists
  const Binding *&innermost_;
};

/// A statement being executed, for errors
struct Context {
  /// What is executed, such as a name
  std::string text;
  /// The iteration of a repeat or instances statement, from 1, or 0
  std::size_t iteration = 0;
  /// The number of iterations, or nothing if unknown
  std::optional<std::size_t> iterations;
};

/**
 * Evaluator of the expressions of a reader spec on an input
 *
 * Holds the values defined so far and evaluates expressions and checks with
 * them. Errors point at the current position in the input.
 */
class Evaluator {
public:
  /// Initialize for reader spec `s` and input `text` from `origin`
  Evaluator(const SpecImpl &s, std::string_view text, std::string origin);

protected:
  /// The reader spec
  const SpecImpl &spec;
  /// The input
  Input in;
  /// The work budgets
  Budget budget;
  /// Values: the outermost frame and one per active repeat iteration
  Seq<Seq<Entry>> frames;
  /// Description of the statements being executed, for errors
  Seq<Context> context;
  /// Reader-spec line of the current statement, or 0
  std::size_t spec_line = 0;
  /// Names bound by the comprehensions being evaluated
  const Binding *bindings = nullptr;
  /// The check being evaluated, or null
  const Stmt *checked = nullptr;
  /// Values of its record names (see Record)
  Seq<const Value *> records;
  /// The records of the deepest of them, one index per repeat level
  const Seq<std::size_t> *record_at = nullptr;
  /// Why the last condition evaluated is false, for messages: the elements
  /// of the all-comprehensions that failed, innermost first
  Seq<Witness> why;

  /// Return where an error at `at` happened, with the reader-spec line and
  /// the context
  [[nodiscard]] ReadSite site(const Place &at) const;
  /// Return the error `code` with message `msg` at `at`
  [[nodiscard]] Error located(ErrorCode code, std::string msg,
                              const Place &at) const;
  /// Throw the error `code` with message `msg` at the current position
  [[noreturn]] void fail(ErrorCode code, std::string msg) const;
  /// Throw an error for the shape error `e` of the value of `n`
  [[noreturn]] void fail(const ShapeError &e, const std::string &n) const;
  /// Account for visiting `k` values in expressions (Specification §10.3)
  void visit(std::uint64_t k) { budget.charge_effort(k); }
  /// Define `n` as `v` in the innermost frame
  void set(const std::string &n, Value &&v);
  /// Return the value of `n`, or null if it is not defined
  [[nodiscard]] const Value *lookup(std::string_view n) const;

  /*
   * Expressions
   */
  /// Test whether `v` is true
  static bool truth(const Value &v);
  /// Return `b` as an integer
  static Value boolean(bool b);
  /**
   * Evaluate `e`, avoiding a copy for names
   *
   * The value is stored in `tmp` unless `e` is a name. Callers pass an
   * integer scalar without a leaf as `tmp`, which needs no memory.
   */
  const Value &ref(const Expr &e, Value &tmp);
  /// Return the indices of the record `at` of the check that select the
  /// records of its record name `i`
  [[nodiscard]] Seq<std::size_t> record_path(std::size_t i) const;
  /// Return the record starts of repeat `r`, failing for `n` if the repeat
  /// has not run yet
  const Value &starts(const Stmt *r, const std::string &n);
  /// Return the number of records in list `l`
  std::size_t count_records(const RecordRef &l);
  /// Return where record `i` of list `l` starts in the input, or nothing
  std::optional<std::size_t> record_start(const RecordRef &l, std::size_t i);
  /// Evaluate `e`, a record or a list of records
  RecordRef records_of(const Expr &e);
  /**
   * Evaluate field `e` of a record or a list of records
   *
   * The field of a list is the array of the field of each record. The
   * value is stored in `tmp` unless it needs no copy.
   */
  const Value &field(const Expr &e, Value &tmp);
  /// Return the integer of single value `v`, or fail for `op`
  std::int64_t integer(const Value &v, const std::string &op);
  /// Test whether `e` passes on why it is false (see why)
  static bool explains(const Expr &e);
  /// Evaluate `e`, keeping why only if `e` explains it
  Value eval(const Expr &e);
  /// Evaluate `e`
  Value evaluate(const Expr &e);
  /// Return whether `e` is true, avoiding a copy for names
  bool test(const Expr &e);
  /// Return whether `e` is true, with only its own explanation in why
  bool explained(const Expr &e);
  /// Evaluate unary expression `e`
  Value unary(const Expr &e);
  /// Evaluate binary expression `e`
  Value binary(const Expr &e);
  /// Return the result of the integer arithmetic of `e` on `x` and `y`
  std::int64_t arithmetic(const Expr &e, std::int64_t x, std::int64_t y);
  /// Compare single values `a` and `b` with the operator of `e`
  bool compare(const Value &a, const Value &b, const Expr &e);
  /// Call a function (names and arguments are checked by the parser)
  Value call(const Expr &e);
  /// Test whether name `e` is defined
  [[nodiscard]] bool defined(const Expr &e) const;
  /// Test whether all integers in the arguments of call `e` differ
  bool distinct(const Expr &e);
  /// Describe the array of index expression `e`, for messages
  static std::string indexed(const Expr &e);
  /// Return the value of index `e`
  std::int64_t index(const Expr &e);
  /**
   * Return a hint for indexing `e`, a single value
   *
   * A record name in a check is one record, so that indexing the records
   * needs their name.
   */
  [[nodiscard]] std::string scalar_hint(const Expr &e) const;
  /// a[i]: element i, counted from 0, of the outermost list of a
  Value subscript(const Expr &e);
  /// Describe array `a` of a comprehension, for messages
  static std::string array_name(const Expr &a);
  /// Return the records that are the elements of array `a` of
  /// comprehension `e`, an array of values (see Expr::elements)
  RecordRef element_records(const Expr &e, const Expr &a);
  /// Add why element `i` of the array of all-comprehension `e`, with
  /// records `r`, is false
  void explain(const Expr &e, const RecordRef &r, std::size_t i);
  /**
   * all, any, count, and sum over the elements of an array
   *
   * The name of the comprehension `e` is bound to each element of the
   * outermost list of the array in turn, or to each record of a list of
   * records. `all` and `any` stop at the first element that decides the
   * result.
   */
  Value comprehension(const Expr &e);
  /// Return the value of `e`, a count described by `what`
  std::size_t count(const Expr &e, std::string_view what);
  /**
   * Test whether check `s` holds, and otherwise store why it fails in
   * `error`
   *
   * The check holds for every record of its record names (see Record) that
   * are defined. The others are not defined when its expression uses them.
   */
  bool holds(const Stmt &s, std::optional<Error> &error);
  /// Test whether record name `i` of check `s` is defined and ranges over
  /// the repeat at `level`
  [[nodiscard]] bool ranges(const Stmt &s, std::size_t i,
                            std::size_t level) const;
  /**
   * Evaluate check `s` for the records inside the records `at` of its
   * record name `deepest`, and return whether it holds
   *
   * `at` has a record number per repeat level, from the outermost. Each
   * record name that ranges over a level is set to its record. If the check
   * fails, `error` is why.
   */
  bool check_records(const Stmt &s, std::optional<std::size_t> deepest,
                     Seq<std::size_t> &at, std::optional<Error> &error);
  /// Return where the record `at` of record name `r` starts in the input,
  /// or nothing if that is unknown
  std::optional<std::size_t> record_place(const Record &r,
                                          const Seq<std::size_t> &at);
  /// Return the place of position `q` in the input, or the current place if
  /// there is none or it is ahead
  [[nodiscard]] Place place(std::optional<std::size_t> q) const;
  /// Describe the records `at` of record name `r`, innermost first
  static std::string record_text(const Record &r, const Seq<std::size_t> &at);
  /// Return why check `s` fails for the records `at` of its record name
  /// `deepest`
  Error failure(const Stmt &s, std::optional<std::size_t> deepest,
                const Seq<std::size_t> &at);
};

} // namespace iris::detail
