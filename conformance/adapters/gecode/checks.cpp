#include "errors.hh"
#include "json.hh"
#include <fstream>
#include <iostream>
#include <limits>
#include <regex>
#include <set>
#include <sstream>
#include <tuple>

namespace {
void require(bool condition, const std::string &message) {
  if (!condition)
    throw std::runtime_error(message);
}

template <typename Exception = std::runtime_error, typename Action>
void rejects(Action action, const std::string &message) {
  try {
    action();
  } catch (const Exception &) {
    return;
  }
  throw std::runtime_error(message);
}

void check_registry_coverage(const std::string &path) {
  std::ifstream file(path);
  require(bool(file), "cannot read registry");
  using Row = std::tuple<iris::Phase, std::string, std::string>;
  std::set<Row> registry;
  std::set<std::string> exclusions;
  int section = 0;
  std::string line;
  const std::regex row(R"(^\| `(E-[A-Z0-9-]+)` \| ([\s\S]*?) \|)");
  while (std::getline(file, line)) {
    if (line.rfind("## 11.", 0) == 0)
      section = std::stoi(line.substr(6));
    std::smatch match;
    if (section < 4 || section > 6 || !std::regex_search(line, match, row))
      continue;
    const std::string code = match[1];
    if (section == 6) {
      require(line.find("None — Iris-only.") != std::string::npos,
              "unaccounted no-pattern code: " + code);
      require(exclusions.insert(code).second, "duplicate Iris-only code");
      continue;
    }
    const std::string cell = match[2];
    auto start = cell.find('`');
    require(start != std::string::npos,
            "Gecode-backed code without pattern: " + code);
    while (true) {
      const auto end = cell.find('`', start + 1);
      require(end != std::string::npos, "unclosed registry template");
      require(
          registry
              .emplace(section == 4 ? iris::Phase::parse : iris::Phase::read,
                       code, cell.substr(start + 1, end - start - 1))
              .second,
          "duplicate registry template");
      // Only the initial run of semicolon-separated backtick alternatives.
      start = end + 1;
      if (cell.substr(start, 3) != "; `")
        break;
      start += 2;
    }
  }
  require(exclusions == std::set<std::string>{"E-SPEC-UTF8", "E-UTF8"},
          "changed Iris-only exclusions");
  std::set<Row> actual;
  std::set<std::string> codes;
  int parse_count = 0;
  for (const auto &pattern : iris::patterns()) {
    require(actual.emplace(pattern.phase, pattern.code, pattern.text).second,
            "duplicate adapter template");
    codes.insert(pattern.code);
    parse_count += pattern.phase == iris::Phase::parse;
    const std::regex compiled(iris::expand_pattern(pattern));
    (void)compiled;
  }
  require(actual == registry,
          "adapter table differs from complete registry alternatives");
  std::cout << "Registry coverage: codes=" << codes.size()
            << " alternatives=" << actual.size() << " parse=" << parse_count
            << " read=" << actual.size() - parse_count
            << " Iris-only exclusions=" << exclusions.size() << '\n';
}

void check_params() {
  const auto pairs = iris::decode_params(
      R"({"z":"0007","a":" a\n\"x\" ","comma":"a,b","zero":"\u0000","unicode":"\u00e9\ud83d\ude00","escapes":"\/\b\f\r\t\\"})");
  require(pairs.size() == 6 && pairs[0].first == "z" &&
              pairs[0].second == "0007" && pairs[1].first == "a" &&
              pairs[1].second == " a\n\"x\" " && pairs[2].second == "a,b" &&
              pairs[3].second == std::string(1, '\0') &&
              pairs[4].second == "é😀" && pairs[5].second == "/\b\f\r\t\\",
          "params lost bytes/order");
  require(iris::decode_params(" \n { } \t").empty(), "empty params");
  for (const std::string text :
       {"", "[]", "null", "{\"a\":1}", "{\"a\":{}}", "{\"a\":[]}",
        "{\"a\":\"1\",\"\\u0061\":\"2\"}", "{\"a\":\"\\ud800\"}",
        "{\"a\":\"\\udc00\"}", "{\"a\":\"\\ud800\\u0041\"}", "{\"a\":\"\\q\"}",
        "{\"a\":\"\\u0x00\"}", "{\"a\":\"x\",}", "{}{}", "\xef\xbb\xbf{}",
        "{\"a\":\"\xff\"}", "{\"a\":\"\n\"}"})
    rejects<iris::Misuse>([&] { iris::decode_params(text); },
                          "invalid params accepted");
}

