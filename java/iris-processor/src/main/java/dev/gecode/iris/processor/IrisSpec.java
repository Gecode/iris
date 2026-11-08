package dev.gecode.iris.processor;

import java.lang.annotation.Documented;
import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

/**
 * Generates typed handles and a record for the reader spec in a file, at compile time.
 *
 * <p>On a top-level type {@code Jobshop}, {@link IrisSpecProcessor} reads the file, checks the
 * reader spec, and writes two classes beside the type:
 *
 * <ul>
 *   <li>{@code JobshopHandles}, with the parsed {@code Spec} as {@code spec}, a typed handle for
 *       each entry, named and typed as {@code iris gen --lang java} names and types it, and a
 *       {@code RecordReader} as {@code reader};
 *   <li>{@code JobshopData}, a record with a component for each entry, which {@code reader} reads.
 * </ul>
 *
 * <pre>{@code
 * @IrisSpec("formats/jobshop/jobshop.iris")
 * final class Jobshop {}
 *
 * Data data = JobshopHandles.spec.read(input);
 * List<long[]> machine = data.get(JobshopHandles.machine);
 * JobshopData shop = JobshopHandles.reader.read(input);
 * }</pre>
 *
 * <p>A spec whose entries take more than 253 parameter slots in a record, where a {@code long} or
 * {@code double} component takes two, gets no record and no {@code reader}, since {@code
 * RecordReader} could not call the record's constructor; the processor notes it.
 *
 * <p>A relative path resolves against the directory that the processor option {@code iris.root}
 * names, as in {@code -Airis.root=/path/to/project}, or without the option against the directory of
 * the annotated type's source file. The generated classes keep the text of the spec, so the file is
 * not needed at run time; a build that should regenerate them when the file changes declares it as
 * an input of the compilation.
 *
 * <p>The annotation is kept only in the source, so the processor's jar is needed only to compile.
 */
@Documented
@Retention(RetentionPolicy.SOURCE)
@Target(ElementType.TYPE)
public @interface IrisSpec {
  /**
   * The path of the reader spec.
   *
   * @return the path, absolute or relative as the annotation's documentation says
   */
  String value();
}
