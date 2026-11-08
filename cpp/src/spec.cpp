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

#include <iris/spec.hpp>

#include "ast.hpp"
#include "call_sites.hpp"
#include "lexer.hpp"
#include "numbers.hpp"
#include "parser.hpp"
#include "reader.hpp"
#include "text.hpp"
#include "value.hpp"

#include <algorithm>
#include <cerrno>
#include <cstddef>
#include <cstdio>
#include <memory>
#include <optional>
#include <string>
#include <system_error>
#include <utility>

#ifdef _WIN32
#include <share.h>
#endif

namespace iris {

namespace {

/// Throw E-SPEC-UTF8 if reader spec `text` from `origin` is not valid UTF-8
/// (Specification §5.1)
void check_utf8(std::string_view text, std::string_view origin) {
  const std::optional<std::size_t> bad = detail::find_invalid_utf8(text);
  if (!bad)
    return;
  const std::string_view before = text.substr(0, *bad);
  const std::size_t last_lf = before.rfind('\n');
  const std::size_t line_start =
      (last_lf == std::string_view::npos) ? 0 : last_lf + 1;
  throw Error(ErrorCode::SpecUtf8, "reader spec is not valid UTF-8",
              {.spec_location =
                   Location{.origin = std::string(origin),
                            .line = 1 + static_cast<std::size_t>(
                                            std::ranges::count(before, '\n')),
                            .column = 1 + *bad - line_start}});
}

/// Return the bytes of the file at `path`; throws std::system_error if it
/// cannot be read
///
/// The C library opens the file, without a buffer, so that the bytes are
/// the only memory a read takes besides the FILE.
detail::String read_file(const std::filesystem::path &path) {
  const auto fail = [&path](std::string_view what) {
    throw std::system_error(errno, std::generic_category(),
                            std::string(what) + ' ' + path.string());
  };
#ifdef _WIN32
  // _wfsopen, as MSVC deprecates _wfopen, and _wfopen_s denies other
  // readers of the file.
  std::FILE *file = _wfsopen(path.c_str(), L"rb", _SH_DENYNO);
#else
  std::FILE *file = std::fopen(path.c_str(), "rb");
#endif
  if (file == nullptr)
    fail("cannot open");
  const std::unique_ptr<std::FILE, int (*)(std::FILE *)> closer(file,
                                                                &std::fclose);
  std::setvbuf(file, nullptr, _IONBF, 0);
  static constexpr std::size_t chunk = std::size_t{1} << 16;
  detail::String bytes;
  for (;;) {
    const std::size_t old_size = bytes.size();
    bytes.resize(old_size + chunk);
    const std::size_t n = std::fread(bytes.data() + old_size, 1, chunk, file);
    bytes.resize(old_size + n);
    if (n < chunk)
      break;
  }
  if (std::ferror(file) != 0)
    fail("cannot read");
  return bytes;
}

/// Return `path` as the origin of its text in errors
detail::String origin_of(const std::filesystem::path &path) {
  return path.string<char, std::char_traits<char>, detail::Allocator<char>>(
      detail::Allocator<char>());
}

/// Return the result of `operation`, or the Error it throws
template <class F>
auto expected(F operation) -> std::expected<decltype(operation()), Error> {
  try {
    return operation();
  } catch (const Error &error) {
    return std::unexpected(error);
  }
}

/// Return the result of `operation`, and throw the Error it throws with the
/// call site from `sites` of the statement the error belongs to
template <class F>
auto locating(const detail::CallSites &sites, F operation)
    -> decltype(operation()) {
  try {
    return operation();
  } catch (const Error &error) {
    if (sites.empty())
      throw;
    throw sites.locate(error);
  }
}

/// Parse and check the reader spec `text` from `origin`, allocating from
/// `memory`, and keep a copy of the call sites `sites` of its lines, if
/// any, which is allocated from `memory` too. The empty call sites of a
/// spec without them are made inside the scope as well, since a standard
/// library may allocate for an empty container.
std::shared_ptr<const detail::SpecImpl>
parse_spec(std::string_view text, std::string_view origin,
           std::pmr::memory_resource &memory, const detail::CallSites *sites) {
  const detail::MemoryScope scope(memory);
  const detail::NearestRounding rounding;
  detail::String name(origin);
  check_utf8(text, name);
  detail::ParsedSpec parsed =
      detail::parse_tokens(detail::lex(text, name), name);
  detail::resolve(parsed.body, name);
  return detail::make_shared<const detail::SpecImpl>(detail::SpecImpl{
      .memory = &memory,
      .origin = std::move(name),
      .name = std::move(parsed.name),
      .body = std::move(parsed.body),
      // A copy takes the resource of the scope
      .call_sites = (sites != nullptr) ? detail::CallSites(*sites)
                                       : detail::CallSites()});
}

} // namespace

namespace detail {

std::expected<Spec, Error> SpecAccess::try_parse(std::string_view text,
                                                 std::string_view origin,
                                                 CallSites sites) {
  return expected([&] {
    return Spec(
        parse_spec(text, origin, *std::pmr::get_default_resource(), &sites));
  });
}

const CallSites &SpecAccess::call_sites(const Spec &spec) {
  return spec.impl_->call_sites;
}

} // namespace detail

Spec::Spec(std::shared_ptr<const detail::SpecImpl> impl)
    : impl_(std::move(impl)) {}

Spec Spec::parse(std::string_view text, std::string_view origin,
                 std::pmr::memory_resource &memory) {
  return Spec(parse_spec(text, origin, memory, nullptr));
}

std::expected<Spec, Error> Spec::try_parse(std::string_view text,
                                           std::string_view origin,
                                           std::pmr::memory_resource &memory) {
  return expected([&] { return parse(text, origin, memory); });
}

Spec Spec::load(const std::filesystem::path &path,
                std::pmr::memory_resource &memory) {
  const detail::MemoryScope scope(memory);
  return parse(read_file(path), origin_of(path), memory);
}

std::expected<Spec, Error> Spec::try_load(const std::filesystem::path &path,
                                          std::pmr::memory_resource &memory) {
  const detail::MemoryScope scope(memory);
  const detail::String text = read_file(path);
  return try_parse(text, origin_of(path), memory);
}

std::string_view Spec::name() const { return impl_->name; }

Data Spec::read(const std::filesystem::path &path, const Params &params) const {
  const detail::MemoryScope scope(*impl_->memory);
  return read_text(read_file(path), origin_of(path), params);
}

Data Spec::read_text(std::string_view text, std::string_view origin,
                     const Params &params) const {
  const detail::MemoryScope scope(*impl_->memory);
  const detail::NearestRounding rounding;
  return locating(impl_->call_sites, [&] {
    return Data(detail::make_shared<const detail::DataImpl>(
        detail::read(*impl_, text, detail::String(origin), params.values())));
  });
}

std::expected<Data, Error> Spec::try_read(const std::filesystem::path &path,
                                          const Params &params) const {
  const detail::MemoryScope scope(*impl_->memory);
  return try_read_text(read_file(path), origin_of(path), params);
}

std::expected<Data, Error> Spec::try_read_text(std::string_view text,
                                               std::string_view origin,
                                               const Params &params) const {
  return expected([&] { return read_text(text, origin, params); });
}

Data Spec::catalog(const std::filesystem::path &path,
                   const Params &params) const {
  const detail::MemoryScope scope(*impl_->memory);
  return catalog_text(read_file(path), origin_of(path), params);
}

Data Spec::catalog_text(std::string_view text, std::string_view origin,
                        const Params &params) const {
  const detail::MemoryScope scope(*impl_->memory);
  const detail::NearestRounding rounding;
  return locating(impl_->call_sites, [&] {
    return Data(detail::make_shared<const detail::DataImpl>(detail::catalog(
        *impl_, text, detail::String(origin), params.values())));
  });
}

std::expected<Data, Error> Spec::try_catalog(const std::filesystem::path &path,
                                             const Params &params) const {
  const detail::MemoryScope scope(*impl_->memory);
  return try_catalog_text(read_file(path), origin_of(path), params);
}

std::expected<Data, Error> Spec::try_catalog_text(std::string_view text,
                                                  std::string_view origin,
                                                  const Params &params) const {
  return expected([&] { return catalog_text(text, origin, params); });
}

} // namespace iris
