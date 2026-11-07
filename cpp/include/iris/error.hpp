#pragma once

#include <cstddef>
#include <optional>
#include <source_location>
#include <stdexcept>
#include <string>
#include <string_view>
#include <vector>

namespace iris {

namespace detail {
struct ErrorAccess;
} // namespace detail

/// A registered error code of the Iris Language Specification (chapter 11).
enum class ErrorCode {
  SpecUtf8,         ///< `E-SPEC-UTF8`
  SpecString,       ///< `E-SPEC-STRING`
  SpecNumber,       ///< `E-SPEC-NUMBER`
  SpecCharacter,    ///< `E-SPEC-CHARACTER`
  SpecPattern,      ///< `E-SPEC-PATTERN`
  SpecSyntax,       ///< `E-SPEC-SYNTAX`
  SpecPlacement,    ///< `E-SPEC-PLACEMENT`
  SpecName,         ///< `E-SPEC-NAME`
  SpecNameConflict, ///< `E-SPEC-NAME-CONFLICT`
  SpecUnknownType,  ///< `E-SPEC-UNKNOWN-TYPE`
  SpecType,         ///< `E-SPEC-TYPE`
  SpecNesting,      ///< `E-SPEC-NESTING`
  SpecFunction,     ///< `E-SPEC-FUNCTION`
  SpecConstant,     ///< `E-SPEC-CONSTANT`
  SpecRecordScope,  ///< `E-SPEC-RECORD-SCOPE`
  SpecField,        ///< `E-SPEC-FIELD`
  SpecRecordValue,  ///< `E-SPEC-RECORD-VALUE`
  SpecCheckScope,   ///< `E-SPEC-CHECK-SCOPE`
  // Read-time codes (§11.5), then E-UTF8 (§11.6)
  Budget,             ///< `E-BUDGET`
  CheckFailed,        ///< `E-CHECK-FAILED`
  CheckRecords,       ///< `E-CHECK-RECORDS`
  ComparisonType,     ///< `E-COMPARISON-TYPE`
  Count,              ///< `E-COUNT`
  DivisionZero,       ///< `E-DIVISION-ZERO`
  ExpectedToken,      ///< `E-EXPECTED-TOKEN`
  ExpectedValue,      ///< `E-EXPECTED-VALUE`
  Exponent,           ///< `E-EXPONENT`
  FieldUndefined,     ///< `E-FIELD-UNDEFINED`
  FindMatch,          ///< `E-FIND-MATCH`
  FunctionEmpty,      ///< `E-FUNCTION-EMPTY`
  FunctionType,       ///< `E-FUNCTION-TYPE`
  IndexContext,       ///< `E-INDEX-CONTEXT`
  IndexMismatch,      ///< `E-INDEX-MISMATCH`
  IndexRange,         ///< `E-INDEX-RANGE`
  IndexScalar,        ///< `E-INDEX-SCALAR`
  IndexType,          ///< `E-INDEX-TYPE`
  InstanceEmpty,      ///< `E-INSTANCE-EMPTY`
  InstanceNotFound,   ///< `E-INSTANCE-NOT-FOUND`
  InstanceNumber,     ///< `E-INSTANCE-NUMBER`
  InstanceSelection,  ///< `E-INSTANCE-SELECTION`
  IntegerOverflow,    ///< `E-INTEGER-OVERFLOW`
  IntegerType,        ///< `E-INTEGER-TYPE`
  Layout,             ///< `E-LAYOUT`
  LineMatch,          ///< `E-LINE-MATCH`
  ListTerminator,     ///< `E-LIST-TERMINATOR`
  NameUndefined,      ///< `E-NAME-UNDEFINED`
  NoProgress,         ///< `E-NO-PROGRESS`
  OffsetType,         ///< `E-OFFSET-TYPE`
  ParamMissing,       ///< `E-PARAM-MISSING`
  ParamType,          ///< `E-PARAM-TYPE`
  ParamUnknown,       ///< `E-PARAM-UNKNOWN`
  PatternReference,   ///< `E-PATTERN-REFERENCE`
  RangeBounds,        ///< `E-RANGE-BOUNDS`
  RepeatDepth,        ///< `E-REPEAT-DEPTH`
  RepeatMissing,      ///< `E-REPEAT-MISSING`
  RepeatTerminator,   ///< `E-REPEAT-TERMINATOR`
  SectionsRepeated,   ///< `E-SECTIONS-REPEATED`
  SectionsUnexpected, ///< `E-SECTIONS-UNEXPECTED`
  SizeLimit,          ///< `E-SIZE-LIMIT`
  SkipEof,            ///< `E-SKIP-EOF`
  Symbol,             ///< `E-SYMBOL`
  TrailingInput,      ///< `E-TRAILING-INPUT`
  TypeUndefined,      ///< `E-TYPE-UNDEFINED`
  ValueTypeMix,       ///< `E-VALUE-TYPE-MIX`
  Utf8,               ///< `E-UTF8`
};

/// Return the registry spelling of `code`, such as `E-SPEC-SYNTAX`.
std::string_view code_name(ErrorCode code);

/// A position in a reader spec or an input. Lines and columns count from 1.
struct Location {
  /// The name of the reader spec or input
  std::string origin;
  /// The line, if the position has one
  std::optional<std::size_t> line;
  /// The column, if the position has one
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
  /// The host code that wrote the statement the error belongs to, for a
  /// reader spec from a spec builder (see iris/build.hpp)
  std::optional<std::source_location> call_site = std::nullopt;
};

/**
 * A language error with the members of the canonical error document
 * (Specification §3.7), and everything its diagnostic shows
 *
 * `what()` renders the diagnostic of Specification §11.1: it starts with
 * `origin:line:column: message`, or `origin:line: message` without a
 * column. A read-time error goes on with the excerpt and a caret under the
 * column, then `  while reading: ` and the context, `  spec: origin:line`
 * for the statement, `call site: file:line:column` for the call site,
 * and `  after an earlier error in the instance: ` and the earlier
 * rejection, each on a line of its own if the error has it.
 */
class Error : public std::runtime_error {
public:
  /// An error with the code `code`, the message `message` and the parts
  /// `details`
  Error(ErrorCode code, std::string message, ErrorDetails details = {});

