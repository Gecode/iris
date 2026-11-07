package dev.gecode.iris.build;

import java.util.List;
import org.jspecify.annotations.Nullable;

/** The type and end of a {@code list} statement, from {@code type.until(end)}. */
public final class ListReading {
  final Tag tag;
  final Declared declared;
  final @Nullable List<Op> end;

  ListReading(Tag tag, Declared declared, @Nullable List<Op> end) {
    this.tag = tag;
    this.declared = declared;
    this.end = end;
  }

  /**
   * {@return this list with {@code offset expression}}
   *
   * @param offset the offset
   */
  public ListReading offset(Expr offset) {
    return new ListReading(tag, declared.withOffset(Exprs.node(offset).postfix()), end);
  }

  /**
   * {@return this list with {@code offset expression}}
   *
   * @param offset the offset
   */
  public ListReading offset(long offset) {
    return new ListReading(tag, declared.withOffset(Exprs.literal(offset).postfix()), end);
  }
}
