package dev.gecode.iris.build;

/** A value type as data: {@code DYN} is a value with no static type. */
enum Tag {
  INT("int"),
  FLOAT("float"),
  NUM("num"),
  STR("string"),
  DYN("dynamic");

  private final String description;

  Tag(String description) {
    this.description = description;
  }

  String description() {
    return description;
  }

  /** Whether integer arithmetic and logic take this type. */
  boolean integral() {
    return this == INT || this == DYN;
  }

  /** Whether comparisons and unary minus take this type. */
  boolean numeric() {
    return this != STR;
  }
}
