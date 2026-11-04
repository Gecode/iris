package dev.gecode.iris;

/**
 * A stable identifier for a kind of error, from the Specification's error registry.
 *
 * <p>Compare codes rather than messages: the wording of a message can change.
 */
public enum ErrorCode {
  SPEC_UTF8("E-SPEC-UTF8"),
  SPEC_STRING("E-SPEC-STRING"),
  SPEC_NUMBER("E-SPEC-NUMBER"),
  SPEC_CHARACTER("E-SPEC-CHARACTER"),
  SPEC_PATTERN("E-SPEC-PATTERN"),
  SPEC_SYNTAX("E-SPEC-SYNTAX"),
  SPEC_PLACEMENT("E-SPEC-PLACEMENT"),
  SPEC_NAME("E-SPEC-NAME"),
  SPEC_NAME_CONFLICT("E-SPEC-NAME-CONFLICT"),
  SPEC_UNKNOWN_TYPE("E-SPEC-UNKNOWN-TYPE"),
  SPEC_TYPE("E-SPEC-TYPE"),
  SPEC_NESTING("E-SPEC-NESTING"),
  SPEC_FUNCTION("E-SPEC-FUNCTION"),
  SPEC_CONSTANT("E-SPEC-CONSTANT"),
  SPEC_RECORD_SCOPE("E-SPEC-RECORD-SCOPE"),
  SPEC_FIELD("E-SPEC-FIELD"),
  SPEC_RECORD_VALUE("E-SPEC-RECORD-VALUE"),
  SPEC_CHECK_SCOPE("E-SPEC-CHECK-SCOPE");

  private final String code;

  ErrorCode(String code) {
    this.code = code;
  }

  /** Returns the text of this code, such as {@code E-SPEC-SYNTAX}. */
  public String code() {
    return code;
  }
}
