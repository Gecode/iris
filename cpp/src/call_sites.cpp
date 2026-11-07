// Where the statements of a reader spec from a spec builder came from in the
// host program, and the errors that name them.

#include "call_sites.hpp"

#include <iris/error.hpp>

#include <cstddef>
#include <optional>
#include <source_location>
#include <string>
#include <string_view>
#include <utility>

namespace iris::detail {

std::string escape_controls(std::string_view text) {
  std::string escaped;
  escaped.reserve(text.size());
  const auto hex = [&](unsigned value) {
    constexpr std::string_view digits = "0123456789abcdef";
    escaped += "\\u{";
    if (value >= 16)
      escaped.push_back(digits[value / 16]);
    escaped.push_back(digits[value % 16]);
    escaped.push_back('}');
  };
  for (std::size_t i = 0; i < text.size(); i++) {
    const auto byte = static_cast<unsigned char>(text[i]);
    if (byte == '\t')
      escaped += "\\t";
    else if (byte == '\n')
      escaped += "\\n";
    else if (byte == '\r')
      escaped += "\\r";
    else if (byte < 0x20 || byte == 0x7f)
      hex(byte);
    else if (byte == 0xc2 && i + 1 < text.size() &&
             static_cast<unsigned char>(text[i + 1]) >= 0x80 &&
             static_cast<unsigned char>(text[i + 1]) <= 0x9f) {
      // A C1 control character, U+0080 to U+009F.
      hex(static_cast<unsigned char>(text[i + 1]));
      i++;
    } else
      escaped.push_back(text[i]);
  }
  return escaped;
}

std::string site_text(const std::source_location &site) {
  return escape_controls(std::string(site.file_name()) + ":" +
                         std::to_string(site.line()) + ":" +
                         std::to_string(site.column()));
}

Error CallSites::locate(const Error &error) const {
  std::optional<std::size_t> line = ErrorAccess::owner_line(error);
  if (!line && error.spec_location())
    line = error.spec_location()->line;
  const std::optional<std::source_location> site =
      line ? at(*line) : std::nullopt;
  if (!site)
    return error;
  ErrorDetails details = error.details();
  details.call_site = site;
  Error located(error.code(), error.message(), std::move(details));
  ErrorAccess::set_owner_line(located, ErrorAccess::owner_line(error));
  return located;
}

} // namespace iris::detail
