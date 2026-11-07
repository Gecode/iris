#include <iris/error.hpp>

#include "call_sites.hpp"

#include <algorithm>
#include <cstddef>
#include <optional>
#include <string>
#include <utility>

namespace iris {

namespace {

/// Return `origin:line:column`, without the parts `at` lacks
std::string position(const Location &at) {
  std::string text = at.origin;
  if (at.line) {
    text += ':' + std::to_string(*at.line);
    if (at.column)
      text += ':' + std::to_string(*at.column);
  }
  return text;
}

/// Render Gecode's diagnostic of the error `message` with `details`
/// (Specification §11.1)
std::string diagnostic(const std::string &message,
                       const ErrorDetails &details) {
  const std::optional<Location> &primary =
      details.input_location ? details.input_location : details.spec_location;
  std::string text = primary ? position(*primary) + ": " + message : message;
  if (const std::optional<Location> &at = details.input_location) {
    const std::size_t line = at->line.value_or(1);
    const std::size_t column = at->column.value_or(1);
    const std::string number = std::to_string(line);
    text += '\n' +
            std::string(6 - std::min<std::size_t>(6, number.size()), ' ') +
            number + " | " + details.excerpt.value_or("");
    text += "\n       | " + std::string(column - 1, ' ') + '^';
  }
  if (details.context)
    text += "\n  while reading: " + *details.context;
  if (details.input_location && details.spec_location)
    text += "\n  spec: " + position(*details.spec_location);
  if (details.call_site)
    text += "\ncall site: " + detail::site_text(*details.call_site);
  if (details.earlier_rejection)
    text += "\n  after an earlier error in the instance: " +
            *details.earlier_rejection;
  return text;
}

} // namespace

std::string_view code_name(ErrorCode code) {
  switch (code) {
  case ErrorCode::SpecUtf8:
    return "E-SPEC-UTF8";
  case ErrorCode::SpecString:
    return "E-SPEC-STRING";
  case ErrorCode::SpecNumber:
    return "E-SPEC-NUMBER";
  case ErrorCode::SpecCharacter:
    return "E-SPEC-CHARACTER";
  case ErrorCode::SpecPattern:
    return "E-SPEC-PATTERN";
  case ErrorCode::SpecSyntax:
    return "E-SPEC-SYNTAX";
  case ErrorCode::SpecPlacement:
    return "E-SPEC-PLACEMENT";
  case ErrorCode::SpecName:
    return "E-SPEC-NAME";
  case ErrorCode::SpecNameConflict:
    return "E-SPEC-NAME-CONFLICT";
  case ErrorCode::SpecUnknownType:
    return "E-SPEC-UNKNOWN-TYPE";
  case ErrorCode::SpecType:
    return "E-SPEC-TYPE";
  case ErrorCode::SpecNesting:
    return "E-SPEC-NESTING";
  case ErrorCode::SpecFunction:
    return "E-SPEC-FUNCTION";
  case ErrorCode::SpecConstant:
    return "E-SPEC-CONSTANT";
  case ErrorCode::SpecRecordScope:
    return "E-SPEC-RECORD-SCOPE";
  case ErrorCode::SpecField:
    return "E-SPEC-FIELD";
  case ErrorCode::SpecRecordValue:
    return "E-SPEC-RECORD-VALUE";
  case ErrorCode::SpecCheckScope:
    return "E-SPEC-CHECK-SCOPE";
  case ErrorCode::Budget:
    return "E-BUDGET";
  case ErrorCode::CheckFailed:
    return "E-CHECK-FAILED";
  case ErrorCode::CheckRecords:
    return "E-CHECK-RECORDS";
  case ErrorCode::ComparisonType:
    return "E-COMPARISON-TYPE";
  case ErrorCode::Count:
    return "E-COUNT";
  case ErrorCode::DivisionZero:
    return "E-DIVISION-ZERO";
  case ErrorCode::ExpectedToken:
    return "E-EXPECTED-TOKEN";
  case ErrorCode::ExpectedValue:
    return "E-EXPECTED-VALUE";
  case ErrorCode::Exponent:
    return "E-EXPONENT";
  case ErrorCode::FieldUndefined:
    return "E-FIELD-UNDEFINED";
  case ErrorCode::FindMatch:
    return "E-FIND-MATCH";
  case ErrorCode::FunctionEmpty:
    return "E-FUNCTION-EMPTY";
  case ErrorCode::FunctionType:
    return "E-FUNCTION-TYPE";
  case ErrorCode::IndexContext:
    return "E-INDEX-CONTEXT";
  case ErrorCode::IndexMismatch:
    return "E-INDEX-MISMATCH";
  case ErrorCode::IndexRange:
    return "E-INDEX-RANGE";
  case ErrorCode::IndexScalar:
    return "E-INDEX-SCALAR";
  case ErrorCode::IndexType:
    return "E-INDEX-TYPE";
  case ErrorCode::InstanceEmpty:
    return "E-INSTANCE-EMPTY";
  case ErrorCode::InstanceNotFound:
    return "E-INSTANCE-NOT-FOUND";
  case ErrorCode::InstanceNumber:
    return "E-INSTANCE-NUMBER";
  case ErrorCode::InstanceSelection:
    return "E-INSTANCE-SELECTION";
  case ErrorCode::IntegerOverflow:
    return "E-INTEGER-OVERFLOW";
  case ErrorCode::IntegerType:
    return "E-INTEGER-TYPE";
  case ErrorCode::Layout:
    return "E-LAYOUT";
  case ErrorCode::LineMatch:
    return "E-LINE-MATCH";
  case ErrorCode::ListTerminator:
    return "E-LIST-TERMINATOR";
  case ErrorCode::NameUndefined:
    return "E-NAME-UNDEFINED";
  case ErrorCode::NoProgress:
    return "E-NO-PROGRESS";
  case ErrorCode::OffsetType:
    return "E-OFFSET-TYPE";
  case ErrorCode::ParamMissing:
    return "E-PARAM-MISSING";
  case ErrorCode::ParamType:
    return "E-PARAM-TYPE";
  case ErrorCode::ParamUnknown:
    return "E-PARAM-UNKNOWN";
  case ErrorCode::PatternReference:
    return "E-PATTERN-REFERENCE";
  case ErrorCode::RangeBounds:
    return "E-RANGE-BOUNDS";
  case ErrorCode::RepeatDepth:
    return "E-REPEAT-DEPTH";
  case ErrorCode::RepeatMissing:
    return "E-REPEAT-MISSING";
  case ErrorCode::RepeatTerminator:
    return "E-REPEAT-TERMINATOR";
  case ErrorCode::SectionsRepeated:
    return "E-SECTIONS-REPEATED";
  case ErrorCode::SectionsUnexpected:
    return "E-SECTIONS-UNEXPECTED";
  case ErrorCode::SizeLimit:
    return "E-SIZE-LIMIT";
  case ErrorCode::SkipEof:
    return "E-SKIP-EOF";
  case ErrorCode::Symbol:
    return "E-SYMBOL";
  case ErrorCode::TrailingInput:
    return "E-TRAILING-INPUT";
  case ErrorCode::TypeUndefined:
    return "E-TYPE-UNDEFINED";
  case ErrorCode::ValueTypeMix:
    return "E-VALUE-TYPE-MIX";
  case ErrorCode::Utf8:
    return "E-UTF8";
  }
  std::unreachable();
}

Error::Error(ErrorCode code, std::string message, ErrorDetails details)
    : std::runtime_error(diagnostic(message, details)), code_(code),
      message_(std::move(message)), details_(std::move(details)) {}

} // namespace iris
