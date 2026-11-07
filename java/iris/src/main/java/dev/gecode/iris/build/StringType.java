package dev.gecode.iris.build;

/** A type that reads strings. */
public final class StringType extends Type<StringType> {
  StringType(Declared declared) {
    super(declared);
  }

  @Override
  Tag tag() {
    return Tag.STR;
  }

  @Override
  StringType with(Declared value) {
    return new StringType(value);
  }
}
