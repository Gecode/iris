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

/// The parts of an Error besides its code and message, each absent unless
/// the error has it
struct ErrorDetails {
  /// Where in the input the error is, for a read-time error
  std::optional<Location> input_location = std::nullopt;
  /// Where in the reader spec the error is: the statement that runs for a
  /// read-time error
  std::optional<Location> spec_location = std::nullopt;
  /// The input line at the input location without trailing blanks, cut
  /// after 100 bytes and then followed by `...`
  std::optional<std::string> excerpt = std::nullopt;
  /// The statements that run, outermost first and separated by ` > `, such
  /// as `repeat [2 of 5] > list clause`
  std::optional<std::string> context = std::nullopt;
  /// The failed check's witness, outermost index first and counted from 0
  std::optional<std::vector<std::size_t>> record_path = std::nullopt;
  /// The summary of an earlier rejection in the same instance, which may
  /// explain this error
  std::optional<std::string> earlier_rejection = std::nullopt;
};

/**
 * A language error with the members of the canonical error document
 * (Specification §3.7), and everything its diagnostic shows
 *
 * `what()` renders the diagnostic of Specification §11.1: it starts with
 * `origin:line:column: message`, or `origin:line: message` without a
 * column. A read-time error goes on with the excerpt and a caret under the
 * column, then `  while reading: ` and the context, `  spec: origin:line`
 * for the statement, and `  after an earlier error in the instance: ` and
 * the earlier rejection, each on a line of its own if the error has it.
 */
class Error : public std::runtime_error {
public:
  Error(ErrorCode code, std::string message, ErrorDetails details = {});

  [[nodiscard]] ErrorCode code() const { return code_; }
  [[nodiscard]] const std::string &message() const { return message_; }
  [[nodiscard]] const ErrorDetails &details() const { return details_; }
  [[nodiscard]] const std::optional<Location> &input_location() const {
    return details_.input_location;
  }
  [[nodiscard]] const std::optional<Location> &spec_location() const {
    return details_.spec_location;
  }
  [[nodiscard]] const std::optional<std::string> &excerpt() const {
    return details_.excerpt;
  }
  [[nodiscard]] const std::optional<std::string> &context() const {
    return details_.context;
  }
  /// The failed check's witness, outermost index first and counted from 0,
  /// or no path for an error that is not a failed check.
  [[nodiscard]] const std::optional<std::vector<std::size_t>> &
  record_path() const {
    return details_.record_path;
  }
  [[nodiscard]] const std::optional<std::string> &earlier_rejection() const {
    return details_.earlier_rejection;
  }

private:
  ErrorCode code_;
  std::string message_;
  ErrorDetails details_;
};

} // namespace iris
