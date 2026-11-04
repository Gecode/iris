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

// Ported from gecode/instance/resolve.cpp at Gecode commit
// 5e7bbd5c26b0702c6fd652356b2289da0710ac3c.

#include "ast.hpp"
#include "fail.hpp"

#include <algorithm>
#include <cstddef>
#include <optional>
#include <string>
#include <utility>

namespace iris::detail {

namespace {

/// Repeats around a statement or a definition, outermost first
using Repeats = Seq<const Stmt *>;

/// A definition of a name, with the repeats around it
struct Definition {
  String name;
  Repeats repeats;
};

/// A repeat that names its records (see Stmt::list and Stmt::each), with
/// the repeats around it
struct NamedRepeat {
  const Stmt *repeat;
  Repeats repeats;
};

/// A name bound by a comprehension, with the repeat whose records it stands
/// for (null for the elements of an array)
struct Bound {
  String name;
  const Stmt *records;
};

/// Test whether `a` is a prefix of `b`
bool is_prefix(const Repeats &a, const Repeats &b) {
  return (a.size() <= b.size()) && std::equal(a.begin(), a.end(), b.begin());
}

/// Return the number of repeats that `a` and `b` start with in common
std::size_t common_prefix(const Repeats &a, const Repeats &b) {
  const auto [end, unused] = std::ranges::mismatch(a, b);
  return static_cast<std::size_t>(end - a.begin());
}

/// Return the index of `r` in `rs`, or nothing if it is not there
std::optional<std::size_t> position(const Repeats &rs, const Stmt *r) {
  const auto at = std::ranges::find(rs, r);
  if (at == rs.end())
    return std::nullopt;
  return static_cast<std::size_t>(at - rs.begin());
}

/**
 * Test whether operand `k` of `e` is a whole array
 *
 * The array of a comprehension and the argument of `defined` are whole
 * arrays: a record name there is not one record. Unlike in Gecode, the
 * argument of `size` is not (Specification appendix C.5, ADR 0016).
 */
bool whole_array(const Expr &e, std::size_t k) {
  if (e.kind == Expr::Kind::Comprehension)
    return k == 0;
  return (e.kind == Expr::Kind::Call) && (e.s == "defined");
}

/**
 * Resolver of the names in the expressions of a reader spec
 *
 * Finds the expressions that are records or lists of records of a named
 * repeat (see Expr) and the record names of the checks (see Record), and
 * reports their misuse.
 */
class Resolver {
public:
  /// Initialize for a reader spec from `origin`
  explicit Resolver(const String &origin) : origin_(origin) {}
  /// Resolve the expressions in `b`, the body of a reader spec
  void run(Block &b) {
    define(b);
    check_lists();
    visit(b);
  }

private:
  /// Where the reader spec comes from
  const String &origin_;
  /// All definitions of names
  Seq<Definition> definitions_;
  /// All repeats that name their records
  Seq<NamedRepeat> lists_;
  /// The repeats around the statement visited, outermost first
  Repeats repeats_;
  /// The statement visited
  Stmt *stmt_ = nullptr;
  /// The names bound by the comprehensions around the expression
  Seq<Bound> bound_;

