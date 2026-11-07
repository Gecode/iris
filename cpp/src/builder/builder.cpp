// The builder's entry points: printing, parsing, and what they report.

#include "state.hpp"

#include "../call_sites.hpp"

#include <cstddef>
#include <cstdint>
#include <optional>
#include <source_location>
#include <string>
#include <utility>
#include <vector>

namespace iris::build {

namespace {

/// The origin reported for the text a builder emits
constexpr std::string_view origin = "builder.iris";

/// Return line `number` of `text`, counted from 1
std::optional<std::string_view> line_of(std::string_view text,
                                        std::size_t number) {
  std::size_t start = 0;
  for (std::size_t line = 1; line < number; line++) {
    const std::size_t newline = text.find('\n', start);
    if (newline == std::string_view::npos)
      return std::nullopt;
    start = newline + 1;
  }
  if (start >= text.size())
    return std::nullopt;
  const std::size_t end = text.find('\n', start);
  return text.substr(start, end == std::string_view::npos ? text.size() - start
                                                          : end - start);
}

std::string render(const std::string &message,
                   const std::optional<Error> &language_error,
                   const std::optional<std::string> &text,
                   const std::optional<std::source_location> &call_site) {
  std::string out;
  if (language_error) {
    out += code_name(language_error->code());
    out += ": ";
  }
  out += message;
  if (language_error && text) {
    const std::optional<Location> &location = language_error->spec_location();
    if (location && location->line) {
      if (const std::optional<std::string_view> line =
              line_of(*text, *location->line)) {
        std::string number = std::to_string(*location->line);
        if (number.size() < 6)
          number.insert(0, 6 - number.size(), ' ');
        out += "\n" + number + " | " + iris::detail::escape_controls(*line);
      }
    }
  }
  if (call_site)
    out += "\ncall site: " + iris::detail::site_text(*call_site);
  return out;
}

/// The call site of each line of the emitted text: the first statement
/// that starts on the line and is not a `}`, else the first that starts
/// there, else the statement that holds the line, as for an `each` line
iris::detail::CallSites line_sites(const detail::Printed &printed) {
  iris::detail::Seq<std::optional<std::source_location>> lines;
  const std::vector<detail::Printed::Entry> &entries = printed.sites;
  std::optional<std::source_location> holder;
  std::size_t next = 0;
  for (std::size_t start = 0; start < printed.text.size();) {
    const std::size_t newline = printed.text.find('\n', start);
    const std::size_t end =
        newline == std::string::npos ? printed.text.size() : newline + 1;
    for (; next < entries.size() && entries[next].offset < start; next++)
      holder = entries[next].site;
    std::optional<std::source_location> statement;
    std::optional<std::source_location> close;
    for (std::size_t k = next; k < entries.size() && entries[k].offset < end;
         k++) {
      if (!entries[k].close && !statement)
        statement = entries[k].site;
      if (entries[k].close && !close)
        close = entries[k].site;
    }
    lines.push_back(statement ? statement : close ? close : holder);
    start = end;
  }
  return iris::detail::CallSites(std::move(lines));
}

BuildError issue_error(detail::Issue issue) {
  return detail::Access::error(issue.kind, std::move(issue.message),
                               std::nullopt, std::nullopt, issue.site);
}

std::expected<detail::Printed, BuildError> printed(const detail::State &state) {
  if (state.issue)
    return std::unexpected(issue_error(*state.issue));
  if (std::optional<detail::Issue> issue = detail::check_bindings(state))
    return std::unexpected(issue_error(std::move(*issue)));
  std::expected<detail::Printed, detail::Issue> result = detail::print(state);
  if (!result)
    return std::unexpected(issue_error(std::move(result.error())));
  return std::move(*result);
}

} // namespace

BuildError::BuildError(BuildErrorKind kind, std::string message,
                       std::optional<Error> language_error,
                       std::optional<std::string> text,
                       std::optional<std::source_location> call_site)
    : std::runtime_error(render(message, language_error, text, call_site)),
      kind_(kind), message_(std::move(message)),
      language_error_(std::move(language_error)), text_(std::move(text)),
      call_site_(call_site) {}

std::optional<std::string_view> BuildError::emitted_line() const {
  if (!text_ || !language_error_)
    return std::nullopt;
  const std::optional<Location> &location = language_error_->spec_location();
  if (!location || !location->line)
    return std::nullopt;
  return line_of(*text_, *location->line);
}

namespace detail {

std::expected<std::string, BuildError> try_source(const State &state) {
  std::expected<Printed, BuildError> result = printed(state);
  if (!result)
    return std::unexpected(std::move(result.error()));
  return std::move(result->text);
}

std::expected<Spec, BuildError> try_build(const State &state) {
  std::expected<Printed, BuildError> result = printed(state);
  if (!result)
    return std::unexpected(std::move(result.error()));
  const iris::detail::CallSites sites = line_sites(*result);
  std::expected<Spec, Error> spec =
      iris::detail::SpecAccess::try_parse(result->text, origin, sites);
  if (spec)
    return std::move(*spec);
  // The message is the error without its call site, which the builder
  // error shows after the emitted line
  const Error located = sites.locate(spec.error());
  const std::optional<std::source_location> call_site = located.call_site();
  return std::unexpected(Access::error(BuildErrorKind::Spec,
                                       spec.error().what(), located,
                                       std::move(result->text), call_site));
}

} // namespace detail

Alphabet Alphabet::blank(std::string_view characters) const {
  Alphabet result = *this;
  result.options_.push_back({.kind = detail::AlphabetOption::Kind::Blank,
                             .text = std::string(characters),
                             .base = 0});
  return result;
}

Alphabet Alphabet::strip(std::string_view characters) const {
  Alphabet result = *this;
  result.options_.push_back({.kind = detail::AlphabetOption::Kind::Strip,
                             .text = std::string(characters),
                             .base = 0});
  return result;
}

Alphabet Alphabet::base(std::int64_t value) const {
  Alphabet result = *this;
  result.options_.push_back(
      {.kind = detail::AlphabetOption::Kind::Base, .text = {}, .base = value});
  return result;
}

Pattern &Pattern::text(std::string_view text) & {
  detail::append_text(pieces_, text);
  return *this;
}

Pattern &&Pattern::text(std::string_view text) && {
  detail::append_text(pieces_, text);
  return std::move(*this);
}

SpecBuilder::SpecBuilder()
    : Scope(detail::new_state(), detail::root_block, {}) {}

SpecBuilder::SpecBuilder(std::string_view format, std::source_location site)
    : SpecBuilder() {
  this->format(format, site);
}

void SpecBuilder::format(std::string_view name, std::source_location site) {
  detail::simple(*state_, block_, detail::Simple::Format, {std::string(name)},
                 site);
}

void SpecBuilder::comment(const Strings &markers, std::source_location site) {
  detail::simple(*state_, block_, detail::Simple::Comment, markers.values(),
                 site);
}

void SpecBuilder::stop(const Strings &markers, std::source_location site) {
  detail::simple(*state_, block_, detail::Simple::Stop, markers.values(), site);
}

void SpecBuilder::tokens(std::string_view characters,
                         std::source_location site) {
  detail::simple(*state_, block_, detail::Simple::Tokens,
                 {std::string(characters)}, site);
}

void SpecBuilder::separators(std::string_view characters,
                             std::source_location site) {
  detail::simple(*state_, block_, detail::Simple::Separators,
                 {std::string(characters)}, site);
}

void SpecBuilder::ignore_trailing(std::source_location site) {
  detail::simple(*state_, block_, detail::Simple::IgnoreTrailing, {}, site);
}

std::string SpecBuilder::source() const {
  std::expected<std::string, BuildError> result = try_source();
  if (!result)
    throw std::move(result.error());
  return std::move(*result);
}

std::expected<std::string, BuildError> SpecBuilder::try_source() const {
  return detail::try_source(*state_);
}

Spec SpecBuilder::build() const {
  std::expected<Spec, BuildError> result = try_build();
  if (!result)
    throw std::move(result.error());
  return std::move(*result);
}

std::expected<Spec, BuildError> SpecBuilder::try_build() const {
  return detail::try_build(*state_);
}

} // namespace iris::build
