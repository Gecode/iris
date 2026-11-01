#include "errors.hh"
#include <algorithm>
#include <charconv>
#include <gecode/instance.hh>
#include <iomanip>
#include <regex>
#include <sstream>
#include <stdexcept>

namespace iris {
const std::vector<Pattern> &patterns() {
  static const std::vector<Pattern> table = {
      {Phase::parse, "E-SPEC-STRING", R"pattern(unterminated string)pattern"},
      {Phase::parse, "E-SPEC-STRING",
       R"pattern(unknown escape '{A}' in string (use {A}))pattern"},
      {Phase::parse, "E-SPEC-NUMBER", R"pattern(bad number {A})pattern"},
      {Phase::parse, "E-SPEC-CHARACTER",
       R"pattern(unexpected character '{A}')pattern"},
      {Phase::parse, "E-SPEC-PATTERN",
       R"pattern(unterminated capture in pattern {P})pattern"},
      {Phase::parse, "E-SPEC-PATTERN",
       R"pattern(missing or bad name in pattern {P})pattern"},
      {Phase::parse, "E-SPEC-PATTERN",
       R"pattern(capture name {Q} in pattern {P} is not a name)pattern"},
      {Phase::parse, "E-SPEC-PATTERN",
       R"pattern('...' must end the pattern {P})pattern"},
      {Phase::parse, "E-SPEC-PATTERN",
       R"pattern(a capture cannot be of an alphabet type)pattern"},
      {Phase::parse, "E-SPEC-PATTERN",
       R"pattern(a capture needs a type after ':')pattern"},
      {Phase::parse, "E-SPEC-PATTERN",
       R"pattern(unexpected {T} in the capture type {Q})pattern"},
      {Phase::parse, "E-SPEC-SYNTAX",
       R"pattern(expected {A}, found {T})pattern"},
      {Phase::parse, "E-SPEC-SYNTAX", R"pattern(missing '}')pattern"},
      {Phase::parse, "E-SPEC-SYNTAX",
       R"pattern(unknown statement '{A}')pattern"},
      {Phase::parse, "E-SPEC-SYNTAX",
       R"pattern('..<' needs an upper bound)pattern"},
      {Phase::parse, "E-SPEC-SYNTAX",
       R"pattern(a range needs a lower or an upper bound)pattern"},
      {Phase::parse, "E-SPEC-PLACEMENT",
       R"pattern('instances' must be the last statement of the spec)pattern"},
      {Phase::parse, "E-SPEC-PLACEMENT",
       R"pattern(the setting '{N}' must be at the top level of the spec)pattern"},
      {Phase::parse, "E-SPEC-PLACEMENT",
       R"pattern('each' must be the first statement of a repeat, and comes at most once)pattern"},
      {Phase::parse, "E-SPEC-PLACEMENT",
       R"pattern('instances' must be at the top level of the spec)pattern"},
      {Phase::parse, "E-SPEC-NAME",
       R"pattern('{A}' cannot be defined: names starting with 'gecode.' are Gecode constants)pattern"},
      {Phase::parse, "E-SPEC-NAME",
       R"pattern('_' cannot be defined here)pattern"},
      {Phase::parse, "E-SPEC-NAME", R"pattern('_' cannot be bound)pattern"},
      {Phase::parse, "E-SPEC-NAME-CONFLICT",
       R"pattern('{N}' is bound by a comprehension but is also a name in the spec)pattern"},
      {Phase::parse, "E-SPEC-NAME-CONFLICT",
       R"pattern('{N}' names a type, so it cannot also name a value)pattern"},
      {Phase::parse, "E-SPEC-NAME-CONFLICT",
       R"pattern('{N}' is a base type)pattern"},
      {Phase::parse, "E-SPEC-NAME-CONFLICT",
       R"pattern(the type '{N}' is already defined)pattern"},
      {Phase::parse, "E-SPEC-NAME-CONFLICT",
       R"pattern('{N}' is already bound by an enclosing comprehension)pattern"},
      {Phase::parse, "E-SPEC-NAME-CONFLICT",
       R"pattern('{N}' already names records)pattern"},
      {Phase::parse, "E-SPEC-NAME-CONFLICT",
       R"pattern('{N}' names records, so it cannot also be defined)pattern"},
      {Phase::parse, "E-SPEC-NAME-CONFLICT",
       R"pattern('{N}' cannot name both the list and each record)pattern"},
      {Phase::parse, "E-SPEC-NAME-CONFLICT",
       R"pattern('{N}' is bound by a comprehension but names records)pattern"},
      {Phase::parse, "E-SPEC-UNKNOWN-TYPE",
       R"pattern(unknown type '{N}')pattern"},
      {Phase::parse, "E-SPEC-TYPE", R"pattern('offset' needs numbers)pattern"},
      {Phase::parse, "E-SPEC-TYPE",
       R"pattern('rest' values cannot be arrays)pattern"},
      {Phase::parse, "E-SPEC-TYPE",
       R"pattern(an alphabet type needs a two-dimensional array)pattern"},
      {Phase::parse, "E-SPEC-TYPE",
       R"pattern('layout' needs a two-dimensional array of numbers)pattern"},
      {Phase::parse, "E-SPEC-TYPE",
       R"pattern(a list cannot be of 'rest' or of an alphabet type)pattern"},
      {Phase::parse, "E-SPEC-NESTING",
       R"pattern(blocks are nested too deeply)pattern"},
      {Phase::parse, "E-SPEC-NESTING",
       R"pattern(expressions are nested too deeply)pattern"},
      {Phase::parse, "E-SPEC-FUNCTION",
       R"pattern('{N}' needs the form {N}(x in a: e))pattern"},
      {Phase::parse, "E-SPEC-FUNCTION",
       R"pattern(unknown function '{N}')pattern"},
      {Phase::parse, "E-SPEC-FUNCTION",
       R"pattern(wrong arguments for function '{N}')pattern"},
      {Phase::parse, "E-SPEC-CONSTANT",
       R"pattern(unknown Gecode constant '{A}' (known: gecode.int_max, gecode.int_min))pattern"},
      {Phase::parse, "E-SPEC-RECORD-SCOPE",
       R"pattern('{N}' is inside a repeat without a list name in the records of '{N}'; name that list with 'as' to reach it)pattern"},
      {Phase::parse, "E-SPEC-RECORD-SCOPE",
       R"pattern('{N}' cannot be used inside its own repeat)pattern"},
      {Phase::parse, "E-SPEC-RECORD-SCOPE",
       R"pattern(the field '{N}' cannot be used inside the repeat that reads it)pattern"},
      {Phase::parse, "E-SPEC-RECORD-SCOPE",
       R"pattern('{N}' is a list in each record of the repeat around it; {A})pattern"},
      {Phase::parse, "E-SPEC-RECORD-SCOPE",
       R"pattern('{N}' is each record of a repeat, so only checks can use it outside that repeat)pattern"},
      {Phase::parse, "E-SPEC-FIELD",
       R"pattern(the records of '{N}' have no field '{A}' ({A}))pattern"},
      {Phase::parse, "E-SPEC-FIELD",
       R"pattern('{N}' is defined inside and outside the repeat of '{N}', so it cannot be a field of its records)pattern"},
      {Phase::parse, "E-SPEC-FIELD",
       R"pattern(only records have fields such as '.{A}'; {A})pattern"},
      {Phase::parse, "E-SPEC-FIELD",
       R"pattern('{N}' is a list of records, so '.{A}' needs one of them: {A})pattern"},
      {Phase::parse, "E-SPEC-RECORD-VALUE",
       R"pattern(a record of '{N}' is not a value; {A})pattern"},
      {Phase::parse, "E-SPEC-RECORD-VALUE",
       R"pattern('{N}' is a list of records, not a value; {A})pattern"},
      {Phase::parse, "E-SPEC-RECORD-VALUE",
       R"pattern(a record of '{N}' cannot be indexed; {A})pattern"},
      {Phase::parse, "E-SPEC-RECORD-VALUE",
       R"pattern(a record of '{N}' is not a list; use one of its fields)pattern"},
      {Phase::parse, "E-SPEC-CHECK-SCOPE",
       R"pattern(a check can only range over the records of one repeat, but '{N}' and '{N}' are defined in different repeats)pattern"},
      {Phase::parse, "E-SPEC-CHECK-SCOPE",
       R"pattern('{N}' is defined in several repeats, or inside and outside a repeat, so a check here cannot range over its records)pattern"},
      {Phase::read, "E-BUDGET",
       R"pattern(too many repetitions or empty rows without reading input)pattern"},
      {Phase::read, "E-BUDGET",
       R"pattern(the expressions do too much work for the size of the input)pattern"},
      {Phase::read, "E-CHECK-FAILED", R"pattern(check failed)pattern"},
      {Phase::read, "E-CHECK-FAILED", R"pattern(check failed: {A})pattern"},
      {Phase::read, "E-CHECK-FAILED", R"pattern(check failed for {A})pattern"},
      {Phase::read, "E-CHECK-RECORDS", R"pattern('{N}' has no records)pattern"},
      {Phase::read, "E-CHECK-RECORDS",
       R"pattern('{N}' and '{N}' have different numbers of records)pattern"},
      {Phase::read, "E-COMPARISON-TYPE",
       R"pattern('{A}' compares single values, not arrays)pattern"},
      {Phase::read, "E-COMPARISON-TYPE",
       R"pattern('{A}' compares numbers)pattern"},
      {Phase::read, "E-COUNT", R"pattern({A} must be an integer)pattern"},
      {Phase::read, "E-COUNT", R"pattern({A} is negative ({I}))pattern"},
      {Phase::read, "E-COUNT", R"pattern({A} is too large ({I}))pattern"},
      {Phase::read, "E-DIVISION-ZERO", R"pattern(division by zero)pattern"},
      {Phase::read, "E-EXPECTED-TOKEN",
       R"pattern(expected {Q}, found {F})pattern", "{F}"},
      {Phase::read, "E-EXPECTED-VALUE",
       R"pattern(expected {D}, found {F})pattern", "{F}"},
      {Phase::read, "E-EXPECTED-VALUE",
       R"pattern(expected a line of values, found end of input)pattern"},
      {Phase::read, "E-EXPONENT", R"pattern(negative exponent in '^')pattern"},
      {Phase::read, "E-FIELD-UNDEFINED",
       R"pattern('{N}' is not defined in this record of '{N}')pattern"},
      {Phase::read, "E-FIND-MATCH",
       R"pattern(no line matching {P} found)pattern"},
      {Phase::read, "E-FUNCTION-EMPTY",
       R"pattern('{N}' of no integers)pattern"},
      {Phase::read, "E-FUNCTION-TYPE", R"pattern('{N}' takes an array)pattern"},
      {Phase::read, "E-FUNCTION-TYPE", R"pattern('{N}' takes integers)pattern"},
      {Phase::read, "E-FUNCTION-TYPE",
       R"pattern('{N}({N} in ...)' needs an array)pattern"},
      {Phase::read, "E-INDEX-CONTEXT",
       R"pattern('index' must be inside a repeat)pattern"},
      {Phase::read, "E-INDEX-MISMATCH",
       R"pattern(expected index {I}, found {I})pattern"},
      {Phase::read, "E-INDEX-RANGE",
       R"pattern(index {I} is out of range for {A} (size {U}, indices start at 0))pattern"},
      {Phase::read, "E-INDEX-SCALAR",
       R"pattern(only arrays can be indexed, not {A})pattern"},
      {Phase::read, "E-INDEX-TYPE",
       R"pattern(an index must be an integer)pattern"},
      {Phase::read, "E-INSTANCE-EMPTY",
       R"pattern(the file contains no instance)pattern"},
      {Phase::read, "E-INSTANCE-NOT-FOUND",
       R"pattern(there is no instance {Q} (the file has {U} instance{A}))pattern"},
      {Phase::read, "E-INSTANCE-NUMBER",
       R"pattern(instance numbers start at 1)pattern"},
      {Phase::read, "E-INSTANCE-SELECTION",
       R"pattern(the file has {U} instances; select one with the parameter instance (a number from 1, or a name))pattern"},
      {Phase::read, "E-INTEGER-OVERFLOW",
       R"pattern(integer overflow in '{A}')pattern"},
      {Phase::read, "E-INTEGER-OVERFLOW",
       R"pattern(integer overflow in the upper bound of a range)pattern"},
      {Phase::read, "E-INTEGER-OVERFLOW",
       R"pattern(integer overflow when applying offset)pattern"},
      {Phase::read, "E-INTEGER-OVERFLOW",
       R"pattern(integer overflow in the value of a symbol)pattern"},
      {Phase::read, "E-INTEGER-TYPE", R"pattern('{A}' needs integers)pattern"},
      {Phase::read, "E-LAYOUT",
       R"pattern(matrix layout must be a string)pattern"},
      {Phase::read, "E-LAYOUT",
       R"pattern(layout {A} needs a square matrix)pattern"},
      {Phase::read, "E-LAYOUT", R"pattern(unknown matrix layout {Q})pattern"},
      {Phase::read, "E-LINE-MATCH",
       R"pattern(expected a line matching {P})pattern"},
      {Phase::read, "E-LINE-MATCH",
       R"pattern(expected a line matching {P}, found end of input)pattern"},
      {Phase::read, "E-LINE-MATCH",
       R"pattern(expected a line matching {P} before {A})pattern"},
      {Phase::read, "E-LIST-TERMINATOR",
       R"pattern(list terminator must be a single value)pattern"},
      {Phase::read, "E-LIST-TERMINATOR",
       R"pattern(a list of words needs a string or integer terminator)pattern"},
      {Phase::read, "E-NAME-UNDEFINED",
       R"pattern('{N}' is not defined yet)pattern"},
      {Phase::read, "E-NO-PROGRESS",
       R"pattern(an instance must read some input)pattern"},
      {Phase::read, "E-NO-PROGRESS",
       R"pattern(the body of a repeat until read no input)pattern"},
      {Phase::read, "E-OFFSET-TYPE",
       R"pattern(offset must be an integer)pattern"},
      {Phase::read, "E-PARAM-MISSING",
       R"pattern(parameter '{N}' must be given)pattern"},
      {Phase::read, "E-PARAM-TYPE",
       R"pattern(parameter '{N}' must be an integer, not {Q})pattern"},
      {Phase::read, "E-PARAM-TYPE",
       R"pattern(parameter '{N}' must be a number, not {Q})pattern"},
      {Phase::read, "E-PARAM-UNKNOWN",
       R"pattern(the spec has no parameter '{A}')pattern"},
      {Phase::read, "E-PATTERN-REFERENCE",
       R"pattern('{N}' must be a string or an integer)pattern"},
      {Phase::read, "E-RANGE-BOUNDS",
       R"pattern(the bounds of a range must be integers)pattern"},
      {Phase::read, "E-REPEAT-DEPTH",
       R"pattern('{N}' has rows of different depths)pattern"},
      {Phase::read, "E-REPEAT-MISSING",
       R"pattern('{N}' is defined in only some repetitions)pattern"},
      {Phase::read, "E-REPEAT-MISSING",
       R"pattern('{N}' is defined in no repetition)pattern"},
      {Phase::read, "E-REPEAT-TERMINATOR",
       R"pattern(expected {Q}{A} before end of input)pattern"},
      {Phase::read, "E-SECTIONS-REPEATED",
       R"pattern(a second line matching {P} (the first is on line {U}))pattern"},
      {Phase::read, "E-SECTIONS-UNEXPECTED",
       R"pattern(unexpected line; expected one of {A})pattern"},
      {Phase::read, "E-SIZE-LIMIT", R"pattern(array is too large)pattern"},
      {Phase::read, "E-SIZE-LIMIT", R"pattern(matrix is too large)pattern"},
      {Phase::read, "E-SKIP-EOF",
       R"pattern(cannot skip a line at end of input)pattern"},
      {Phase::read, "E-SYMBOL",
       R"pattern(expected a symbol for row {U}, column {U}, found end of input)pattern"},
      {Phase::read, "E-SYMBOL",
       R"pattern(unexpected symbol {Q}, expected one of {Q})pattern", "{Q}"},
      {Phase::read, "E-TRAILING-INPUT",
       R"pattern(unexpected input after the end of the spec: {Q})pattern",
       "{Q}"},
      {Phase::read, "E-TYPE-UNDEFINED",
       R"pattern(the type '{N}' is not defined yet)pattern"},
      {Phase::read, "E-VALUE-TYPE-MIX",
       R"pattern('{N}' mixes strings and numbers)pattern"},
      {Phase::read, "E-VALUE-TYPE-MIX",
       R"pattern('{N}': strings and numbers are mixed)pattern"},
  };
  return table;
}
namespace {
std::string literal(char c) {
  const std::string_view special = R"(\.^$|()[]{}*+?)";
  return (special.find(c) == std::string_view::npos ? "" : "\\") +
         std::string(1, c);
}

std::string placeholder(std::string_view name, const std::string &code) {
  const std::string any = R"([\s\S]*)";
  const std::string identifier = "[A-Za-z_][A-Za-z0-9_]*";
  const std::string quoted = "'" + any + "'";
  const std::string pattern = "\"" + any + "\"";
  const std::string base =
      R"((?:int|float|a number|a word|the rest of the line|a symbol|an int in (?:-?[0-9]+)?\.\.(?:-?[0-9]+)?))";
  if (name == "N")
    return code == "E-FUNCTION-EMPTY" ? "(?:min|max)" : identifier;
  if (name == "I")
    return "-?[0-9]+";
  if (name == "U")
    return "[0-9]+";
  if (name == "Q")
    return quoted;
  if (name == "P")
    return pattern;
  if (name == "T")
    return "(?:end of spec|" + quoted + "|" + pattern + ")";
  if (name == "F")
    return "(?:end of input|" + quoted + ")";
  if (name == "B")
    return base;
  if (name == "D")
    return "(?:" + base + "|" + identifier + " \\(" + base + "\\))";
  if (name == "A") {
    if (code == "E-COUNT")
      return "(?:array size|number of lines|index base|repeat count)";
    if (code == "E-INTEGER-TYPE")
      return R"((?:\+|-|\*|/|%|\^|sum))";
    if (code == "E-INSTANCE-NOT-FOUND")
      return "s?";
    if (code == "E-REPEAT-TERMINATOR")
      return "(?: or " + quoted + ")*";
    return any;
  }
  throw std::runtime_error("unknown diagnostic template placeholder: " +
                           std::string(name));
}

std::size_t positive(std::string_view text) {
  std::size_t value = 0;
  const auto result =
      std::from_chars(text.data(), text.data() + text.size(), value);
  if (result.ec != std::errc() || result.ptr != text.data() + text.size() ||
      value == 0)
    throw std::runtime_error("invalid diagnostic coordinate or witness index");
  return value;
}

std::size_t coordinate(std::string_view text, std::size_t &pos,
                       char separator) {
  const auto end = text.find(separator, pos);
  if (end == std::string_view::npos)
    throw std::runtime_error("incomplete reference diagnostic");
  const auto value = positive(text.substr(pos, end - pos));
  pos = end + 1;
  return value;
}

bool consume(std::string_view text, std::size_t &pos, std::string_view prefix) {
  if (text.substr(pos, prefix.size()) != prefix)
    return false;
  pos += prefix.size();
  return true;
}

std::vector<std::size_t> check_path(const std::string &message) {
  std::size_t pos = 0;
  if (!consume(message, pos, "check failed"))
    throw std::runtime_error("invalid check diagnostic");
  std::vector<std::size_t> indices;
  if (pos == message.size() || message.compare(pos, 2, ": ") == 0)
    return indices;
  if (!consume(message, pos, " for "))
    throw std::runtime_error("invalid check witness");
  while (true) {
    if (!consume(message, pos, "record ") && !consume(message, pos, "element "))
      throw std::runtime_error("invalid check witness kind");
    indices.push_back(coordinate(message, pos, ' ') - 1);
    if (!consume(message, pos, "of "))
      throw std::runtime_error("invalid check witness target");
    // Anonymous record chains omit intermediate labels and 'in'.
    if (message.compare(pos, 7, "record ") == 0)
      continue;
    if (!consume(message, pos, "the array")) {
      if (!consume(message, pos, "'"))
        throw std::runtime_error("invalid check witness label");
      const auto end = message.find('\'', pos);
      if (end == std::string::npos)
        throw std::runtime_error("incomplete check witness label");
      const auto label = message.substr(pos, end - pos);
      if (!std::regex_match(
              label,
              std::regex(
                  R"([A-Za-z_][A-Za-z0-9_]*(\.[A-Za-z_][A-Za-z0-9_]*)*)")))
        throw std::runtime_error("invalid check witness label");
      pos = end + 1;
    }
    if (pos == message.size() || message.compare(pos, 2, ": ") == 0)
      break;
    if (!consume(message, pos, " in "))
      throw std::runtime_error("invalid check witness chain");
  }
  std::reverse(indices.begin(), indices.end());
  return indices;
}

std::size_t input_offset(const std::string &input, std::size_t line,
                         std::size_t column) {
  std::size_t start = 0;
  for (std::size_t n = 1; n < line; ++n) {
    const auto lf = input.find('\n', start);
    if (lf == std::string::npos)
      throw std::runtime_error("diagnostic line outside input");
    start = lf + 1;
  }
  auto end = input.find('\n', start);
  if (end == std::string::npos)
    end = input.size();
  if (column - 1 > end - start)
    throw std::runtime_error("diagnostic column outside input line");
  return start + column - 1;
}

std::string excerpt(const std::string &input, std::size_t line,
                    std::size_t column) {
  const auto start = input_offset(input, line, column) - (column - 1);
  auto end = input.find('\n', start);
  if (end == std::string::npos)
    end = input.size();
  while (end > start && (input[end - 1] == ' ' || input[end - 1] == '\t' ||
                         input[end - 1] == '\r'))
    --end;
  std::string text = input.substr(start, end - start);
  if (text.size() > 100)
    text = text.substr(0, 100) + "...";
  std::ostringstream out;
  out << '\n'
      << std::setw(6) << line << " | " << text << "\n       | "
      << std::string(column - 1, ' ') << '^';
  return out.str();
}

// Only an input-valued quoted slot can explain a C-string truncation. The
// retained Error string supplies the complete token and envelope; no token
// boundaries, settings, context, or missing template suffixes are guessed.
void validate_nul_quote(Phase phase, const Diagnostic &diagnostic,
                        const std::string &input) {
  const auto nul = diagnostic.message.find('\0');
  if (phase != Phase::read || !diagnostic.input_location ||
      nul == std::string::npos)
    throw std::runtime_error("unsupported NUL-truncated reference diagnostic");
  const auto &location = *diagnostic.input_location;
  const auto start = input_offset(input, *location.line, *location.column);
  const auto input_nul = input.find('\0', start);
  if (input_nul == std::string::npos)
    throw std::runtime_error("reference truncation has no matching input NUL");
  std::string partial;
  for (auto i = start; i < input_nul; ++i) {
    if (input[i] == '\n')
      throw std::runtime_error("reference truncation crosses an input line");
    partial += input[i] == '\t' ? "\\t" : std::string(1, input[i]);
  }

  const auto prefix = diagnostic.message.substr(0, nul);
  std::size_t matches = 0;
  bool matches_input = false;
  for (const auto &pattern : patterns()) {
    if (pattern.phase != phase || pattern.input_quote.empty())
      continue;
    const auto slot = pattern.text.find(pattern.input_quote);
    const std::regex before_quote(
        expand_pattern({phase, pattern.code, pattern.text.substr(0, slot)}));
    for (auto at = prefix.find('\''); at != std::string::npos;
         at = prefix.find('\'', at + 1)) {
      if (!std::regex_match(prefix.substr(0, at), before_quote))
        continue;
      ++matches;
      matches_input =
          pattern.code == diagnostic.code && prefix.substr(at + 1) == partial;
    }
  }
  if (matches != 1 || !matches_input)
    throw std::runtime_error(
        "ambiguous or unverified NUL-truncated diagnostic");
}

// A footer is accepted only as a complete suffix, with the supplied origin.
// Keep all plausible decodings: unescaped user text can make one ambiguous.
std::vector<Diagnostic> decode_footer(const Diagnostic &base,
                                      std::string_view tail,
                                      const std::string &spec_origin) {
  std::vector<Diagnostic> candidates;
  if (tail.empty()) {
    candidates.push_back(base);
    return candidates;
  }
  constexpr std::string_view context_prefix = "\n  while reading: ";
  const std::string spec_prefix = "\n  spec: " + spec_origin + ":";
  for (std::size_t at = tail.find(spec_prefix); at != std::string_view::npos;
       at = tail.find(spec_prefix, at + 1)) {
    try {
      const auto line = positive(tail.substr(at + spec_prefix.size()));
      Diagnostic result = base;
      result.spec_location = Location{spec_origin, line, std::nullopt};
      if (at != 0) {
        if (tail.substr(0, context_prefix.size()) != context_prefix ||
            at == context_prefix.size())
          continue;
        result.context = std::string(
            tail.substr(context_prefix.size(), at - context_prefix.size()));
      }
      candidates.push_back(std::move(result));
    } catch (const std::runtime_error &) {
      // Not a complete generated spec suffix.
    }
  }
  // The only input-only reference profile is trailing input, without context.
  return candidates;
}
Diagnostic decode_runtime_error(const std::string &diagnostic,
                                const std::string &spec_origin,
                                const std::string &input_origin,
                                const std::string &input) {
  Diagnostic base;
  std::size_t pos = 0;
  if (!consume(diagnostic, pos, input_origin + ":"))
    throw std::runtime_error("invalid runtime diagnostic origin");
  const auto line = coordinate(diagnostic, pos, ':');
  const auto column = coordinate(diagnostic, pos, ':');
  if (!consume(diagnostic, pos, " "))
    throw std::runtime_error("invalid runtime diagnostic prefix");
  base.input_location = Location{input_origin, line, column};
  const auto block = excerpt(input, line, column);
  std::vector<Diagnostic> candidates;
  constexpr std::string_view note =
      "\n  after an earlier error in the instance: ";
  // Consider the full envelope and each possible generated secondary note.
  // Validate every excerpt/footer candidate instead of splitting on markers.
  std::vector<std::size_t> ends{diagnostic.size()};
  for (auto at = diagnostic.find(note, pos); at != std::string::npos;
       at = diagnostic.find(note, at + 1))
    ends.push_back(at);
  for (auto at = diagnostic.find(block, pos); at != std::string::npos;
       at = diagnostic.find(block, at + 1)) {
    Diagnostic candidate = base;
    candidate.message = diagnostic.substr(pos, at - pos);
    for (auto end : ends) {
      if (end < at + block.size())
        continue;
      auto decoded =
          decode_footer(candidate,
                        std::string_view(diagnostic)
                            .substr(at + block.size(), end - at - block.size()),
                        spec_origin);
      for (auto &result : decoded) {
        const auto code = map_message(Phase::read, result.message);
        if ((!result.spec_location && code != "E-TRAILING-INPUT" &&
             code != "E-UNMAPPED") ||
            (result.spec_location && code == "E-TRAILING-INPUT"))
          continue;
        candidates.push_back(std::move(result));
      }
    }
  }
  if (candidates.size() != 1)
    throw std::runtime_error("ambiguous or incomplete reference diagnostic");
  base = std::move(candidates.front());
  return base;
}
} // namespace

std::string expand_pattern(const Pattern &pattern) {
  std::string out;
  for (std::size_t i = 0; i < pattern.text.size();) {
    if (pattern.text[i] != '{') {
      out += literal(pattern.text[i++]);
      continue;
    }
    const auto end = pattern.text.find('}', i);
    if (end == std::string::npos)
      throw std::runtime_error("incomplete diagnostic template placeholder");
    out +=
        placeholder(std::string_view(pattern.text).substr(i + 1, end - i - 1),
                    pattern.code);
    i = end + 1;
  }
  return out;
}

std::string map_message(Phase phase, const std::string &message) {
  struct Compiled {
    Phase phase;
    std::string code;
    std::regex expression;
  };
  static const auto compiled = [] {
    std::vector<Compiled> result;
    for (const auto &pattern : patterns())
      result.push_back(
          {pattern.phase, pattern.code, std::regex(expand_pattern(pattern))});
    return result;
  }();
  std::string code = "E-UNMAPPED";
  for (const auto &pattern : compiled) {
    if (pattern.phase != phase ||
        !std::regex_match(message, pattern.expression))
      continue;
    if (code != "E-UNMAPPED" && code != pattern.code)
      throw std::runtime_error(
          "reference message matches different error codes");
    code = pattern.code;
  }
  return code;
}

Diagnostic decode_error(Phase phase, const std::string &diagnostic,
                        const std::string &spec_origin,
                        const std::string &input_origin,
                        const std::string &input) {
  Diagnostic base;
  std::size_t pos = 0;
  if (phase == Phase::parse) {
    if (!consume(diagnostic, pos, spec_origin + ":"))
      throw std::runtime_error("invalid parse diagnostic origin");
    const auto line = coordinate(diagnostic, pos, ':');
    if (!consume(diagnostic, pos, " "))
      throw std::runtime_error("invalid parse diagnostic prefix");
    base.spec_location = Location{spec_origin, line, std::nullopt};
    base.message = diagnostic.substr(pos);
  } else if (consume(diagnostic, pos, spec_origin + ": ")) {
    base.spec_location = Location{spec_origin, std::nullopt, std::nullopt};
    base.message = diagnostic.substr(pos);
    if (const auto code = map_message(phase, base.message);
        code != "E-PARAM-UNKNOWN" && code != "E-UNMAPPED")
      throw std::runtime_error("invalid locationless reference diagnostic");
  } else {
    base = decode_runtime_error(diagnostic, spec_origin, input_origin, input);
  }
  base.code = map_message(phase, base.message);
  if (diagnostic.find('\0') != std::string::npos)
    validate_nul_quote(phase, base, input);
  if (base.code == "E-UNMAPPED")
    base.message = diagnostic;
  if (base.code == "E-CHECK-FAILED")
    base.record_path = check_path(base.message);
  return base;
}

Diagnostic decode_error(Phase phase, const Gecode::Instance::Error &error,
                        const std::string &spec_origin,
                        const std::string &input_origin,
                        const std::string &input) {
  // Copy the public exception into a derived object to read its protected
  // std::string legally. what() alone loses everything from the first NUL.
  struct StoredError : Gecode::Instance::Error {
    explicit StoredError(const Gecode::Instance::Error &source)
        : Gecode::Instance::Error(source) {}
    const std::string &text() const { return msg; }
  };
  return decode_error(phase, StoredError(error).text(), spec_origin,
                      input_origin, input);
}
} // namespace iris