void check_json() {
  for (const std::string text :
       {"\xc0\x80", "\xed\xa0\x80", "\xf4\x90\x80\x80", "\xe2\x82", "\x80"})
    require(!iris::valid_utf8(text), "invalid UTF-8 accepted");
  require(iris::valid_utf8("café😀") && iris::valid_utf8(std::string(1, '\0')),
          "valid UTF-8 rejected");
  require(iris::quote_json(std::string("\0\n\"\\", 4)) ==
              R"("\u0000\u000a\"\\")",
          "JSON escaping");
  rejects([&] { iris::quote_json("\xff"); }, "invalid JSON string accepted");
  require(
      iris::encode_float(0.0) == "0" && iris::encode_float(-0.0) == "-0.0" &&
          iris::encode_float(3.0) == "3" && iris::encode_float(0.5) == "0.5" &&
          iris::encode_float(100.0) == "100" &&
          iris::encode_float(1000.0) == "1e3" &&
          iris::encode_float(0.001) == "1e-3" &&
          iris::encode_float(1.2345678901234567) == "1.2345678901234567",
      "canonical floats");
  rejects([] { iris::encode_float(std::numeric_limits<double>::infinity()); },
          "infinity accepted");
}

std::string runtime(const std::string &message,
                    const std::string &footer = "\n  spec: spec.iris:2") {
  return "input.bin:1:2: " + message + "\n     1 | 0\n       |  ^" + footer;
}

void check_diagnostics() {
  using iris::Phase;
  require(iris::map_message(Phase::read, "repeat count is negative (-1)") ==
                  "E-COUNT" &&
              iris::map_message(Phase::read, "bogus must be an integer") ==
                  "E-UNMAPPED" &&
              iris::map_message(Phase::read, "'bogus' needs integers") ==
                  "E-UNMAPPED" &&
              iris::map_message(Phase::read, "'sum' needs integers") ==
                  "E-INTEGER-TYPE" &&
              iris::map_message(Phase::read, "'min' of no integers") ==
                  "E-FUNCTION-EMPTY" &&
              iris::map_message(Phase::read, "'sum' of no integers") ==
                  "E-UNMAPPED" &&
              iris::map_message(
                  Phase::read,
                  "there is no instance 'a' (the file has 2 instances)") ==
                  "E-INSTANCE-NOT-FOUND" &&
              iris::map_message(
                  Phase::read,
                  "there is no instance 'a' (the file has 2 instancesx)") ==
                  "E-UNMAPPED" &&
              iris::map_message(Phase::read,
                                "expected 'a' or 'b' before end of input") ==
                  "E-REPEAT-TERMINATOR" &&
              iris::map_message(Phase::read,
                                "expected 'a' xyz before end of input") ==
                  "E-UNMAPPED" &&
              iris::map_message(Phase::read,
                                "expected count (an int in ..3), found 'no'") ==
                  "E-EXPECTED-VALUE" &&
              iris::map_message(Phase::parse, "unknown statement 'a\n\"b'") ==
                  "E-SPEC-SYNTAX",
          "pattern expansion restrictions");
  const auto decode = [](const std::string &text) {
    return iris::decode_error(Phase::read, text, "spec.iris", "input.bin",
                              "0\n");
  };
  for (const auto &witness :
       {"record 3 of record 2 of 'x'",
        "record 3 of 'steps' in record 2 of 'jobs'",
        "element 3 of 'r' in element 2 of 'a'",
        "element 3 of the array in record 2 of 'jobs.x'"}) {
    const auto d = decode(runtime(std::string("check failed for ") + witness +
                                  ": message: record 99 of 'fake'"));
    require(d.code == "E-CHECK-FAILED" &&
                d.record_path == std::vector<std::size_t>{1, 2},
            "witness ordering");
  }
  const std::string dynamic =
      "check failed: first\n  spec: spec.iris:99\n  while reading: fake";
  const auto d = decode(runtime(dynamic));
  require(d.message == dynamic && d.record_path->empty() && !d.context &&
              d.spec_location->line == 2,
          "fake footer inside message");
  const auto context = decode(
      runtime("expected int, found 'oops'",
              "\n  while reading: repeat [2 of 2] > a\n  spec: spec.iris:2"));
  require(!context.record_path && context.context == "repeat [2 of 2] > a",
          "context became witness");
  const auto deferred =
      decode(runtime("division by zero") +
             "\n  after an earlier error in the instance: positive");
  require(deferred.code == "E-DIVISION-ZERO" && !deferred.record_path,
          "deferred note replaced primary");
  const auto unknown_text = runtime("future error");
  const auto unknown = decode(unknown_text);
  require(unknown.code == "E-UNMAPPED" && unknown.message == unknown_text,
          "unknown message sentinel");
  rejects([&] { decode("input.bin:1:2: truncated"); },
          "truncated diagnostic accepted");
  rejects(
      [&] {
        decode(runtime(dynamic +
                       "\n     1 | 0\n       |  ^\n  while reading: fake"));
      },
      "ambiguous envelope accepted");
  const auto parsed = iris::decode_error(
      Phase::parse, "s:with:colon:3: unknown statement 'a\nb'", "s:with:colon",
      "", "");
  require(parsed.spec_location->line == 3 && !parsed.spec_location->column &&
              parsed.message == "unknown statement 'a\nb'",
          "parse envelope");
  const auto param =
      decode("spec.iris: the spec has no parameter 'z_surprise'");
  require(param.code == "E-PARAM-UNKNOWN" && !param.input_location &&
              !param.spec_location->line,
          "unknown params location");
  const auto trailing =
      decode(runtime("unexpected input after the end of the spec: '0'", ""));
  require(trailing.code == "E-TRAILING-INPUT" && !trailing.spec_location,
          "trailing input envelope");
}

