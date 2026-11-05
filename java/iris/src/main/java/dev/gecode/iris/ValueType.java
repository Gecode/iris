package dev.gecode.iris;

/** The type of the leaves of a value. */
public enum ValueType {
  /** A 64-bit integer. */
  INT("int"),
  /** A 64-bit float. */
  FLOAT("float"),
  /** A string. */
  STRING("string");

  private final String text;

  ValueType(String text) {
    this.text = text;
  }

  /** {@return the name of this type in canonical JSON, such as {@code int}} */
  public String text() {
    return text;
  }
}
