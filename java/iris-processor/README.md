# iris-processor

An annotation processor that writes typed handles and a record for a reader
spec while javac compiles. It is the compile-time counterpart of
`iris gen --lang java`, and is part of the spec builder work of
[`PLAN.md`](../../PLAN.md) §8.3 and the builders brief in
[`docs/builders.md`](../../docs/builders.md).

On a top-level type annotated with `@IrisSpec("path/to/spec.iris")`, the
processor reads the file, checks the reader spec with `dev.gecode:iris`, and
writes two classes beside the type. For a type `Jobshop`, `JobshopHandles`
holds the parsed spec as `spec`, a typed handle for each entry, named and typed
as `iris gen --lang java` names and types it, and a `RecordReader` as `reader`.
`JobshopData` is the record that `reader` reads. An invalid spec, or a file the
processor cannot read, is a compile error at the annotation. A spec whose
entries take more than 253 parameter slots, where a `long` or `double`
component takes two, gets no record or reader, with a note: a method takes at
most 255 slots, and the record's constructor spends one on the object it
constructs and the method handle that `RecordReader` calls it through one on
itself. Its handles class is generated all the same. The generated classes
qualify the types of `java.lang`, so classes such as `Long` or `String` in the
annotated type's package do not take their place.

The processor is needed only to compile, and adds no dependency to
`dev.gecode:iris` or to the program: the annotation is kept only in the source,
and the generated classes depend only on `dev.gecode:iris`.

```kotlin
dependencies {
    compileOnly("dev.gecode:iris-processor:0.1.0")
    annotationProcessor("dev.gecode:iris-processor:0.1.0")
}

tasks.compileJava {
    options.compilerArgs.add("-Airis.root=${projectDir}")
    inputs.dir("specs")
}
```

The option `iris.root` names the directory that relative paths start from;
without it, a path starts at the directory of the annotated source file.
Declaring the specs as inputs recompiles when one changes. The processor leaves
`@IrisSpec` unclaimed, so that other processors still see it, and a build with
`-Xlint:all -Werror` adds `-Xlint:-processing`.

The processor uses the library's internal syntax packages to find the entries
of a spec, so it has no module descriptor and runs from the processor path. Its
naming ports the handles mode of `rust/iris-gen/src/java/names.rs`;
`java/iris-views` compares the two on every format.

## Building and testing

From `java/`: `./gradlew :iris-processor:build` compiles the processor, runs
its tests, which compile annotated sources in process with javac, and builds
its Javadoc with warnings as errors. `./gradlew :iris-views:test` compiles the
processor's classes for every format beside the views of `iris gen` and reads
every case through both.
