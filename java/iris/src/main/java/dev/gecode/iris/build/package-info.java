/**
 * Builds Iris reader specs from Java code, with typed handles to read the data.
 *
 * <p>{@link dev.gecode.iris.build.Iris} holds the statements, handle factories, types, patterns and
 * expression functions; import its members statically. A handle such as {@code int2("machine")} is
 * an {@link dev.gecode.iris.build.IntField}, which is both an expression in the spec and a {@link
 * dev.gecode.iris.Field} that {@link dev.gecode.iris.Data#get(Field)} reads with its Java type.
 *
 * <p>The builder emits Iris text and parses it with {@link dev.gecode.iris.Spec#parse(String,
 * String)}, so a built spec reads exactly as its text does. {@link
 * dev.gecode.iris.build.Builder#source} gives that text. A mistake throws {@link
 * dev.gecode.iris.build.BuildException}, whose call site is the line of the caller's code that made
 * the statement.
 */
@NullMarked
package dev.gecode.iris.build;

import dev.gecode.iris.Field;
import org.jspecify.annotations.NullMarked;