  [[noreturn]] void fail(ErrorCode code, std::string_view m) const {
    fail_at(code, origin_, stmt_->line, m);
  }
  /// Add a definition of `n`
  void define(const String &n) {
    if (n.empty() || (n == "_"))
      return;
    definitions_.push_back(Definition{.name = n, .repeats = repeats_});
  }
  /// Add the definitions of the captures of `p`
  void define(const Pattern &p) {
    for (const Pattern::Item &it : p.items)
      if (it.kind == Pattern::Item::Kind::Cap)
        define(it.text);
  }
  /// Add the definitions and named repeats in `b`
  void define(const Block &b) {
    for (const Node<Stmt> &s : b) {
      switch (s->kind) {
      case Stmt::Kind::Read:
      case Stmt::Kind::List:
      case Stmt::Kind::Let:
      case Stmt::Kind::Param:
        define(s->name);
        break;
      case Stmt::Kind::Line:
      case Stmt::Kind::Find:
        define(s->pattern);
        break;
      case Stmt::Kind::Repeat:
        define(s->store);
        if (!s->list.empty() || !s->each.empty())
          lists_.push_back(NamedRepeat{.repeat = s.get(), .repeats = repeats_});
        repeats_.push_back(s.get());
        define(s->body);
        repeats_.pop_back();
        break;
      case Stmt::Kind::Sections:
        define(s->store);
        for (const Arm &a : s->arms) {
          define(a.pattern);
          define(a.body);
        }
        break;
      case Stmt::Kind::If:
        define(s->body);
        define(s->orelse);
        break;
      case Stmt::Kind::Instances:
        define(s->body);
        break;
      default:
        break;
      }
    }
  }
  /// Test whether `n` names the records of a repeat
  [[nodiscard]] bool names_records(const String &n) const {
    return std::ranges::any_of(lists_, [&n](const NamedRepeat &l) {
      return (l.repeat->list == n) || (l.repeat->each == n);
    });
  }
  /// Check that name `n` of the records of repeat `i` is not used otherwise
  void check_name(const String &n, std::size_t i) const {
    const Stmt *r = lists_[i].repeat;
    if (n.empty())
      return;
    for (std::size_t j = 0; j < i; j++)
      if ((lists_[j].repeat->list == n) || (lists_[j].repeat->each == n))
        fail_at(ErrorCode::SpecNameConflict, origin_, r->line,
                "'" + n + "' already names records");
    if (std::ranges::contains(definitions_, n, &Definition::name))
      fail_at(ErrorCode::SpecNameConflict, origin_, r->line,
              "'" + n + "' names records, so it cannot also be defined");
  }
  /// Check that the names of the records of repeats are not used otherwise
  void check_lists() const {
    for (std::size_t i = 0; i < lists_.size(); i++) {
      const Stmt *r = lists_[i].repeat;
      check_name(r->list, i);
      if (r->each == r->list)
        fail_at(ErrorCode::SpecNameConflict, origin_, r->line,
                "'" + r->each + "' cannot name both the list and each record");
      check_name(r->each, i);
    }
  }
  /// Return the repeat whose list of records is called `n`, or null
  [[nodiscard]] const NamedRepeat *find_list(const String &n) const {
    const auto l = std::ranges::find_if(lists_, [&n](const NamedRepeat &named) {
      return named.repeat->list == n;
    });
    return (l == lists_.end()) ? nullptr : &*l;
  }
  /// Return the repeat each of whose records is called `n`, or null
  [[nodiscard]] const NamedRepeat *find_each(const String &n) const {
    const auto l = std::ranges::find_if(lists_, [&n](const NamedRepeat &named) {
      return named.repeat->each == n;
    });
    return (l == lists_.end()) ? nullptr : &*l;
  }
  /// Return the name of the records of repeat `r`, for messages
  static const String &records_name(const Stmt *r) {
    return r->list.empty() ? r->each : r->list;
  }
  /// Test whether `n` is bound by a comprehension
  [[nodiscard]] bool is_bound(const String &n) const {
    return std::ranges::contains(bound_, n, &Bound::name);
  }
  /// Return the list of records called `n` directly inside repeat `r`, or
  /// null
  [[nodiscard]] const NamedRepeat *nested_list(const Stmt *r,
                                               const String &n) const {
    const NamedRepeat *m = find_list(n);
    if ((m == nullptr) || !std::ranges::contains(m->repeats, r))
      return nullptr;
    if (m->repeats.back() != r)
      fail(ErrorCode::SpecRecordScope,
           "'" + n +
               "' is inside a repeat without a list name in the records of '" +
               records_name(r) + "'; name that list with 'as' to reach it");
    if (std::ranges::contains(repeats_, m->repeat))
      fail(ErrorCode::SpecRecordScope,
           "'" + n + "' cannot be used inside its own repeat");
    return m;
  }
  /// Return the names that the records of repeat `r` have, for messages
  [[nodiscard]] String fields(const Stmt *r) const {
    Seq<String> f;
    for (const Definition &d : definitions_)
      if (std::ranges::contains(d.repeats, r))
        f.push_back(d.name);
    for (const NamedRepeat &l : lists_)
      if (!l.repeats.empty() && (l.repeats.back() == r) &&
          !l.repeat->list.empty())
        f.push_back(l.repeat->list);
    String s;
    for (auto i = f.begin(); i != f.end(); ++i)
      if (std::find(f.begin(), i, *i) == i)
        s += (s.empty() ? "" : ", ") + *i;
    return s;
  }
  /// Return an example of a field of a record of repeat `r`, through its
  /// list if `list`, for messages
  [[nodiscard]] String example(const Stmt *r, bool list) const {
    const String f = fields(r);
    const std::size_t c = f.find(',');
    const String x = (list || r->each.empty()) ? r->list + "[0]" : r->each;
    return f.empty() ? x : x + "." + ((c == String::npos) ? f : f.substr(0, c));
  }
  /// Test whether definition `d` of a field of the records of repeat `r` is
  /// in a repeat around the statement inside `r`, so that its values for
  /// the record are not complete
  [[nodiscard]] bool incomplete(const Definition &d, const Stmt *r) const {
    const std::optional<std::size_t> at = position(d.repeats, r);
    return common_prefix(d.repeats, repeats_) > (at ? *at + 1 : 0);
  }
  /// Check that `f` is a field of the records of repeat `r`
  void require_field(const Stmt *r, const String &f) const {
    bool inside = false;
    bool outside = false;
    for (const Definition &d : definitions_)
      if (d.name == f) {
        inside = inside || std::ranges::contains(d.repeats, r);
        outside = outside || !std::ranges::contains(d.repeats, r);
      }
    if (!inside) {
      const String known = fields(r);
      fail(ErrorCode::SpecField,
           "the records of '" + records_name(r) + "' have no field '" + f +
               "'" +
               (known.empty() ? String(" (they have none)")
                              : " (they have " + known + ")"));
    }
    if (outside)
      fail(ErrorCode::SpecField,
           "'" + f + "' is defined inside and outside the repeat of '" +
               records_name(r) + "', so it cannot be a field of its records");
    for (const Definition &d : definitions_)
      if ((d.name == f) && incomplete(d, r))
        fail(ErrorCode::SpecRecordScope,
             "the field '" + f +
                 "' cannot be used inside the repeat that reads it");
  }
  /// Check that `e` is not a record or a list of records
  void require_value(const Expr &e) const {
    const Stmt *r = e.records;
    if (r == nullptr)
      return;
    if (!e.record_list)
      fail(ErrorCode::SpecRecordValue,
           "a record of '" + records_name(r) +
               "' is not a value; use one of its fields, as in '" +
               example(r, false) + "'");
    fail(ErrorCode::SpecRecordValue,
         "'" + r->list + "' is a list of records, not a value; use size(" +
             r->list + "), an index, or a field of a record, as in '" +
             example(r, true) + "'");
  }

