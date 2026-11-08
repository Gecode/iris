package dev.gecode.iris.build;

import java.lang.annotation.Documented;
import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

/**
 * Marks a class as a layer over this package, such as a builder in another JVM language, whose
 * frames call sites skip.
 *
 * <p>The call site of a statement is the innermost frame outside this package. Frames of a class
 * that carries this annotation, or that a class carrying it encloses, are skipped too, so that a
 * statement made through the layer points at the code that called the layer, as Rust's {@code
 * #[track_caller]} does. A Kotlin file opts in with {@code @file:TrackCaller}.
 */
@Documented
@Retention(RetentionPolicy.RUNTIME)
@Target(ElementType.TYPE)
public @interface TrackCaller {}
