package dev.gecode.iris;

import org.jspecify.annotations.Nullable;

/**
 * A typed handle: the name of an entry, and the Java type {@code V} that {@link Data#get(Field)}
 * returns for it.
 *
 * <p>Handles come from {@code dev.gecode.iris.build}, whose factories fix the value type and depth,
 * as {@code int2("machine")} gives an {@code IntField<List<long[]>>}. A spec builder uses them in
 * statements and expressions; after reading, {@code data.get(handle)} returns the entry as {@code
 * V}. A handle whose entry may be absent has a nullable {@code V}, and {@code get} returns null for
 * it.
 *
 * @param <V> the type of the entry's value
 */
public abstract class Field<V extends @Nullable Object> {
  private final String name;

  /**
   * Creates a handle. Only the handle classes of {@code dev.gecode.iris.build} extend this class.
   *
   * @param name the name of the entry
   */
  protected Field(String name) {
    this.name = name;
  }

  /** {@return the name of the entry} */
  public final String name() {
    return name;
  }

  /**
   * Reads the entry of this handle from {@code data}.
   *
   * @param data the data that a read produced
   * @return the value of the entry, or null for an absent entry of an optional handle
   * @throws IllegalStateException if the entry is missing, or has another type or depth
   */
  protected abstract V readFrom(Data data);
}