  /// Resolve the expressions of the statements in `b`
  void visit(Block &b) {
    for (Node<Stmt> &s : b) {
      statement(*s);
      const bool repeat = (s->kind == Stmt::Kind::Repeat);
      if (repeat)
        repeats_.push_back(s.get());
      visit(s->body);
      if (repeat)
        repeats_.pop_back();
      visit(s->orelse);
      for (Arm &a : s->arms)
        visit(a.body);
    }
  }
  /// Resolve the expressions of statement `s`
  void statement(Stmt &s) {
    stmt_ = &s;
    type(s.type);
    pattern(s.pattern);
    for (Arm &a : s.arms)
      pattern(a.pattern);
    for (Node<Expr> &d : s.dims)
      value(*d);
    if (s.expr)
      value(*s.expr);
    if (s.sentinel)
      value(*s.sentinel);
    if (s.offset)
      value(*s.offset);
    if (s.kind == Stmt::Kind::Check)
      check_chain(s);
  }
  /// Resolve the bounds of type `t`
  void type(Type &t) {
    if (t.lo)
      value(*t.lo);
    if (t.hi)
      value(*t.hi);
  }
  /// Resolve the bounds of the types of the captures of `p`
  void pattern(Pattern &p) {
    for (Pattern::Item &it : p.items)
      type(it.type);
  }
  /// Test whether the statement visited is a check, so that record names in
  /// it range over their records
  [[nodiscard]] bool lifting() const {
    return stmt_->kind == Stmt::Kind::Check;
  }
  /// Check that the record names of check `s` range over one repeat and the
  /// repeats inside it
  void check_chain(const Stmt &s) const {
    std::size_t deepest = 0;
    for (std::size_t i = 1; i < s.records.size(); i++)
      if (s.records[i].repeats.size() > s.records[deepest].repeats.size())
        deepest = i;
    for (std::size_t i = 0; i < s.records.size(); i++)
      if (!is_prefix(s.records[i].repeats, s.records[deepest].repeats))
        fail(ErrorCode::SpecCheckScope,
             "a check can only range over the records of one repeat, but '" +
                 s.records[std::min(i, deepest)].label + "' and '" +
                 s.records[std::max(i, deepest)].label +
                 "' are defined in different repeats");
  }

