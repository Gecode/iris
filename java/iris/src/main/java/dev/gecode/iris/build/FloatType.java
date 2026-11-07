package dev.gecode.iris.build;

/** A type that reads floats. */
public final class FloatType extends Type<FloatType> {
  FloatType(Declared declared) {
    super(declared);
  }

  @Override
  Tag tag() {
    return Tag.FLOAT;
  }

  @Override
  FloatType with(Declared value) {
    return new FloatType(value);
  }
}
