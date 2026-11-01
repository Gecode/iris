#include "json.hh"
#include <charconv>
#include <cmath>
#include <locale>
#include <set>
#include <sstream>

namespace iris {
namespace {
// Only the flat string map required by the driver protocol is accepted.
class ParamsParser {
public:
  explicit ParamsParser(std::string_view json) : text(json) {}
  ParamPairs parse() {
    if (!valid_utf8(text))
      fail();
    expect('{');
    ParamPairs pairs;
    std::set<std::string> names;
    if (!take('}')) {
      do {
        auto name = string();
        expect(':');
        auto value = string();
        if (!names.insert(name).second)
          fail();
        pairs.emplace_back(std::move(name), std::move(value));
      } while (take(','));
      expect('}');
    }
    whitespace();
    if (pos != text.size())
      fail();
    return pairs;
  }

private:
  std::string_view text;
  std::size_t pos = 0;
  [[noreturn]] static void fail() {
    throw Misuse("params must be one JSON object with unique string keys and "
                 "string values");
  }
  void whitespace() {
    while (pos < text.size() && (text[pos] == ' ' || text[pos] == '\t' ||
                                 text[pos] == '\r' || text[pos] == '\n'))
      ++pos;
  }
  bool take(char c) {
    whitespace();
    if (pos == text.size() || text[pos] != c)
      return false;
    ++pos;
    return true;
  }
  void expect(char c) {
    if (!take(c))
      fail();
  }
  unsigned hex_quad() {
    unsigned value = 0;
    for (int i = 0; i < 4; ++i) {
      if (pos == text.size())
        fail();
      const char c = text[pos++];
      unsigned digit;
      if (c >= '0' && c <= '9')
        digit = c - '0';
      else if (c >= 'a' && c <= 'f')
        digit = c - 'a' + 10;
      else if (c >= 'A' && c <= 'F')
        digit = c - 'A' + 10;
      else
        fail();
      value = value * 16 + digit;
    }
    return value;
  }
  void unicode(std::string &out) {
    unsigned scalar = hex_quad();
    if (scalar >= 0xd800 && scalar <= 0xdbff) {
      if (text.substr(pos, 2) != "\\u")
        fail();
      pos += 2;
      const unsigned low = hex_quad();
      if (low < 0xdc00 || low > 0xdfff)
        fail();
      scalar = 0x10000 + ((scalar - 0xd800) << 10) + low - 0xdc00;
    } else if (scalar >= 0xdc00 && scalar <= 0xdfff) {
      fail();
    }
    if (scalar < 0x80) {
      out += static_cast<char>(scalar);
    } else {
      if (scalar < 0x800) {
        out += static_cast<char>(0xc0 | (scalar >> 6));
      } else {
        if (scalar < 0x10000) {
          out += static_cast<char>(0xe0 | (scalar >> 12));
        } else {
          out += static_cast<char>(0xf0 | (scalar >> 18));
          out += static_cast<char>(0x80 | ((scalar >> 12) & 0x3f));
        }
        out += static_cast<char>(0x80 | ((scalar >> 6) & 0x3f));
      }
      out += static_cast<char>(0x80 | (scalar & 0x3f));
    }
  }
  std::string string() {
    expect('"');
    std::string out;
    while (pos < text.size()) {
      const unsigned char c = text[pos++];
      if (c == '"')
        return out;
      if (c < 0x20)
        fail();
      if (c != '\\') {
        out += static_cast<char>(c);
        continue;
      }
      if (pos == text.size())
        fail();
      switch (text[pos++]) {
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

void require_utf8(std::string_view text) {
  if (!valid_utf8(text))
    throw std::runtime_error("cannot serialize non-UTF-8 reference string");
}

// A stack of array cursors avoids adding a host recursion limit. The path
// contains the current child index at each open array level.
enum class Traversal { write_json, validate_strings };

void traverse_value(std::ostream &out, const Gecode::Instance::Data &data,
                    const std::string &name, Traversal mode) {
  const bool validate_only = mode == Traversal::validate_strings;
  const int depth = data.depth(name);
  const auto type = data.type(name);
  Gecode::IntArgs path(depth);
  std::vector<int> sizes;
  int level = 0;
  while (true) {
    Gecode::IntArgs at(level);
    for (int i = 0; i < level; ++i)
      at[i] = path[i];
    if (level < depth) {
      const int size = data.size(name, at);
      if (!validate_only)
        out << '[';
      if (size > 0) {
        sizes.push_back(size);
        path[level++] = 0;
        continue;
      }
      if (!validate_only)
        out << ']';
    } else {
      switch (type) {
      case Gecode::Instance::ValueType::INT:
        if (!validate_only)
          out << data.integer(name, at);
        break;
      case Gecode::Instance::ValueType::FLOAT:
        if (!validate_only)
          out << encode_float(data.number(name, at));
        break;
      case Gecode::Instance::ValueType::STRING:
        if (validate_only)
          require_utf8(data.text(name, at));
        else
          out << quote_json(data.text(name, at));
        break;
      }
    }
    while (level > 0 && ++path[level - 1] == sizes.back()) {
      if (!validate_only)
        out << ']';
      --level;
      sizes.pop_back();
    }
    if (level == 0)
      return;
    if (!validate_only)
      out << ',';
  }
}
std::string encode_location(const std::optional<Location> &location) {
  if (!location)
    return "null";
  return "{\"origin\":" + quote_json(location->origin) + ",\"line\":" +
         (location->line ? std::to_string(*location->line) : "null") +
         ",\"column\":" +
         (location->column ? std::to_string(*location->column) : "null") + "}";
}
} // namespace

ParamPairs decode_params(std::string_view json) {
  return ParamsParser(json).parse();
}

bool valid_utf8(std::string_view text) {
  for (std::size_t i = 0; i < text.size();) {
    const auto first = static_cast<unsigned char>(text[i++]);
    if (first < 0x80)
      continue;
    unsigned scalar;
    int following;
    unsigned minimum;
    if (first >= 0xc2 && first <= 0xdf) {
      scalar = first & 0x1f;
      following = 1;
      minimum = 0x80;
    } else if (first >= 0xe0 && first <= 0xef) {
      scalar = first & 0x0f;
      following = 2;
      minimum = 0x800;
    } else if (first >= 0xf0 && first <= 0xf4) {
      scalar = first & 0x07;
      following = 3;
      minimum = 0x10000;
    } else {
      return false;
    }
    for (int j = 0; j < following; ++j) {
      if (i == text.size())
        return false;
      const auto c = static_cast<unsigned char>(text[i++]);
      if ((c & 0xc0) != 0x80)
        return false;
      scalar = (scalar << 6) | (c & 0x3f);
    }
    if (scalar < minimum || scalar > 0x10ffff ||
        (scalar >= 0xd800 && scalar <= 0xdfff))
      return false;
  }
  return true;
}

std::string quote_json(std::string_view text) {
  require_utf8(text);
  std::string out = "\"";
  constexpr char hex[] = "0123456789abcdef";
  for (unsigned char c : text) {
    if (c == '"' || c == '\\') {
      out += '\\';
      out += static_cast<char>(c);
    } else if (c < 0x20) {
      out += "\\u00";
      out += hex[c >> 4];
      out += hex[c & 15];
    } else {
      out += static_cast<char>(c);
    }
  }
  return out + '"';
}

std::string encode_float(double value) {
  if (!std::isfinite(value))
    throw std::runtime_error("cannot serialize non-finite reference number");
  if (value == 0)
    return std::signbit(value) ? "-0.0" : "0";
  char buffer[64];
  const auto converted =
      std::to_chars(buffer, buffer + sizeof(buffer), std::fabs(value),
                    std::chars_format::scientific);
  if (converted.ec != std::errc())
    throw std::runtime_error("float conversion failed");
  const std::string scientific(buffer, converted.ptr);
  const auto e = scientific.find('e');
  const int exponent = std::stoi(scientific.substr(e + 1));
  std::string digits = scientific.substr(0, e);
  const auto dot = digits.find('.');
  if (dot != std::string::npos)
    digits.erase(dot, 1);
  while (digits.size() > 1 && digits.back() == '0')
    digits.pop_back();
  const int point = exponent + 1;
  std::string fixed;
  if (point <= 0)
    fixed = "0." + std::string(-point, '0') + digits;
  else if (point >= static_cast<int>(digits.size()))
    fixed = digits + std::string(point - digits.size(), '0');
  else
    fixed = digits.substr(0, point) + "." + digits.substr(point);
  const std::string normalized =
      digits.substr(0, 1) + (digits.size() > 1 ? "." + digits.substr(1) : "") +
      "e" + std::to_string(exponent);
  return (value < 0 ? "-" : "") +
         (fixed.size() <= normalized.size() ? fixed : normalized);
}

std::string encode_data(const Gecode::Instance::Data &data) {
  std::ostringstream out;
  out.imbue(std::locale::classic());
  out << "{\"iris\":\"0.1\",\"entries\":[";
  for (int i = 0; i < data.entries(); ++i) {
    if (i)
      out << ',';
    const auto name = data.name(i);
    const auto type = data.type(name);
    out << "{\"name\":" << quote_json(name) << ",\"type\":"
        << quote_json(type == Gecode::Instance::ValueType::INT     ? "int"
                      : type == Gecode::Instance::ValueType::FLOAT ? "float"
                                                                   : "string")
        << ",\"depth\":" << data.depth(name) << ",\"value\":";
    traverse_value(out, data, name, Traversal::write_json);
    out << '}';
  }
  out << "]}\n";
  if (!out)
    throw std::runtime_error("JSON serialization failed");
  return out.str();
}

std::string render_dzn(const Gecode::Instance::Data &data) {
  std::ostringstream out;
  out.imbue(std::locale::classic());
  for (int i = 0; i < data.entries(); ++i) {
    const auto name = data.name(i);
    require_utf8(name);
    if (data.type(name) == Gecode::Instance::ValueType::STRING)
      traverse_value(out, data, name, Traversal::validate_strings);
  }
  data.dzn(out);
  if (!out)
    throw std::runtime_error("dzn rendering failed");
  return out.str();
}

std::string encode_error(const Diagnostic &diagnostic) {
  std::string out =
      "{\"code\":" + quote_json(diagnostic.code) +
      ",\"message\":" + quote_json(diagnostic.message) +
      ",\"input_location\":" + encode_location(diagnostic.input_location) +
      ",\"spec_location\":" + encode_location(diagnostic.spec_location) +
      ",\"context\":" +
      (diagnostic.context ? quote_json(*diagnostic.context) : "null") +
      ",\"record_path\":";
  if (diagnostic.record_path) {
    out += '[';
    for (auto index : *diagnostic.record_path) {
      if (out.back() != '[')
        out += ',';
      out += std::to_string(index);
    }
    out += ']';
  } else {
    out += "null";
  }
  return out + "}\n";
}
} // namespace iris
