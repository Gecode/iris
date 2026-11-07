package dev.gecode.iris.build;

/** A type that reads integers or floats, decided by the data. */
public final class NumType extends Type<NumType> {
  NumType(Declared declared) {
    super(declared);
  }

  @Override
  Tag tag() {
    return Tag.NUM;
  }

  @Override
  NumType with(Declared value) {
    return new NumType(value);
  }
}