  /// Resolve `e`, which must be a value
  void value(Expr &e) {
    resolve(e, false);
    require_value(e);
  }
  /// Resolve `e`, which is a whole array if `whole`
  void resolve(Expr &e, bool whole) {
    switch (e.kind) {
    case Expr::Kind::Name:
      name(e, whole);
      break;
    case Expr::Kind::Field:
      field(e);
      break;
    case Expr::Kind::Index:
      index(e);
      break;
    case Expr::Kind::Comprehension:
      comprehension(e);
      break;
    case Expr::Kind::Call:
      call(e);
      break;
    default:
      for (Node<Expr> &a : e.args)
        value(*a);
      break;
    }
  }
  /// Resolve name `e`
  void name(Expr &e, bool whole) {
    const auto b =
        std::ranges::find(bound_.rbegin(), bound_.rend(), e.s, &Bound::name);
    if (b != bound_.rend()) {
      e.records = b->records;
      return;
    }
    if (const NamedRepeat *l = find_list(e.s))
      list(e, *l);
    else if (const NamedRepeat *r = find_each(e.s))
      record_name(e, *r);
    else if (lifting() && !whole)
      e.record = record(e.s);
  }
  /// Return the repeats of `d` that are not around the statement visited
  [[nodiscard]] Repeats beyond(const Repeats &d) const {
    const auto common = static_cast<std::ptrdiff_t>(common_prefix(d, repeats_));
    return {d.begin() + common, d.end()};
  }
  /// Resolve name `e` of the list of records of `l`
  void list(Expr &e, const NamedRepeat &l) {
    e.records = l.repeat;
    e.record_list = true;
    if (std::ranges::contains(repeats_, l.repeat))
      fail(ErrorCode::SpecRecordScope,
           "'" + e.s + "' cannot be used inside its own repeat");
    if (beyond(l.repeats).empty())
      return;
    // One list for each record of the repeat around it
    const String &outer = l.repeats.back()->each;
    fail(ErrorCode::SpecRecordScope,
         "'" + e.s + "' is a list in each record of the repeat around it; " +
             (outer.empty() ? "name each of these records with 'each' and use "
                              "its field '" +
                                  e.s + "'"
                            : "use '" + outer + "." + e.s + "'"));
  }
  /**
   * Resolve name `e` of each record of `l`
   *
   * Inside the repeat, the name is the current record. Elsewhere, it is a
   * record name: a check ranges over the records.
   */
  void record_name(Expr &e, const NamedRepeat &l) {
    e.records = l.repeat;
    if (std::ranges::contains(repeats_, l.repeat))
      return;
    if (!lifting())
      fail(ErrorCode::SpecRecordScope,
           "'" + e.s +
               "' is each record of a repeat, so only checks can use it "
               "outside that repeat");
    Repeats o = beyond(l.repeats);
    o.push_back(l.repeat);
    e.record = lift(record_starts(l.repeat->id), o, e.s);
  }
  /// Resolve field `e`
  void field(Expr &e) {
    Expr &base = *e.args[0];
    resolve(base, false);
    const Stmt *r = base.records;
    const String &f = e.s;
    if (r == nullptr)
      fail(ErrorCode::SpecField,
           "only records have fields such as '." + f +
               "'; to name each record of a repeat, write 'repeat n as nodes "
               "{ each node ... }' and use 'node." +
               f + "' or 'nodes[0]." + f + "'");
    if (base.record_list)
      fail(ErrorCode::SpecField,
           "'" + r->list + "' is a list of records, so '." + f +
               "' needs one of them: use '" + r->list + "[i]." + f + "', or " +
               (r->each.empty()
                    ? "name each record with 'each' in the repeat and use its "
                      "field"
                    : "'" + r->each + "." + f + "' for each record"));
    if (const NamedRepeat *m = nested_list(r, f)) {
      e.records = m->repeat;
      e.record_list = true;
    } else {
      require_field(r, f);
    }
  }
  /// Resolve index expression `e`
  void index(Expr &e) {
    Expr &a = *e.args[0];
    resolve(a, false);
    value(*e.args[1]);
    if (a.records == nullptr)
      return;
    if (!a.record_list)
      fail(ErrorCode::SpecRecordValue,
           "a record of '" + records_name(a.records) +
               "' cannot be indexed; use one of its fields, as in '" +
               example(a.records, false) + "'");
    e.records = a.records;
  }
  /// The repeat whose records are the elements of array `a`, or null
  [[nodiscard]] const Stmt *elements(const Expr &a) const {
    if (a.records != nullptr)
      return a.records;
    if (a.kind == Expr::Kind::Field)
      return outermost(a.s, a.args[0]->records);
    if ((a.kind != Expr::Kind::Name) || is_bound(a.s))
      return nullptr;
    return outermost(a.s, nullptr);
  }
  /**
   * Return the outermost repeat whose records are the rows of name `n`, or
   * null
   *
   * These are the repeats inside `r` if not null, otherwise those not around
   * the statement visited.
   */
  [[nodiscard]] const Stmt *outermost(const String &n, const Stmt *r) const {
    const Stmt *o = nullptr;
    bool found = false;
    for (const Definition &d : definitions_) {
      if (d.name != n)
        continue;
      std::size_t k = 0;
      if (r == nullptr) {
        k = common_prefix(d.repeats, repeats_);
      } else if (const std::optional<std::size_t> at = position(d.repeats, r)) {
        k = *at + 1;
      }
      const Stmt *x = (k < d.repeats.size()) ? d.repeats[k] : nullptr;
      if (found && (x != o))
        return nullptr;
      o = x;
      found = true;
    }
    return o;
  }
  /// Resolve comprehension `e`
  void comprehension(Expr &e) {
    if (names_records(e.var))
      fail(ErrorCode::SpecNameConflict,
           "'" + e.var + "' is bound by a comprehension but names records");
    Expr &a = *e.args[0];
    resolve(a, true);
    if ((a.records != nullptr) && !a.record_list)
      fail(ErrorCode::SpecRecordValue,
           "a record of '" + records_name(a.records) +
               "' is not a list; use one of its fields");
    e.elements = elements(a);
    bound_.push_back(Bound{.name = e.var, .records = a.records});
    value(*e.args[1]);
    bound_.pop_back();
  }
  /// Resolve call `e`
  void call(Expr &e) {
    const bool counted =
        (e.s == "size") || (e.s == "count") || (e.s == "defined");
    for (std::size_t k = 0; k < e.args.size(); k++) {
      Expr &a = *e.args[k];
      resolve(a, whole_array(e, k));
      if (!counted || !a.record_list)
        require_value(a);
    }
  }

