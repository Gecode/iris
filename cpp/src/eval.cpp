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

// Ported from gecode/instance/eval.cpp at Gecode commit
// 599f977b7855dba2a1a5f67db1892722372067d3.

#include "eval.hpp"

#include "numbers.hpp"

#include <algorithm>
#include <cassert>
#include <climits>
#include <utility>

namespace iris::detail {

String quote(std::string_view s) {
  String r = "'";
  for (char c : s) {
    if (c == '\n')
      r += "\\n";
    else if (c == '\t')
      r += "\\t";
    else
      r += c;
  }
  return r + "'";
}

Evaluator::Evaluator(const SpecImpl &s, std::string_view text, String origin)
    : spec(s), in(text, std::move(origin)), budget(text.size()) {
  frames.emplace_back();
}

ReadSite Evaluator::site(const Place &at) const {
  ReadSite r{.input = Location{.origin = std::string(in.origin),
                               .line = at.line,
                               .column = at.column()},
             .excerpt = in.excerpt(at.start),
             .spec = std::nullopt,
             .context = std::nullopt};
  if (spec_line > 0)
    r.spec = Location{.origin = std::string(spec.origin),
                      .line = spec_line,
                      .column = std::nullopt};
  if (!context.empty()) {
    std::string c;
    for (std::size_t k = 0; k < context.size(); k++) {
      const Context &x = context[k];
      c += k > 0 ? " > " : "";
      c += x.text;
      if (x.iteration > 0) {
        c += " [" + std::to_string(x.iteration);
        if (x.iterations)
          c += " of " + std::to_string(*x.iterations);
        c += "]";
      }
    }
    r.context = std::move(c);
  }
  return r;
}

Error Evaluator::located(ErrorCode code, std::string_view msg,
                         const Place &at) const {
  return read_error(code, msg, site(at));
}

void Evaluator::fail(ErrorCode code, std::string_view msg) const {
  throw located(code, msg, in.here());
}

void Evaluator::fail(const ShapeError &e, const String &n) const {
  fail(e.code, "'" + n + "': " + e.msg);
}

void Evaluator::set(const String &n, Value &&v) {
  Seq<Entry> &f = frames.back();
  for (Entry &e : f)
    if (e.name == n) {
      e.value = std::move(v);
      return;
    }
  f.push_back(Entry{.name = n, .value = std::move(v)});
}

const Value *Evaluator::lookup(std::string_view n) const {
  for (const Binding *b = bindings; b != nullptr; b = b->outer)
    if (b->name == n)
      return b->value;
  for (std::size_t k = frames.size(); k-- > 0;)
    for (const Entry &e : frames[k])
      if (e.name == n)
        return &e.value;
  return nullptr;
}

bool Evaluator::truth(const Value &v) {
  if (v.depth() > 0)
    return v.size() > 0;
  switch (v.type) {
  case ValueType::Int:
    return v.ints[0] != 0;
  case ValueType::Float:
    return v.floats[0] != 0.0;
  case ValueType::String:
    return !v.strings[0].empty();
  }
  std::unreachable();
}

Value Evaluator::boolean(bool b) { return Value::integer(b ? 1 : 0); }

const Value &Evaluator::ref(const Expr &e, Value &tmp) {
  assert(e.records == nullptr);
  if (e.kind == Expr::Kind::Name) {
    const Value *v = e.record ? records[*e.record] : lookup(e.s);
    if (v == nullptr)
      fail(ErrorCode::NameUndefined, "'" + e.s + "' is not defined yet");
    return *v;
  }
  if (e.kind == Expr::Kind::Field)
    return field(e, tmp);
  tmp = eval(e);
  return tmp;
}

Seq<std::size_t> Evaluator::record_path(std::size_t i) const {
  const std::size_t n = checked->records[i].repeats.size();
  return {record_at->begin(),
          record_at->begin() + static_cast<std::ptrdiff_t>(n)};
}

const Value &Evaluator::starts(const Stmt *r, const String &n) {
  const Value *v = lookup(record_starts(r->id));
  if (v == nullptr)
    fail(ErrorCode::NameUndefined, "'" + n + "' is not defined yet");
  return *v;
}

std::size_t Evaluator::count_records(const RecordRef &l) {
  const Value &v = starts(l.repeat, l.repeat->list);
  std::optional<Value::Position> at = v.locate(l.path);
  if (!at || (at->level >= v.depth()))
    return 0;
  return v.list_size(at->level, at->pos);
}

std::optional<std::size_t> Evaluator::record_start(const RecordRef &l,
                                                   std::size_t i) {
  const Value *v = lookup(record_starts(l.repeat->id));
  if (v == nullptr)
    return std::nullopt;
  Seq<std::size_t> p(l.path);
  p.push_back(i);
  std::optional<Value::Position> at = v->locate(p);
  if (!at || (at->level != v->depth()))
    return std::nullopt;
  return static_cast<std::size_t>(v->ints[at->pos]);
}

RecordRef Evaluator::records_of(const Expr &e) {
  assert(e.records != nullptr);
  RecordRef r;
  r.repeat = e.records;
  if (e.record) {
    // A list of records that a check ranges over
    if (records[*e.record] == nullptr)
      fail(ErrorCode::NameUndefined,
           "'" + checked->records[*e.record].label + "' is not defined yet");
    r.path = record_path(*e.record);
    return r;
  }
  switch (e.kind) {
  case Expr::Kind::Name:
    for (const Binding *b = bindings; b != nullptr; b = b->outer)
      if (b->name == e.s)
        return *b->record;
    // A record name outside a check is the current record
    if (e.record_list)
      (void)starts(e.records, e.s);
    return r;
  case Expr::Kind::Field:
    // A list of records in a record
    r.path = records_of(*e.args[0]).path;
    return r;
  default: {
    r = records_of(*e.args[0]);
    std::int64_t k = index(*e.args[1]);
    std::size_t n = count_records(r);
    if ((k < 0) || std::cmp_greater_equal(k, n))
      fail(ErrorCode::IndexRange,
           "index " + to_string(k) + " is out of range for '" + r.repeat->list +
               "' (size " + to_string(n) + ", indices start at 0)");
    r.path.push_back(static_cast<std::size_t>(k));
    visit(1);
    return r;
  }
  }
}

const Value &Evaluator::field(const Expr &e, Value &tmp) {
  if (e.record) {
    if (records[*e.record] == nullptr)
      fail(ErrorCode::NameUndefined, "'" + e.s + "' is not defined yet");
    return *records[*e.record];
  }
  RecordRef r = records_of(*e.args[0]);
  const Value *v = lookup(e.s);
  if (v == nullptr)
    fail(ErrorCode::NameUndefined, "'" + e.s + "' is not defined yet");
  if (r.path.empty())
    return *v;
  std::optional<Value::Position> at = v->locate(r.path);
  if (!at)
    fail(ErrorCode::FieldUndefined, "'" + e.s +
                                        "' is not defined in this record of '" +
                                        r.repeat->list + "'");
  tmp = v->part(*at);
  visit(std::uint64_t{1} + tmp.leaves());
  return tmp;
}

std::int64_t Evaluator::integer(const Value &v, const String &op) {
  if ((v.depth() > 0) || (v.type != ValueType::Int))
    fail(ErrorCode::IntegerType, "'" + op + "' needs integers");
  return v.ints[0];
}

bool Evaluator::explains(const Expr &e) {
  if (e.kind == Expr::Kind::Binary)
    return (e.op == Expr::Op::Or) || (e.op == Expr::Op::And);
  return (e.kind == Expr::Kind::Comprehension) && (e.s == "all");
}

Value Evaluator::eval(const Expr &e) {
  Value r = evaluate(e);
  if (!explains(e))
    why.clear();
  return r;
}

Value Evaluator::evaluate(const Expr &e) {
  switch (e.kind) {
  case Expr::Kind::Int:
    return Value::integer(e.int_value);
  case Expr::Kind::Float:
    return Value::real(e.float_value);
  case Expr::Kind::Str:
    return Value::string(e.s);
  case Expr::Kind::Name:
  case Expr::Kind::Field: {
    Value tmp(ValueType::Int);
    return ref(e, tmp);
  }
  case Expr::Kind::Unary:
    return unary(e);
  case Expr::Kind::Binary:
    return binary(e);
  case Expr::Kind::Index:
    return subscript(e);
  case Expr::Kind::Comprehension:
    return comprehension(e);
  case Expr::Kind::Call:
    return call(e);
  }
  std::unreachable();
}

bool Evaluator::test(const Expr &e) {
  Value tmp(ValueType::Int);
  return truth(ref(e, tmp));
}

bool Evaluator::explained(const Expr &e) {
  why.clear();
  return test(e);
}

Value Evaluator::unary(const Expr &e) {
  if (e.op == Expr::Op::Not)
    return boolean(!test(*e.args[0]));
  Value tmp(ValueType::Int);
  const Value &a = ref(*e.args[0], tmp);
  if (a.is_number() && (a.type == ValueType::Float))
    return Value::real(-a.floats[0]);
  std::optional<std::int64_t> r = checked_neg(integer(a, e.s));
  if (!r)
    fail(ErrorCode::IntegerOverflow, "integer overflow in '-'");
  return Value::integer(*r);
}

Value Evaluator::binary(const Expr &e) {
  // Only evaluate the second operand if needed
  if (e.op == Expr::Op::Or)
    return boolean(explained(*e.args[0]) || explained(*e.args[1]));
  if (e.op == Expr::Op::And)
    return boolean(explained(*e.args[0]) && explained(*e.args[1]));
  Value ta(ValueType::Int);
  Value tb(ValueType::Int);
  const Value &a = ref(*e.args[0], ta);
  const Value &b = ref(*e.args[1], tb);
  switch (e.op) {
  case Expr::Op::Eq:
  case Expr::Op::Ne:
  case Expr::Op::Lt:
  case Expr::Op::Gt:
  case Expr::Op::Le:
  case Expr::Op::Ge:
    return boolean(compare(a, b, e));
  default:
    break;
  }
  // Arithmetic is on integers: reader specs compute sizes and counts
  std::int64_t x = integer(a, e.s);
  std::int64_t y = integer(b, e.s);
  return Value::integer(arithmetic(e, x, y));
}

std::int64_t Evaluator::arithmetic(const Expr &e, std::int64_t x,
                                   std::int64_t y) {
  std::optional<std::int64_t> r;
  switch (e.op) {
  case Expr::Op::Add:
    r = checked_add(x, y);
    break;
  case Expr::Op::Sub:
    r = checked_sub(x, y);
    break;
  case Expr::Op::Mul:
    r = checked_mul(x, y);
    break;
  case Expr::Op::Pow:
    if (y < 0)
      fail(ErrorCode::Exponent, "negative exponent in '^'");
    r = checked_pow(x, y);
    break;
  default:
    // Division and remainder truncate towards zero
    if (y == 0)
      fail(ErrorCode::DivisionZero, "division by zero");
    r = (e.op == Expr::Op::Div) ? checked_div(x, y) : checked_rem(x, y);
    break;
  }
  if (!r)
    fail(ErrorCode::IntegerOverflow, "integer overflow in '" + e.s + "'");
  return *r;
}

bool Evaluator::compare(const Value &a, const Value &b, const Expr &e) {
  if ((a.depth() > 0) || (b.depth() > 0))
    fail(ErrorCode::ComparisonType,
         "'" + e.s + "' compares single values, not arrays");
  int c = 0;
  if ((a.type == ValueType::String) || (b.type == ValueType::String)) {
    if ((e.op != Expr::Op::Eq) && (e.op != Expr::Op::Ne))
      fail(ErrorCode::ComparisonType, "'" + e.s + "' compares numbers");
    c = ((a.type == b.type) && (a.strings[0] == b.strings[0])) ? 0 : 1;
  } else if ((a.type == ValueType::Int) && (b.type == ValueType::Int)) {
    c = (a.ints[0] < b.ints[0]) ? -1 : (a.ints[0] > b.ints[0] ? 1 : 0);
  } else {
    // An integer compared with a float is converted to a float
    double x = a.as_double();
    double y = b.as_double();
    c = (x < y) ? -1 : (x > y ? 1 : 0);
  }
  switch (e.op) {
  case Expr::Op::Eq:
    return c == 0;
  case Expr::Op::Ne:
    return c != 0;
  case Expr::Op::Lt:
    return c < 0;
  case Expr::Op::Gt:
    return c > 0;
  case Expr::Op::Le:
    return c <= 0;
  default:
    return c >= 0;
  }
}

Value Evaluator::call(const Expr &e) {
  const String &f = e.s;
  if (f == "defined")
    return boolean(defined(*e.args[0]));
  if (((f == "count") || (f == "size")) && (e.args[0]->records != nullptr))
    return Value::integer(
        static_cast<std::int64_t>(count_records(records_of(*e.args[0]))));
  if ((f == "count") || (f == "size")) {
    Value tmp(ValueType::Int);
    const Value &v = ref(*e.args[0], tmp);
    if (v.depth() == 0)
      fail(ErrorCode::FunctionType, "'" + f + "' takes an array");
    return Value::integer(static_cast<std::int64_t>(v.size()));
  }
  if (f == "distinct")
    return boolean(distinct(e));
  // min, max, and sum over all integers in all arguments
  bool any = false;
  std::int64_t r = 0;
  for (const Node<Expr> &a : e.args) {
    Value tmp(ValueType::Int);
    const Value &v = ref(*a, tmp);
    if ((v.type != ValueType::Int) && (v.leaves() > 0))
      fail(ErrorCode::FunctionType, "'" + f + "' takes integers");
    visit(v.leaves());
    for (std::int64_t x : v.ints) {
      if (f == "sum") {
        std::optional<std::int64_t> s = checked_add(r, x);
        if (!s)
          fail(ErrorCode::IntegerOverflow, "integer overflow in 'sum'");
        r = *s;
      } else if (!any || ((f == "min") ? (x < r) : (x > r))) {
        r = x;
      }
      any = true;
    }
  }
  if (!any && (f != "sum"))
    fail(ErrorCode::FunctionEmpty, "'" + f + "' of no integers");
  return Value::integer(r);
}

bool Evaluator::defined(const Expr &e) const {
  for (const Binding *b = bindings; b != nullptr; b = b->outer)
    if (b->name == e.s)
      return true;
  if (e.records != nullptr)
    return lookup(record_starts(e.records->id)) != nullptr;
  return lookup(e.s) != nullptr;
}

bool Evaluator::distinct(const Expr &e) {
  Seq<std::int64_t> x;
  for (const Node<Expr> &a : e.args) {
    Value tmp(ValueType::Int);
    const Value &v = ref(*a, tmp);
    if ((v.type != ValueType::Int) && (v.leaves() > 0))
      fail(ErrorCode::FunctionType, "'distinct' takes integers");
    visit(v.leaves());
    x.insert(x.end(), v.ints.begin(), v.ints.end());
  }
  std::ranges::sort(x);
  return std::ranges::adjacent_find(x) == x.end();
}

String Evaluator::indexed(const Expr &e) {
  const Expr *a = e.args[0].get();
  bool nested = false;
  while (a->kind == Expr::Kind::Index) {
    a = a->args[0].get();
    nested = true;
  }
  if ((a->kind != Expr::Kind::Name) && (a->kind != Expr::Kind::Field))
    return "the array";
  return (nested ? "an element of '" : "'") + a->s + "'";
}

std::int64_t Evaluator::index(const Expr &e) {
  Value tmp(ValueType::Int);
  const Value &i = ref(e, tmp);
  if ((i.depth() > 0) || (i.type != ValueType::Int))
    fail(ErrorCode::IndexType, "an index must be an integer");
  return i.ints[0];
}

String Evaluator::scalar_hint(const Expr &e) const {
  if (!e.record)
    return "";
  const Record &r = checked->records[*e.record];
  const String &l = r.repeats.back()->list;
  String m = ", which is one value of each record here; to index the records, ";
  if (l.empty())
    return m +
           "name them, as in 'repeat n as nodes { each node ... }', and use "
           "'nodes[0]." +
           e.s + "'";
  return m + "use '" + l + "[0]." + e.s + "'";
}

Value Evaluator::subscript(const Expr &e) {
  assert(e.records == nullptr);
  Value ta(ValueType::Int);
  const Value &a = ref(*e.args[0], ta);
  if (a.depth() == 0)
    fail(ErrorCode::IndexScalar, "only arrays can be indexed, not " +
                                     indexed(e) + scalar_hint(*e.args[0]));
  std::int64_t k = index(*e.args[1]);
  if ((k < 0) || std::cmp_greater_equal(k, a.size()))
    fail(ErrorCode::IndexRange,
         "index " + to_string(k) + " is out of range for " + indexed(e) +
             " (size " + to_string(a.size()) + ", indices start at 0)");
  Value r = a.element(static_cast<std::size_t>(k));
  visit(std::uint64_t{1} + r.leaves());
  return r;
}

String Evaluator::array_name(const Expr &a) {
  if (a.kind == Expr::Kind::Name)
    return "'" + a.s + "'";
  if ((a.kind == Expr::Kind::Field) && (a.args[0]->kind == Expr::Kind::Name))
    return "'" + a.args[0]->s + "." + a.s + "'";
  return "the array";
}

RecordRef Evaluator::element_records(const Expr &e, const Expr &a) {
  RecordRef r;
  r.repeat = e.elements;
  if (a.kind == Expr::Kind::Field)
    r.path = records_of(*a.args[0]).path;
  return r;
}

void Evaluator::explain(const Expr &e, const RecordRef &r, std::size_t i) {
  Witness w{.text = {}, .index = i, .pos = std::nullopt};
  String n = to_string(i + 1);
  if (e.elements == nullptr) {
    w.text = "element " + n + " of " + array_name(*e.args[0]);
  } else {
    const String &l = e.elements->list;
    w.text = "record " + n + " of " +
             (l.empty() ? array_name(*e.args[0]) : "'" + l + "'");
    w.pos = record_start(r, i);
  }
  why.push_back(std::move(w));
}

Value Evaluator::comprehension(const Expr &e) {
  const String &f = e.s;
  const Expr &source = *e.args[0];
  Value ta(ValueType::Int);
  const Value *a = nullptr;
  RecordRef list;
  RecordRef x;
  std::size_t n = 0;
  if (source.records != nullptr) {
    list = records_of(source);
    n = count_records(list);
  } else {
    a = &ref(source, ta);
    if (a->depth() == 0)
      fail(ErrorCode::FunctionType,
           "'" + f + "(" + e.var + " in ...)' needs an array");
    n = a->size();
  }
  RecordRef elements;
  if ((f == "all") && (e.elements != nullptr))
    elements = (a == nullptr) ? list : element_records(e, source);
  Binding b(bindings, e.var);
  std::int64_t r = 0;
  for (std::size_t i = 0; i < n; i++) {
    Value v;
    if (a != nullptr) {
      v = a->element(i);
      visit(std::uint64_t{1} + v.leaves());
      b.value = &v;
    } else {
      x = list;
      x.path.push_back(i);
      visit(1);
      b.record = &x;
    }
    if (f == "sum") {
      Value tv(ValueType::Int);
      std::optional<std::int64_t> s =
          checked_add(r, integer(ref(*e.args[1], tv), "sum"));
      if (!s)
        fail(ErrorCode::IntegerOverflow, "integer overflow in 'sum'");
      r = *s;
    } else if (explained(*e.args[1])) {
      if (f == "any")
        return boolean(true);
      r++;
    } else if (f == "all") {
      explain(e, elements, i);
      return boolean(false);
    }
  }
  why.clear();
  if ((f == "all") || (f == "any"))
    return boolean(f == "all");
  return Value::integer(r);
}

std::size_t Evaluator::count(const Expr &e, std::string_view what) {
  Value tmp(ValueType::Int);
  const Value &v = ref(e, tmp);
  if ((v.depth() > 0) || (v.type != ValueType::Int))
    fail(ErrorCode::Count, String(what) + " must be an integer");
  std::int64_t c = v.ints[0];
  if (c < 0)
    fail(ErrorCode::Count,
         String(what) + " is negative (" + to_string(c) + ")");
  if (c > INT_MAX)
    fail(ErrorCode::Count,
         String(what) + " is too large (" + to_string(c) + ")");
  return static_cast<std::size_t>(c);
}

bool Evaluator::holds(const Stmt &s, std::optional<Error> &error) {
  records.clear();
  std::optional<std::size_t> deepest;
  for (std::size_t i = 0; i < s.records.size(); i++) {
    records.push_back(lookup(s.records[i].name));
    if (ranges(s, i, deepest ? s.records[*deepest].repeats.size() : 0))
      deepest = i;
  }
  Seq<std::size_t> at;
  checked = &s;
  record_at = &at;
  bool ok = check_records(s, deepest, at, error);
  checked = nullptr;
  record_at = nullptr;
  return ok;
}

bool Evaluator::ranges(const Stmt &s, std::size_t i, std::size_t level) const {
  return (records[i] != nullptr) && (s.records[i].repeats.size() > level);
}

bool Evaluator::check_records(const Stmt &s, std::optional<std::size_t> deepest,
                              Seq<std::size_t> &at,
                              std::optional<Error> &error) {
  std::size_t level = at.size();
  if (!deepest || (level == s.records[*deepest].repeats.size())) {
    if (explained(*s.expr))
      return true;
    error = failure(s, deepest, at);
    return false;
  }
  std::size_t n = 0;
  std::optional<std::size_t> first;
  for (std::size_t i = 0; i < records.size(); i++)
    if (ranges(s, i, level)) {
      if (records[i]->depth() == 0)
        fail(ErrorCode::CheckRecords,
             "'" + s.records[i].label + "' has no records");
      if (!first) {
        first = i;
        n = records[i]->size();
      } else if (records[i]->size() != n) {
        fail(ErrorCode::CheckRecords,
             "'" + s.records[*first].label + "' and '" + s.records[i].label +
                 "' have different numbers of records");
      }
    }
  Seq<const Value *> outer(records);
  Seq<Value> rows;
  // No reallocation, so the record names can point at the rows
  rows.reserve(records.size());
  for (std::size_t r = 0; r < n; r++) {
    rows.clear();
    for (std::size_t i = 0; i < records.size(); i++)
      if (ranges(s, i, level)) {
        rows.push_back(outer[i]->element(r));
        visit(std::uint64_t{1} + rows.back().leaves());
        records[i] = &rows.back();
      }
    at.push_back(r);
    bool ok = check_records(s, deepest, at, error);
    at.pop_back();
    if (!ok)
      return false;
  }
  records = outer;
  return true;
}

std::optional<std::size_t> Evaluator::record_place(const Record &r,
                                                   const Seq<std::size_t> &at) {
  const Value *p = lookup(r.starts);
  if (p == nullptr)
    return std::nullopt;
  std::optional<Value::Position> x = p->locate(at);
  if (!x || (x->level != p->depth()))
    return std::nullopt;
  return static_cast<std::size_t>(p->ints[x->pos]);
}

Place Evaluator::place(std::optional<std::size_t> q) const {
  return (q && (*q <= in.pos)) ? in.place(*q) : in.here();
}

String Evaluator::record_text(const Record &r, const Seq<std::size_t> &at) {
  bool named = true;
  for (const Stmt *x : r.repeats)
    named = named && !x->list.empty();
  String m;
  for (std::size_t k = at.size(); k-- > 0;) {
    m += "record " + to_string(at[k] + 1) + " of ";
    if (named)
      m += "'" + r.repeats[k]->list + "'" + (k > 0 ? " in " : "");
  }
  return named ? m : m + "'" + r.label + "'";
}

Error Evaluator::failure(const Stmt &s, std::optional<std::size_t> deepest,
                         const Seq<std::size_t> &at) {
  // The innermost record found is where the check failed
  String m;
  std::optional<std::size_t> q;
  for (const Witness &w : why) {
    m += (m.empty() ? "" : " in ") + w.text;
    if (!q)
      q = w.pos;
  }
  if (deepest && !at.empty()) {
    m += (m.empty() ? "" : " in ") + record_text(s.records[*deepest], at);
    if (!q)
      q = record_place(s.records[*deepest], at);
  }
  m = "check failed" + (m.empty() ? "" : " for " + m);
  if (!s.message.empty())
    m += ": " + s.message;
  // The lifted record indices, then the witnesses, outermost first
  Seq<std::size_t> path(at);
  for (std::size_t k = why.size(); k-- > 0;)
    path.push_back(why[k].index);
  return check_error(m, site(place(q)), path);
}

} // namespace iris::detail
