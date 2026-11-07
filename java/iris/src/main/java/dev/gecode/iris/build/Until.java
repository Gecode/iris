package dev.gecode.iris.build;

import dev.gecode.iris.Field;
import java.util.List;
import org.jspecify.annotations.Nullable;

/**
 * {@code until "text"...}: the terminators that end a repeat or sections. {@link #store} keeps the
 * terminator that ended it.
 */
public final class Until implements SectionClause {
  final List<String> terminators;
  final @Nullable Field<?> store;

  Until(List<String> terminators, @Nullable Field<?> store) {
    this.terminators = terminators;
    this.store = store;
  }

  /**
   * {@return these terminators with {@code store name}, which keeps the one that ended the block in
   * the entry of {@code target}, a string at the depth of the statement}
   *
   * @param target the handle of the entry; {@code forward("_", Str.class)} names the entry {@code
   *     _}
   */
  public Until store(Field<?> target) {
    return new Until(terminators, target);
  }
}
