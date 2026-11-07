package dev.gecode.iris.build;

/** A type that reads integers. */
public sealed class IntType extends Type<IntType> permits Alphabet, Range {
  IntType(Declared declared) {
    super(declared);
  }

  @Override
  final Tag tag() {
    return Tag.INT;
  }

  @Override
  final IntType with(Declared value) {
    return new IntType(value);
  }
}
