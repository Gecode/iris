#include "json.hpp"

#include <cstddef>
#include <cstdint>
#include <optional>
#include <set>

namespace iris::driver {

namespace {

/// Test whether `text` is valid UTF-8
bool valid_utf8(std::string_view text) {
  for (std::size_t i = 0; i < text.size();) {
    const auto first = static_cast<unsigned char>(text[i++]);
    if (first < 0x80)
      continue;
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
      return false;
    }
    for (std::size_t j = 0; j < following; ++j) {
      if (i == text.size())
        return false;
      const auto c = static_cast<unsigned char>(text[i++]);
      if ((c & 0xc0U) != 0x80U)
        return false;
      scalar = (scalar << 6U) | (c & 0x3fU);
    }
    if ((scalar < minimum) || (scalar > 0x10ffff) ||
        ((scalar >= 0xd800) && (scalar <= 0xdfff)))
      return false;
  }
  return true;
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
  return "{\"origin\":" + quote_json(location->origin) +
         ",\"line\":" + number(location->line) +
         ",\"column\":" + number(location->column) + "}";
}

} // namespace

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
      ",\"message\":" + quote_json(error.message()) +
      ",\"input_location\":" + encode_location(error.input_location()) +
      ",\"spec_location\":" + encode_location(error.spec_location()) +
      ",\"context\":" +
      (error.context() ? quote_json(*error.context()) : std::string("null")) +
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
