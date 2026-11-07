package dev.gecode.iris.build;

import java.util.List;

/** Something a statement mentions: an expression, a handle in a pattern, or a named type. */
sealed interface Mention {
  record Expression(List<Op> ops) implements Mention {}

  record HandleUse(Handle handle) implements Mention {}

  record TypeUse(Declared.NamedSpec type) implements Mention {}
}
