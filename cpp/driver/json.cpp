#include "json.hpp"

#include <array>
#include <charconv>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <set>
#include <system_error>

namespace iris::driver {

namespace {

/// Return the length of the valid UTF-8 character at position `i` of `text`,
/// or 0 if there is none
std::size_t character_length(std::string_view text, std::size_t i) {
  const auto first = static_cast<unsigned char>(text[i]);
  if (first < 0x80)
    return 1;
  std::uint32_t scalar = 0;
  std::size_t following = 0;
  std::uint32_t minimum = 0;
  if ((first >= 0xc2) && (first <= 0xdf)) {
    scalar = first & 0x1fU;
    following = 1;
    minimum = 0x80;
  } else if ((first >= 0xe0) && (first <= 0xef)) {
    scalar = first & 0x0fU;
    following = 2;
    minimum = 0x800;
  } else if ((first >= 0xf0) && (first <= 0xf4)) {
    scalar = first & 0x07U;
    following = 3;
    minimum = 0x10000;
  } else {
    return 0;
  }
  if (text.size() - i <= following)
    return 0;
  for (std::size_t j = 1; j <= following; ++j) {
    const auto c = static_cast<unsigned char>(text[i + j]);
    if ((c & 0xc0U) != 0x80U)
      return 0;
    scalar = (scalar << 6U) | (c & 0x3fU);
  }
  if ((scalar < minimum) || (scalar > 0x10ffff) ||
      ((scalar >= 0xd800) && (scalar <= 0xdfff)))
    return 0;
  return following + 1;
}

/// Test whether `text` is valid UTF-8
bool valid_utf8(std::string_view text) {
  for (std::size_t i = 0; i < text.size();) {
    const std::size_t n = character_length(text, i);
    if (n == 0)
      return false;
    i += n;
  }
  return true;
}

/// Return `text` with each byte that does not start a valid UTF-8 character
/// replaced by U+FFFD
std::string repair_utf8(std::string_view text) {
  std::string out;
  for (std::size_t i = 0; i < text.size();) {
    const std::size_t n = character_length(text, i);
    if (n == 0) {
      out += "\xef\xbf\xbd";
      ++i;
    } else {
      out += text.substr(i, n);
      i += n;
    }
  }
  return out;
}

/// Append the UTF-8 encoding of Unicode scalar value `scalar` to `out`
void append_utf8(std::string &out, std::uint32_t scalar) {
  const auto byte = [&out](std::uint32_t b) {
    out += static_cast<char>(static_cast<unsigned char>(b));
  };
  if (scalar < 0x80) {
    byte(scalar);
  } else if (scalar < 0x800) {
    byte(0xc0U | (scalar >> 6U));
    byte(0x80U | (scalar & 0x3fU));
  } else if (scalar < 0x10000) {
    byte(0xe0U | (scalar >> 12U));
    byte(0x80U | ((scalar >> 6U) & 0x3fU));
    byte(0x80U | (scalar & 0x3fU));
  } else {
    byte(0xf0U | (scalar >> 18U));
    byte(0x80U | ((scalar >> 12U) & 0x3fU));
    byte(0x80U | ((scalar >> 6U) & 0x3fU));
    byte(0x80U | (scalar & 0x3fU));
  }
}

/// Return the value of hexadecimal digit `c`, or nothing if it is not one
std::optional<std::uint32_t> hex_digit(char c) {
  if ((c >= '0') && (c <= '9'))
    return static_cast<std::uint32_t>(c - '0');
  if ((c >= 'a') && (c <= 'f'))
    return static_cast<std::uint32_t>(c - 'a' + 10);
  if ((c >= 'A') && (c <= 'F'))
    return static_cast<std::uint32_t>(c - 'A' + 10);
  return std::nullopt;
}

/// Parser for the flat string map that the driver protocol allows
class ParamsParser {
public:
  explicit ParamsParser(std::string_view json) : text_(json) {}

