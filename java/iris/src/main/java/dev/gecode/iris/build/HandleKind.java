package dev.gecode.iris.build;

import dev.gecode.iris.Field;

/**
 * The kind of handle that {@link Iris#forward} makes, named by a class literal such as {@code
 * Int.class}.
 *
 * @param <H> the handle class
 */
public sealed interface HandleKind<H extends Field<?>> permits Int, Real, Num, Str, Dyn {}
