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