  ParamPairs parse() {
    if (!valid_utf8(text_))
      fail();
    expect('{');
    ParamPairs pairs;
    std::set<std::string> names;
    if (!take('}')) {
      do {
        std::string name = string();
        expect(':');
        std::string value = string();
        if (!names.insert(name).second)
          fail();
        pairs.emplace_back(std::move(name), std::move(value));
      } while (take(','));
      expect('}');
    }
    whitespace();
    if (pos_ != text_.size())
      fail();
    return pairs;
  }

private:
  std::string_view text_;
  std::size_t pos_ = 0;

  [[noreturn]] static void fail() {
    throw Misuse("params must be one JSON object with unique string keys and "
                 "string values");
  }
  void whitespace() {
    while ((pos_ < text_.size()) &&
           ((text_[pos_] == ' ') || (text_[pos_] == '\t') ||
            (text_[pos_] == '\r') || (text_[pos_] == '\n')))
      ++pos_;
  }
  bool take(char c) {
    whitespace();
    if ((pos_ == text_.size()) || (text_[pos_] != c))
      return false;
    ++pos_;
    return true;
  }
  void expect(char c) {
    if (!take(c))
      fail();
  }
  std::uint32_t hex_quad() {
    std::uint32_t value = 0;
    for (int i = 0; i < 4; ++i) {
      if (pos_ == text_.size())
        fail();
      const std::optional<std::uint32_t> digit = hex_digit(text_[pos_++]);
      if (!digit)
        fail();
      value = (value * 16) + *digit;
    }
    return value;
  }
  /// The Unicode escape after "\u", including a surrogate pair
  void unicode(std::string &out) {
    std::uint32_t scalar = hex_quad();
    if ((scalar >= 0xd800) && (scalar <= 0xdbff)) {
      if (text_.substr(pos_, 2) != "\\u")
        fail();
      pos_ += 2;
      const std::uint32_t low = hex_quad();
      if ((low < 0xdc00) || (low > 0xdfff))
        fail();
      scalar = 0x10000 + ((scalar - 0xd800) << 10U) + (low - 0xdc00);
    } else if ((scalar >= 0xdc00) && (scalar <= 0xdfff)) {
      fail();
    }
    append_utf8(out, scalar);
  }
  std::string string() {
    expect('"');
    std::string out;
    while (pos_ < text_.size()) {
      const char c = text_[pos_++];
      if (c == '"')
        return out;
      if (static_cast<unsigned char>(c) < 0x20)
        fail();
      if (c != '\\') {
        out += c;
        continue;
      }
      if (pos_ == text_.size())
        fail();
      switch (text_[pos_++]) {
      case '"':
        out += '"';
        break;
      case '\\':
        out += '\\';
        break;
      case '/':
        out += '/';
        break;
      case 'b':
        out += '\b';
        break;
      case 'f':
        out += '\f';
        break;
      case 'n':
        out += '\n';
        break;
      case 'r':
        out += '\r';
        break;
      case 't':
        out += '\t';
        break;
      case 'u':
        unicode(out);
        break;
      default:
        fail();
      }
    }
    fail();
  }
};

std::string encode_location(const std::optional<Location> &location) {
  if (!location)
    return "null";
  const auto number = [](const std::optional<std::size_t> &n) {
    return n ? std::to_string(*n) : std::string("null");
  };
  return "{\"origin\":" + quote_json(repair_utf8(location->origin)) +
         ",\"line\":" + number(location->line) +
         ",\"column\":" + number(location->column) + "}";
}

/// Append the JSON of `value` to `out`
void append_value(std::string &out, const ValueRef &value) {
  if (value.depth() == 0) {
    switch (value.type()) {
    case ValueType::Int:
      out += std::to_string(value.integer());
      return;
    case ValueType::Float:
      out += encode_float(value.number());
      return;
    case ValueType::String:
      out += quote_json(value.text());
      return;
    }
  }
  out += '[';
  for (std::size_t i = 0; i < value.size(); ++i) {
    if (i > 0)
      out += ',';
    append_value(out, value[i]);
  }
  out += ']';
}

} // namespace

std::string encode_float(double x) {
  if (x == 0)
    return std::signbit(x) ? "-0.0" : "0";
  // The shortest digits that round-trip, as d.ddde[-]x
  std::array<char, 64> buffer{};
  const auto [end, ec] =
      std::to_chars(buffer.data(), buffer.data() + buffer.size(), x,
                    std::chars_format::scientific);
  if (ec != std::errc())
    throw std::runtime_error("cannot format a float");
  const std::string_view text(buffer.data(), end);
  const std::size_t e = text.find('e');
  std::string_view mantissa = text.substr(0, e);
  std::string sign;
  if (mantissa.starts_with('-')) {
    sign = "-";
    mantissa.remove_prefix(1);
  }
  std::string digits;
  for (const char c : mantissa)
    if (c != '.')
      digits += c;
  std::string_view exponent_text = text.substr(e + 1);
  if (exponent_text.starts_with('+'))
    exponent_text.remove_prefix(1);
  int exponent = 0;
  std::from_chars(exponent_text.data(),
                  exponent_text.data() + exponent_text.size(), exponent);
  const auto n = static_cast<int>(digits.size());
  std::string fixed;
  if (exponent >= n - 1) {
    const int zeros = exponent - n + 1;
    fixed = digits + std::string(static_cast<std::size_t>(zeros), '0');
  } else if (exponent >= 0) {
    const auto point = static_cast<std::size_t>(exponent) + 1;
    fixed = digits.substr(0, point) + "." + digits.substr(point);
  } else {
    const int zeros = -exponent - 1;
    fixed = "0." + std::string(static_cast<std::size_t>(zeros), '0') + digits;
  }
  std::string scientific = digits.substr(0, 1);
  if (n > 1)
    scientific += "." + digits.substr(1);
  scientific += "e" + std::to_string(exponent);
  return sign + (scientific.size() < fixed.size() ? scientific : fixed);
}

std::string encode_data(const Data &data) {
  std::string out = R"({"iris":"0.1","entries":[)";
  for (std::size_t i = 0; i < data.size(); ++i) {
    const ValueRef value = data.value(i);
    if (i > 0)
      out += ',';
    out += "{\"name\":" + quote_json(data.name(i)) +
           ",\"type\":" + quote_json(type_name(value.type())) +
           ",\"depth\":" + std::to_string(value.depth()) + ",\"value\":";
    append_value(out, value);
    out += '}';
  }
  return out + "]}\n";
}

ParamPairs decode_params(std::string_view json) {
  return ParamsParser(json).parse();
}

std::string quote_json(std::string_view text) {
  if (!valid_utf8(text))
    throw std::runtime_error("cannot serialize a string that is not UTF-8");
  static constexpr std::string_view hex = "0123456789abcdef";
  std::string out = "\"";
  for (const char c : text) {
    const auto byte = static_cast<unsigned char>(c);
    if ((c == '"') || (c == '\\')) {
      out += '\\';
      out += c;
    } else if (byte < 0x20) {
      out += "\\u00";
      out += hex[byte >> 4U];
      out += hex[byte & 15U];
    } else {
      out += c;
    }
  }
  return out + '"';
}

std::string encode_error(const Error &error) {
  std::string out =
      "{\"code\":" + quote_json(code_name(error.code())) +
      ",\"message\":" + quote_json(repair_utf8(error.message())) +
      ",\"input_location\":" + encode_location(error.input_location()) +
      ",\"spec_location\":" + encode_location(error.spec_location()) +
      ",\"context\":" +
      (error.context() ? quote_json(repair_utf8(*error.context()))
                       : std::string("null")) +
      ",\"record_path\":";
  if (const auto &path = error.record_path()) {
    out += '[';
    for (std::size_t i = 0; i < path->size(); ++i) {
      if (i > 0)
        out += ',';
      out += std::to_string((*path)[i]);
    }
    out += ']';
  } else {
    out += "null";
  }
  return out + "}\n";
}

} // namespace iris::driver
