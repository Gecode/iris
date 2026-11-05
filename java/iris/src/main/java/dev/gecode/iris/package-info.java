/**
 * Reads text instance files described by Iris reader specs.
 *
 * <p>{@link dev.gecode.iris.Spec} parses a reader spec and reads inputs into {@link
 * dev.gecode.iris.Data}, whose {@link dev.gecode.iris.Value}s give the entries. Errors in a reader
 * spec or an input throw {@link dev.gecode.iris.IrisException}.
 */
@NullMarked
package dev.gecode.iris;

import org.jspecify.annotations.NullMarked;