  /// Return the error code
  [[nodiscard]] ErrorCode code() const { return code_; }
  /// Return the message, without a location
  [[nodiscard]] const std::string &message() const { return message_; }
  /// Return the parts besides the code and message
  [[nodiscard]] const ErrorDetails &details() const { return details_; }
  /// Return where in the input the error is, for a read-time error
  [[nodiscard]] const std::optional<Location> &input_location() const {
    return details_.input_location;
  }
  /// Return where in the reader spec the error is: the statement that runs
  /// for a read-time error
  [[nodiscard]] const std::optional<Location> &spec_location() const {
    return details_.spec_location;
  }
  /// Return the input line at the input location (see ErrorDetails)
  [[nodiscard]] const std::optional<std::string> &excerpt() const {
    return details_.excerpt;
  }
  /// Return the statements that run, outermost first (see ErrorDetails)
  [[nodiscard]] const std::optional<std::string> &context() const {
    return details_.context;
  }
  /// The failed check's witness, outermost index first and counted from 0,
  /// or no path for an error that is not a failed check.
  [[nodiscard]] const std::optional<std::vector<std::size_t>> &
  record_path() const {
    return details_.record_path;
  }
  /// Return the summary of an earlier rejection in the same instance, which
  /// may explain this error
  [[nodiscard]] const std::optional<std::string> &earlier_rejection() const {
    return details_.earlier_rejection;
  }
  /// Return the host code that wrote the statement the error belongs to,
  /// for a reader spec from a spec builder; a spec parsed from text has no
  /// call sites
  [[nodiscard]] const std::optional<std::source_location> &call_site() const {
    return details_.call_site;
  }

private:
  friend struct detail::ErrorAccess;

  ErrorCode code_;
  std::string message_;
  ErrorDetails details_;
  /// The reader-spec line of the statement the error belongs to, when the
  /// error lies past it: at the token after it, at the end of its block,
  /// or at a statement a repeat or a sections arm ran
  std::optional<std::size_t> owner_line_;
};

} // namespace iris
