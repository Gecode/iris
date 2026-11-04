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

#include <iris/spec.hpp>

#include "ast.hpp"
#include "lexer.hpp"
#include "numbers.hpp"
#include "parser.hpp"
#include "reader.hpp"
#include "text.hpp"
#include "value.hpp"

#include <algorithm>
#include <cerrno>
#include <cstddef>
#include <fstream>
#include <iterator>
#include <optional>
#include <system_error>
#include <utility>

namespace iris {

namespace {

/// Throw E-SPEC-UTF8 if reader spec `text` from `origin` is not valid UTF-8
/// (Specification §5.1)
void check_utf8(std::string_view text, const std::string &origin) {
  const std::optional<std::size_t> bad = detail::find_invalid_utf8(text);
  if (!bad)
    return;
  const std::string_view before = text.substr(0, *bad);
  const std::size_t last_lf = before.rfind('\n');
  const std::size_t line_start =
      (last_lf == std::string_view::npos) ? 0 : last_lf + 1;
  throw Error(ErrorCode::SpecUtf8, "reader spec is not valid UTF-8",
              std::nullopt,
              Location{.origin = origin,
                       .line = 1 + static_cast<std::size_t>(
                                       std::ranges::count(before, '\n')),
                       .column = 1 + *bad - line_start});
}

/// Return the bytes of the file at `path`; throws std::system_error if it
/// cannot be read
std::string read_file(const std::filesystem::path &path) {
  std::ifstream file(path, std::ios::binary);
  if (!file)
    throw std::system_error(errno, std::generic_category(),
                            "cannot open " + path.string());
  std::string bytes{std::istreambuf_iterator<char>(file), {}};
  if (file.bad())
    throw std::system_error(errno, std::generic_category(),
                            "cannot read " + path.string());
  return bytes;
}

} // namespace

Spec::Spec(std::shared_ptr<const detail::SpecImpl> impl)
    : impl_(std::move(impl)) {}

Spec Spec::parse(std::string_view text, std::string origin) {
  const detail::NearestRounding rounding;
  check_utf8(text, origin);
  detail::ParsedSpec parsed =
      detail::parse_tokens(detail::lex(text, origin), origin);
  detail::resolve(parsed.body, origin);
  return Spec(std::make_shared<const detail::SpecImpl>(
      detail::SpecImpl{.origin = std::move(origin),
                       .name = std::move(parsed.name),
                       .body = std::move(parsed.body)}));
}

std::expected<Spec, Error> Spec::try_parse(std::string_view text,
                                           std::string origin) {
  try {
    return parse(text, std::move(origin));
  } catch (const Error &error) {
    return std::unexpected(error);
  }
}

const std::string &Spec::name() const { return impl_->name; }

Data Spec::read(const std::filesystem::path &path, const Params &params) const {
  return read_text(read_file(path), path.string(), params);
}

Data Spec::read_text(std::string_view text, std::string origin,
                     const Params &params) const {
  const detail::NearestRounding rounding;
  return Data(std::make_shared<const detail::DataImpl>(
      detail::read(*impl_, text, std::move(origin), *params.impl_)));
}

std::expected<Data, Error> Spec::try_read(const std::filesystem::path &path,
                                          const Params &params) const {
  return try_read_text(read_file(path), path.string(), params);
}

std::expected<Data, Error> Spec::try_read_text(std::string_view text,
                                               std::string origin,
                                               const Params &params) const {
  try {
    return read_text(text, std::move(origin), params);
  } catch (const Error &error) {
    return std::unexpected(error);
  }
}

Data Spec::catalog(const std::filesystem::path &path,
                   const Params &params) const {
  return catalog_text(read_file(path), path.string(), params);
}

Data Spec::catalog_text(std::string_view text, std::string origin,
                        const Params &params) const {
  const detail::NearestRounding rounding;
  return Data(std::make_shared<const detail::DataImpl>(
      detail::catalog(*impl_, text, std::move(origin), *params.impl_)));
}

std::expected<Data, Error> Spec::try_catalog(const std::filesystem::path &path,
                                             const Params &params) const {
  return try_catalog_text(read_file(path), path.string(), params);
}

std::expected<Data, Error> Spec::try_catalog_text(std::string_view text,
                                                  std::string origin,
                                                  const Params &params) const {
  try {
    return catalog_text(text, std::move(origin), params);
  } catch (const Error &error) {
    return std::unexpected(error);
  }
}

} // namespace iris