  /**
   * Return the index of record name `n` among the record names of the check
   * visited, or nothing if it is not one
   *
   * A record name is defined inside repeats that are not around the check,
   * and only there.
   */
  std::optional<std::size_t> record(const String &n) {
    bool found = false;
    Repeats outside;
    for (const Definition &d : definitions_) {
      if (d.name != n)
        continue;
      Repeats o = beyond(d.repeats);
      if (found && (o != outside))
        fail(ErrorCode::SpecCheckScope,
             "'" + n +
                 "' is defined in several repeats, or inside and outside a "
                 "repeat, so a check here cannot range over its records");
      outside = std::move(o);
      found = true;
    }
    if (outside.empty())
      return std::nullopt;
    return lift(n, outside, n);
  }
  /// Return the index of the record name for the values of `n` in the
  /// records of `rs` among those of the check visited, adding it if needed;
  /// `label` is its name in messages
  std::size_t lift(const String &n, const Repeats &rs, const String &label) {
    Seq<Record> &records = stmt_->records;
    for (std::size_t i = 0; i < records.size(); i++)
      if ((records[i].name == n) && (records[i].repeats.size() == rs.size()))
        return i;
    records.push_back(Record{.name = n,
                             .label = label,
                             .repeats = rs,
                             .starts = record_starts(rs.back()->id)});
    return records.size() - 1;
  }
};

} // namespace

void resolve(Block &b, const String &origin) { Resolver(origin).run(b); }

} // namespace iris::detail
