package dev.gecode.iris.build;

import org.jspecify.annotations.Nullable;

/**
 * A domain whose comprehension variable has a chosen name, from {@link Iris#over}.
 *
 * @param <E> what the comprehension gives its body
 */
public final class Over<E> implements Domain<E> {
  final Domain<E> domain;
  final @Nullable String name;

  Over(Domain<E> domain, @Nullable String name) {
    this.domain = domain;
    this.name = name;
  }

  /**
   * {@return this domain, with its comprehension variable printed as {@code variable}}
   *
   * @param variable the name of the variable
   */
  public Over<E> named(String variable) {
    return new Over<>(domain, variable);
  }
}
