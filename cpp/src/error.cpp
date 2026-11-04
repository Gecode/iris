#include <iris/error.hpp>

#include <utility>

namespace iris {

namespace {

/// Render Gecode's diagnostic envelope around `message`, located at the
/// input location if there is one, otherwise at the reader-spec location.
std::string envelope(const std::string &message,
                     const std::optional<Location> &input_location,
                     const std::optional<Location> &spec_location) {
  const std::optional<Location> &primary =
      input_location ? input_location : spec_location;
  if (!primary)
    return message;
  std::string text = primary->origin;
  if (primary->line) {
    text += ':' + std::to_string(*primary->line);
    if (primary->column)
      text += ':' + std::to_string(*primary->column);
  }
  return text + ": " + message;
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

Error::Error(ErrorCode code, std::string message,
             std::optional<Location> input_location,
             std::optional<Location> spec_location,
             std::optional<std::string> context,
             std::optional<std::vector<std::size_t>> record_path)
    : std::runtime_error(envelope(message, input_location, spec_location)),
      code_(code), message_(std::move(message)),
      input_location_(std::move(input_location)),
      spec_location_(std::move(spec_location)), context_(std::move(context)),
      record_path_(std::move(record_path)) {}

} // namespace iris
