package dev.gecode.iris;

import java.lang.annotation.Documented;
import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

/**
 * Names the entry that a record component reads through {@link Spec#reader}. Without it, a
 * component reads the entry of its own name, spelled exactly as the component is.
 *
 * <pre>{@code
 * record Shop(@Entry("n_jobs") long nJobs, List<long[]> machine) {}
 * }</pre>
 */
@Documented
@Retention(RetentionPolicy.RUNTIME)
@Target(ElementType.RECORD_COMPONENT)
public @interface Entry {
  /** {@return the name of the entry} */
  String value();
}
