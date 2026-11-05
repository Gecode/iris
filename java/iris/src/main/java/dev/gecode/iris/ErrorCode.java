package dev.gecode.iris;

/**
 * A stable identifier for a kind of error, from the Specification's error registry.
 *
 * <p>Compare codes rather than messages: the wording of a message can change.
 */
public enum ErrorCode {
  /** Reader spec is not valid UTF-8. */
  SPEC_UTF8("E-SPEC-UTF8"),
  /** Invalid reader-spec string. */
  SPEC_STRING("E-SPEC-STRING"),
  /** Invalid or unrepresentable reader-spec numeric literal. */
  SPEC_NUMBER("E-SPEC-NUMBER"),
  /** Unrecognized reader-spec character. */
  SPEC_CHARACTER("E-SPEC-CHARACTER"),
  /** Malformed pattern or capture, or an invalid capture type. */
  SPEC_PATTERN("E-SPEC-PATTERN"),
  /** Required syntactic item absent or unrecognized statement. */
  SPEC_SYNTAX("E-SPEC-SYNTAX"),
  /** Statement or setting used in a prohibited position. */
  SPEC_PLACEMENT("E-SPEC-PLACEMENT"),
  /** Reserved or discard name used where a real definition or binding is required. */
  SPEC_NAME("E-SPEC-NAME"),
  /** Conflicting type, value, record, or comprehension names. */
  SPEC_NAME_CONFLICT("E-SPEC-NAME-CONFLICT"),
  /** Reading type name is not known at this point. */
  SPEC_UNKNOWN_TYPE("E-SPEC-UNKNOWN-TYPE"),
  /** Reading type incompatible with a declaration, list or option. */
  SPEC_TYPE("E-SPEC-TYPE"),
  /** Block or expression nesting limit of the reader spec exceeded. */
  SPEC_NESTING("E-SPEC-NESTING"),
  /** Unknown function, or an invalid call form or arity. */
  SPEC_FUNCTION("E-SPEC-FUNCTION"),
  /** Unknown Gecode host constant. */
  SPEC_CONSTANT("E-SPEC-CONSTANT"),
  /** Reference to a record, list or field outside its permitted repeat scope. */
  SPEC_RECORD_SCOPE("E-SPEC-RECORD-SCOPE"),
  /** Invalid field selection or ambiguous field definition. */
  SPEC_FIELD("E-SPEC-FIELD"),
  /** Record or list used as a value, an indexed record or a comprehension list where prohibited. */
  SPEC_RECORD_VALUE("E-SPEC-RECORD-VALUE"),
  /** Check attempts to range over incompatible or ambiguous repeat definitions. */
  SPEC_CHECK_SCOPE("E-SPEC-CHECK-SCOPE"),
  /** A work budget is exhausted. */
  BUDGET("E-BUDGET"),
  /** A check condition is false. */
  CHECK_FAILED("E-CHECK-FAILED"),
  /** Runtime record collections cannot be aligned for a check. */
  CHECK_RECORDS("E-CHECK-RECORDS"),
  /** Comparison operands violate scalar or numeric requirements. */
  COMPARISON_TYPE("E-COMPARISON-TYPE"),
  /** A size, count or base expression is not a permitted count. */
  COUNT("E-COUNT"),
  /** Integer division or remainder by zero. */
  DIVISION_ZERO("E-DIVISION-ZERO"),
  /** Required literal token absent or different. */
  EXPECTED_TOKEN("E-EXPECTED-TOKEN"),
  /** Input does not satisfy the reading type or domain, or the line of values is absent. */
  EXPECTED_VALUE("E-EXPECTED-VALUE"),
  /** Integer exponent is negative. */
  EXPONENT("E-EXPONENT"),
  /** A field is unavailable in the selected runtime record. */
  FIELD_UNDEFINED("E-FIELD-UNDEFINED"),
  /** No remaining line matches a find pattern. */
  FIND_MATCH("E-FIND-MATCH"),
  /** A {@code min} or {@code max} receives no integer leaves. */
  FUNCTION_EMPTY("E-FUNCTION-EMPTY"),
  /** Function or comprehension operand has the wrong type or shape. */
  FUNCTION_TYPE("E-FUNCTION-TYPE"),
  /** An {@code index} statement executes outside a repeat. */
  INDEX_CONTEXT("E-INDEX-CONTEXT"),
  /** Input row index differs from the expected repeat index. */
  INDEX_MISMATCH("E-INDEX-MISMATCH"),
  /** Array or record-list index is outside its bounds. */
  INDEX_RANGE("E-INDEX-RANGE"),
  /** Attempt to index a scalar. */
  INDEX_SCALAR("E-INDEX-SCALAR"),
  /** Index expression is not an integer scalar. */
  INDEX_TYPE("E-INDEX-TYPE"),
  /** Reading requires an instance but none exists. */
  INSTANCE_EMPTY("E-INSTANCE-EMPTY"),
  /** Requested instance number or name does not exist. */
  INSTANCE_NOT_FOUND("E-INSTANCE-NOT-FOUND"),
  /** Numeric instance selection is below one. */
  INSTANCE_NUMBER("E-INSTANCE-NUMBER"),
  /** Multiple instances require an explicit selection. */
  INSTANCE_SELECTION("E-INSTANCE-SELECTION"),
  /** Checked integer arithmetic or value adjustment overflows. */
  INTEGER_OVERFLOW("E-INTEGER-OVERFLOW"),
  /** Integer arithmetic requires integer scalar operands. */
  INTEGER_TYPE("E-INTEGER-TYPE"),
  /** Layout value, name, or matrix shape is invalid. */
  LAYOUT("E-LAYOUT"),
  /** No match for a line pattern, either for {@code line} or a required {@code sections} arm. */
  LINE_MATCH("E-LINE-MATCH"),
  /** List terminator has an invalid type or shape. */
  LIST_TERMINATOR("E-LIST-TERMINATOR"),
  /** Referenced value or record collection is not defined yet. */
  NAME_UNDEFINED("E-NAME-UNDEFINED"),
  /** An instance or indefinite repeat body consumes no input. */
  NO_PROGRESS("E-NO-PROGRESS"),
  /** Offset expression is not an integer scalar. */
  OFFSET_TYPE("E-OFFSET-TYPE"),
  /** Required parameter was not supplied. */
  PARAM_MISSING("E-PARAM-MISSING"),
  /** Supplied parameter cannot convert to its default's numeric value type. */
  PARAM_TYPE("E-PARAM-TYPE"),
  /** Caller supplied a parameter not declared by the reader spec. */
  PARAM_UNKNOWN("E-PARAM-UNKNOWN"),
  /** Current-value pattern reference is neither a string nor an integer scalar. */
  PATTERN_REFERENCE("E-PATTERN-REFERENCE"),
  /** Range bound is not an integer scalar. */
  RANGE_BOUNDS("E-RANGE-BOUNDS"),
  /** Repeated values have inconsistent depths. */
  REPEAT_DEPTH("E-REPEAT-DEPTH"),
  /** Required scalar is absent from some or all nonempty repeat iterations. */
  REPEAT_MISSING("E-REPEAT-MISSING"),
  /** Required terminating token is absent before input ends. */
  REPEAT_TERMINATOR("E-REPEAT-TERMINATOR"),
  /** A sections arm matches a second line. */
  SECTIONS_REPEATED("E-SECTIONS-REPEATED"),
  /** No sections arm matches the current line and skip other is absent. */
  SECTIONS_UNEXPECTED("E-SECTIONS-UNEXPECTED"),
  /** Array or matrix cell count exceeds the supported structural bound. */
  SIZE_LIMIT("E-SIZE-LIMIT"),
  /** A requested line cannot be skipped at end of input. */
  SKIP_EOF("E-SKIP-EOF"),
  /** Alphabet reading encounters end of input or an unrecognized symbol. */
  SYMBOL("E-SYMBOL"),
  /** Disallowed input remains after execution finishes. */
  TRAILING_INPUT("E-TRAILING-INPUT"),
  /** A parsed named type has not been evaluated on the executed path. */
  TYPE_UNDEFINED("E-TYPE-UNDEFINED"),
  /** Repeated or list values mix strings and numbers. */
  VALUE_TYPE_MIX("E-VALUE-TYPE-MIX"),
  /** The supplied input or a word or rest value that was read contains invalid UTF-8. */
  UTF8("E-UTF8");

  private final String code;

  ErrorCode(String code) {
    this.code = code;
  }

  /** {@return the text of this code, such as {@code E-SPEC-SYNTAX}} */
  public String code() {
    return code;
  }
}
