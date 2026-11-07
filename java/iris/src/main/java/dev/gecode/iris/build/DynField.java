package dev.gecode.iris.build;

import dev.gecode.iris.Data;
import dev.gecode.iris.Field;
import dev.gecode.iris.Value;
import org.jspecify.annotations.Nullable;

/**
 * A handle whose value type or depth is open: {@code data.get(handle)} returns the {@link Value},
 * or null if the entry is absent.
 *
 * <p>{@link Iris#dynamic} declares one, {@link Iris#undefined} names an entry that nothing may
 * define, and every definition returns one for the name as it stands afterwards.
 */
public final class DynField extends Field<@Nullable Value> implements DynExpr {
  final Handle handle;
  private final Node node;

  DynField(Handle handle) {
    super(handle.name());
    this.handle = handle;
    this.node = new Node(new Op.NameOp(handle));
  }

  Node node() {
    return node;
  }

  @Override
  protected @Nullable Value readFrom(Data data) {
    return (Value) Views.read(handle, data);
  }

  @Override
  public String toString() {
    return "DynField " + name();
  }
}
