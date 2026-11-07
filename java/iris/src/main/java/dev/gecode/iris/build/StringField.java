package dev.gecode.iris.build;

import dev.gecode.iris.Data;
import dev.gecode.iris.Field;
import org.jspecify.annotations.Nullable;

/**
 * A handle of a string entry. In a builder it is an expression; after reading, {@code
 * data.get(handle)} returns the entry as {@code V}.
 *
 * <p>The factories such as {@link Iris#string0} make handles that any builder may use: the first
 * use in a builder declares the name with the handle's type, depth and presence, and {@link
 * Builder#build} checks every definition against them.
 *
 * @param <V> the Java type of the entry's value
 */
public final class StringField<V extends @Nullable Object> extends Field<V> implements StringExpr {
  final Handle handle;
  private final Node node;

  StringField(Handle handle) {
    super(handle.name());
    this.handle = handle;
    this.node = new Node(new Op.NameOp(handle));
  }

  Node node() {
    return node;
  }

  /**
   * {@return this handle for an entry that may be absent, for which {@code get} returns null}
   *
   * @throws IllegalStateException for a handle from {@code forward} or a definition, whose presence
   *     its builder fixed
   */
  public StringField<@Nullable V> optional() {
    return new StringField<>(Handles.optional(handle));
  }

  @Override
  @SuppressWarnings({"unchecked", "NullAway"})
  protected V readFrom(Data data) {
    return (V) Views.read(handle, data);
  }

  @Override
  public String toString() {
    return "StringField " + name();
  }
}
