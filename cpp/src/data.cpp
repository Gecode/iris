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

// Ported from gecode/instance/data.cpp at Gecode commit
// 5e7bbd5c26b0702c6fd652356b2289da0710ac3c.

#include <iris/data.hpp>
#include <iris/params.hpp>

#include "value.hpp"

#include <string>
#include <utility>

namespace iris {

namespace detail {

/*
 * Values
 */

void Value::to_float() {
  if (type != ValueType::Int)
    return;
  for (const std::int64_t v : ints)
    floats.push_back(static_cast<double>(v));
  ints.clear();
  type = ValueType::Float;
}

void Value::push(const Value &s) {
  if (leaves() == 0) {
    type = s.type;
  } else if ((type == ValueType::String) != (s.type == ValueType::String)) {
    throw ShapeError{.code = ErrorCode::ValueTypeMix,
                     .msg = "strings and numbers are mixed"};
  } else if ((type == ValueType::Int) && (s.type == ValueType::Float)) {
    to_float();
  }
  switch (s.type) {
  case ValueType::Int:
    if (type == ValueType::Int)
      ints.push_back(s.ints[0]);
    else
      floats.push_back(static_cast<double>(s.ints[0]));
    break;
  case ValueType::Float:
    floats.push_back(s.floats[0]);
    break;
  case ValueType::String:
    strings.push_back(s.strings[0]);
    break;
  }
}

void Value::clear_list(ValueType t) {
  type = t;
  ints.clear();
  floats.clear();
  strings.clear();
  levels.resize(1);
  levels[0].assign({0, 0});
}

void Value::shape(const Seq<std::size_t> &dims) {
  levels.clear();
  std::size_t lists = 1;
  for (const std::size_t n : dims) {
    Seq<std::size_t> &offsets = levels.emplace_back();
    offsets.push_back(0);
    for (std::size_t i = 0; i < lists; i++)
      offsets.push_back(offsets.back() + n);
    lists = offsets.back();
  }
}

std::optional<Value::Position>
Value::locate(std::span<const std::size_t> path) const {
  Position at{.level = 0, .pos = 0};
  for (const std::size_t i : path) {
    if ((at.level >= depth()) || (i >= list_size(at.level, at.pos)))
      return std::nullopt;
    at.pos = levels[at.level][at.pos] + i;
    at.level++;
  }
  return at;
}

std::pair<std::size_t, std::size_t> Value::range(Position at) const {
  std::size_t from = at.pos;
  std::size_t to = at.pos + 1;
  for (std::size_t k = at.level; k < depth(); k++) {
    from = levels[k][from];
    to = levels[k][to];
  }
  return {from, to};
}

Value Value::part(Position at) const {
  Value r(type);
  // The lists of the part at each level, then its leaves
  std::size_t from = at.pos;
  std::size_t to = at.pos + 1;
  for (std::size_t k = at.level; k < depth(); k++) {
    Seq<std::size_t> &offsets = r.levels.emplace_back();
    for (std::size_t j = from; j <= to; j++)
      offsets.push_back(levels[k][j] - levels[k][from]);
    from = levels[k][from];
    to = levels[k][to];
  }
  const auto leaves_of = [from, to](const auto &leaves) {
    return std::remove_cvref_t<decltype(leaves)>(
        leaves.begin() + static_cast<std::ptrdiff_t>(from),
        leaves.begin() + static_cast<std::ptrdiff_t>(to));
  };
  switch (type) {
  case ValueType::Int:
    r.ints = leaves_of(ints);
    break;
  case ValueType::Float:
    r.floats = leaves_of(floats);
    break;
  case ValueType::String:
    r.strings = leaves_of(strings);
    break;
  }
  return r;
}

Column::Column(ValueType t, std::size_t depth) : c_(t), d_(depth) {}

void Column::start_levels() { c_.levels.assign(d_ + 1, Seq<std::size_t>{0}); }

void Column::skip() {
  rows_++;
  missing_ = true;
  if (!started_)
    pending_++;
  else if (d_ == 0)
    throw ShapeError{.code = ErrorCode::RepeatMissing,
                     .msg = "is defined in only some repetitions"};
  else
    c_.levels[1].push_back(c_.levels[1].back());
}

void Column::add(Value &v) {
  rows_++;
  if (!started_) {
    if (missing_ && (v.depth() == 0))
      throw ShapeError{.code = ErrorCode::RepeatMissing,
                       .msg = "is defined in only some repetitions"};
    started_ = true;
    d_ = v.depth();
    c_.type = v.type;
    start_levels();
    for (std::size_t i = 0; i < pending_; i++)
      c_.levels[1].push_back(0);
  } else if (v.depth() != d_) {
    throw ShapeError{.code = ErrorCode::RepeatDepth,
                     .msg = "has rows of different depths"};
  }
  if (v.leaves() > 0) {
    // The type follows the values; integers become floats if needed
    if (c_.leaves() == 0)
      c_.type = v.type;
    else if ((c_.type == ValueType::String) != (v.type == ValueType::String))
      throw ShapeError{.code = ErrorCode::ValueTypeMix,
                       .msg = "mixes strings and numbers"};
    else if ((c_.type == ValueType::Int) && (v.type == ValueType::Float))
      c_.to_float();
  }
  // The lists of the row follow those of the earlier rows one level down
  for (std::size_t k = 0; k < d_; k++) {
    Seq<std::size_t> &offsets = c_.levels[k + 1];
    const std::size_t base = offsets.back();
    for (std::size_t j = 1; j < v.levels[k].size(); j++)
      offsets.push_back(base + v.levels[k][j]);
  }
  switch (c_.type) {
  case ValueType::Int:
    c_.ints.insert(c_.ints.end(), v.ints.begin(), v.ints.end());
    break;
  case ValueType::Float:
    for (const std::int64_t x : v.ints)
      c_.floats.push_back(static_cast<double>(x));
    c_.floats.insert(c_.floats.end(), v.floats.begin(), v.floats.end());
    break;
  case ValueType::String:
    for (String &x : v.strings)
      c_.strings.push_back(std::move(x));
    break;
  }
}

Value Column::finish() {
  if (!started_) {
    // No rows added: every row is missing, so empty if it is a list
    if ((d_ == 0) && (rows_ > 0))
      throw ShapeError{.code = ErrorCode::RepeatMissing,
                       .msg = "is defined in no repetition"};
    start_levels();
    for (std::size_t i = 0; (d_ > 0) && (i < rows_); i++)
      c_.levels[1].push_back(0);
  }
  c_.levels[0].assign({0, rows_});
  return std::move(c_);
}

} // namespace detail

/*
 * Data access
 */

std::string_view type_name(ValueType type) {
  switch (type) {
  case ValueType::Int:
    return "int";
  case ValueType::Float:
    return "float";
  case ValueType::String:
    return "string";
  }
  std::unreachable();
}

ValueType ValueRef::type() const { return value_->type; }

std::size_t ValueRef::depth() const { return value_->depth() - level_; }

void ValueRef::require_array() const {
  if (depth() == 0)
    throw AccessError("the value is not an array");
}

void ValueRef::require_single(ValueType type) const {
  if (depth() != 0)
    throw AccessError("the value is an array, not a single value");
  if (value_->type != type)
    throw AccessError("the value is of type " +
                      std::string(type_name(value_->type)) + ", not " +
                      std::string(type_name(type)));
}

void ValueRef::require_row(ValueType type) const {
  if (depth() != 1)
    throw AccessError("the value is not a one-dimensional array");
  if (value_->type != type)
    throw AccessError("the array is of type " +
                      std::string(type_name(value_->type)) + ", not " +
                      std::string(type_name(type)));
}

std::size_t ValueRef::size() const {
  require_array();
  return value_->list_size(level_, pos_);
}

ValueRef ValueRef::operator[](std::size_t i) const {
  const std::size_t n = size();
  if (i >= n)
    throw AccessError("index " + std::to_string(i) + " is out of range (size " +
                      std::to_string(n) + ")");
  return {*value_, level_ + 1, value_->levels[level_][pos_] + i};
}

ValueRef ValueRef::at(std::span<const std::size_t> path) const {
  ValueRef r = *this;
  for (const std::size_t i : path)
    r = r[i];
  return r;
}

std::int64_t ValueRef::integer() const {
  require_single(ValueType::Int);
  return value_->ints[pos_];
}

double ValueRef::number() const {
  if (value_->type == ValueType::Int)
    return static_cast<double>(integer());
  require_single(ValueType::Float);
  return value_->floats[pos_];
}

std::string_view ValueRef::text() const {
  require_single(ValueType::String);
  return value_->strings[pos_];
}

std::span<const std::int64_t> ValueRef::ints() const {
  require_row(ValueType::Int);
  const auto [from, to] = value_->range({.level = level_, .pos = pos_});
  return std::span<const std::int64_t>(value_->ints).subspan(from, to - from);
}

std::span<const double> ValueRef::floats() const {
  require_row(ValueType::Float);
  const auto [from, to] = value_->range({.level = level_, .pos = pos_});
  return std::span<const double>(value_->floats).subspan(from, to - from);
}

Data::Data() : impl_(std::make_shared<const detail::DataImpl>()) {}

Data::Data(std::shared_ptr<const detail::DataImpl> impl)
    : impl_(std::move(impl)) {}

std::size_t Data::size() const { return impl_->entries.size(); }

std::string_view Data::name(std::size_t i) const {
  if (i >= size())
    throw AccessError("entry index " + std::to_string(i) + " is out of range");
  return impl_->entries[i].name;
}

ValueRef Data::value(std::size_t i) const {
  if (i >= size())
    throw AccessError("entry index " + std::to_string(i) + " is out of range");
  return {impl_->entries[i].value, 0, 0};
}

std::optional<ValueRef> Data::find(std::string_view name) const {
  const detail::Value *v = impl_->find(name);
  if (v == nullptr)
    return std::nullopt;
  return ValueRef(*v, 0, 0);
}

ValueRef Data::at(std::string_view name,
                  std::span<const std::size_t> path) const {
  const std::optional<ValueRef> v = find(name);
  if (!v)
    throw AccessError("no value named '" + std::string(name) + "'");
  return v->at(path);
}

std::size_t Data::size(std::string_view name,
                       std::span<const std::size_t> path) const {
  return at(name, path).size();
}

std::int64_t Data::integer(std::string_view name,
                           std::span<const std::size_t> path) const {
  return at(name, path).integer();
}

double Data::number(std::string_view name,
                    std::span<const std::size_t> path) const {
  return at(name, path).number();
}

std::string_view Data::text(std::string_view name,
                            std::span<const std::size_t> path) const {
  return at(name, path).text();
}

std::span<const std::int64_t>
Data::ints(std::string_view name, std::span<const std::size_t> path) const {
  return at(name, path).ints();
}

std::span<const double> Data::floats(std::string_view name,
                                     std::span<const std::size_t> path) const {
  return at(name, path).floats();
}

/*
 * Parameters
 */

Params::Params() : impl_(std::make_shared<detail::ParamsImpl>()) {}

Params::Params(std::initializer_list<std::pair<std::string, std::string>> pairs)
    : Params() {
  for (const auto &[name, value] : pairs)
    set(name, value);
}

void Params::set(std::string name, std::string value) {
  if (impl_.use_count() > 1)
    impl_ = std::make_shared<detail::ParamsImpl>(*impl_);
  for (std::size_t i = 0; i < impl_->names.size(); i++)
    if (impl_->names[i] == name) {
      impl_->values[i] = std::move(value);
      return;
    }
  impl_->names.push_back(std::move(name));
  impl_->values.push_back(std::move(value));
}

std::size_t Params::size() const { return impl_->names.size(); }

std::string_view Params::name(std::size_t i) const {
  if (i >= size())
    throw AccessError("parameter index " + std::to_string(i) +
                      " is out of range");
  return impl_->names[i];
}

std::optional<std::string_view> Params::get(std::string_view name) const {
  return impl_->get(name);
}

} // namespace iris