void check_nul_diagnostics() {
  const auto decode = [](const std::string &message, const std::string &input,
                         bool trailing = false) {
    const auto text =
        "input.bin:1:1: " + message + "\n     1 | " + input + "\n       | ^" +
        (trailing ? "" : "\n  while reading: x\n  spec: spec.iris:2");
    const Gecode::Instance::Error error(text);
    require(std::string(error.what()) == text.substr(0, text.find('\0')),
            "reference what() did not truncate at NUL");
    return iris::decode_error(iris::Phase::read, error, "spec.iris",
                              "input.bin", input);
  };
  const std::string token("1\0'\\2", 5);
  for (const std::string type :
       {"int", "float", "a number", "n (an int in ..3)"}) {
    const auto message = "expected " + type + ", found '" + token + "'";
    const auto d = decode(message, token);
    require(
        d.code == "E-EXPECTED-VALUE" && d.message == message &&
            d.input_location->line == 1 && d.input_location->column == 1 &&
            d.spec_location->line == 2 && d.context == "x" && !d.record_path &&
            iris::encode_error(d).find("1\\u0000'\\\\2") != std::string::npos,
        "NUL value diagnostic lost token, escaping, or envelope");
  }
  const auto literal = decode("expected 'end', found '" + token + "'", token);
  require(literal.code == "E-EXPECTED-TOKEN", "NUL literal mismatch");
  const std::string nul(1, '\0');
  const auto symbol =
      decode("unexpected symbol '" + nul + "', expected one of 'ab'", nul);
  require(symbol.code == "E-SYMBOL" &&
              symbol.message ==
                  "unexpected symbol '" + nul + "', expected one of 'ab'",
          "NUL symbol lost alphabet suffix");
  const std::string rest("word\t\0tail", 10);
  const auto trailing = decode(
      "unexpected input after the end of the spec: 'word\\t" + nul + "tail'",
      rest, true);
  require(trailing.code == "E-TRAILING-INPUT" && !trailing.spec_location,
          "NUL trailing input with escaped tab");
  rejects([&] { decode("expected int, found '1" + nul + "2'", "1x2"); },
          "truncation without an input NUL accepted");
  rejects([&] { decode("expected int, found '1" + nul + "2'", "2" + nul); },
          "truncation with a different input prefix accepted");
  rejects([&] { decode("future message '" + token + "'", token); },
          "NUL truncation without a known template accepted");
  rejects([&] { decode("expected '" + token + "', found 'word'", token); },
          "NUL in expected literal accepted as input truncation");
  rejects([&] { decode("expected int, found 'bad'", token); },
          "NUL in excerpt accepted as message truncation");
  rejects(
      [&] { decode("expected 'x', found 'y', found '" + token + "'", token); },
      "ambiguous quoted slot accepted");
  rejects(
      [&] {
        iris::decode_error(iris::Phase::read,
                           "input.bin:1:1: expected int, found '1", "spec.iris",
                           "input.bin", token);
      },
      "truncation without retained complete diagnostic accepted");
}
} // namespace

int main(int argc, char **argv) {
  try {
    require(argc == 2, "expected registry path");
    check_registry_coverage(argv[1]);
    check_params();
    check_json();
    check_diagnostics();
    check_nul_diagnostics();
    std::cout << "Adapter checks passed\n";
    return 0;
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
