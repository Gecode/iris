# Java and Kotlin changelog

## Unreleased

- Add `dev.gecode:iris`, a native Java 27 implementation of Iris 0.1 in the
  module `dev.gecode.iris`, with no dependency but the JSpecify annotations. It
  parses and checks reader specs, reads input into columnar data under the work
  budgets of the Specification, lists the instances of an input, and writes JSON
  and dzn. Values and errors are sealed, and the whole API is `@NullMarked`.
- Read data into records through `Spec.reader`, which checks the record's
  components against the spec's entries once, when the reader is created.
- Add the spec builder, `dev.gecode.iris.build`, which builds any Iris 0.1
  reader spec from Java code with typed handles, names the caller's line in its
  errors and in the errors of reading a built spec, and reads entries with
  `Data.get(Field<V>)` as the handle's Java type.
- Add `dev.gecode:iris-kotlin`, in the module `dev.gecode.iris.kotlin`, with
  operator `get`, primitive array and `Sequence` extensions, `Params` from a
  map, and a builder DSL. It depends only on `dev.gecode:iris` and the Kotlin
  standard library.
- Add `dev.gecode:iris-processor`, an annotation processor that writes typed
  handles and a record for an `@IrisSpec` reader spec while javac compiles, the
  compile-time counterpart of `iris gen --lang java`.
- Publish all three to Maven Central from a `java-vVERSION` tag, signed and with
  sources and javadoc jars.
