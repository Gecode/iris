package dev.gecode.iris;

/** The type of the leaves of a value. */
public enum ValueType {
  INT("int"),
  FLOAT("float"),
  STRING("string");

  private final String text;

  ValueType(String text) {
    this.text = text;
  }

  /** Returns the name of this type in canonical JSON, such as {@code int}. */
  public String text() {
    return text;
  }
}
