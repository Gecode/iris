#pragma once

#include <cstddef>
#include <optional>
#include <stdexcept>
#include <string>
#include <string_view>
#include <vector>

namespace iris {

/// A registered error code of the Iris Language Specification (chapter 11).
enum class ErrorCode {
  SpecUtf8,
  SpecString,
  SpecNumber,
  SpecCharacter,
  SpecPattern,
  SpecSyntax,
  SpecPlacement,
  SpecName,
  SpecNameConflict,
  SpecUnknownType,
  SpecType,
  SpecNesting,
  SpecFunction,
  SpecConstant,
  SpecRecordScope,
  SpecField,
  SpecRecordValue,
  SpecCheckScope,
  // Read-time codes (§11.5), then E-UTF8 (§11.6)
  Budget,
  CheckFailed,
  CheckRecords,
  ComparisonType,
  Count,
  DivisionZero,
  ExpectedToken,
  ExpectedValue,
  Exponent,
  FieldUndefined,
  FindMatch,
  FunctionEmpty,
  FunctionType,
  IndexContext,
  IndexMismatch,
  IndexRange,
  IndexScalar,
  IndexType,
  InstanceEmpty,
  InstanceNotFound,
  InstanceNumber,
  InstanceSelection,
  IntegerOverflow,
  IntegerType,
  Layout,
  LineMatch,
  ListTerminator,
  NameUndefined,
  NoProgress,
  OffsetType,
  ParamMissing,
  ParamType,
  ParamUnknown,
  PatternReference,
  RangeBounds,
  RepeatDepth,
  RepeatMissing,
  RepeatTerminator,
  SectionsRepeated,
  SectionsUnexpected,
  SizeLimit,
  SkipEof,
  Symbol,
  TrailingInput,
  TypeUndefined,
  ValueTypeMix,
  Utf8,
};

/// Return the registry spelling of `code`, such as `E-SPEC-SYNTAX`.
std::string_view code_name(ErrorCode code);

/// A position in a reader spec or an input. Lines and columns count from 1.
struct Location {
  std::string origin;
  std::optional<std::size_t> line;
  std::optional<std::size_t> column;
};

/// A language error with the members of the canonical error document
/// (Specification §3.7).
///
/// `what()` renders the diagnostic envelope `origin:line: message`, or
/// `origin:line:column: message` when the primary location has a column.
class Error : public std::runtime_error {
public:
  Error(ErrorCode code, std::string message,
        std::optional<Location> input_location = std::nullopt,
        std::optional<Location> spec_location = std::nullopt,
        std::optional<std::string> context = std::nullopt,
        std::optional<std::vector<std::size_t>> record_path = std::nullopt);

  [[nodiscard]] ErrorCode code() const { return code_; }
  [[nodiscard]] const std::string &message() const { return message_; }
  [[nodiscard]] const std::optional<Location> &input_location() const {
    return input_location_;
  }
  [[nodiscard]] const std::optional<Location> &spec_location() const {
    return spec_location_;
  }
  [[nodiscard]] const std::optional<std::string> &context() const {
    return context_;
  }
  /// The failed check's witness, outermost index first and counted from 0,
  /// or no path for an error that is not a failed check.
  [[nodiscard]] const std::optional<std::vector<std::size_t>> &
  record_path() const {
    return record_path_;
  }

private:
  ErrorCode code_;
  std::string message_;
  std::optional<Location> input_location_;
  std::optional<Location> spec_location_;
  std::optional<std::string> context_;
  std::optional<std::vector<std::size_t>> record_path_;
};

} // namespace iris
