package dev.gecode.iris;

import dev.gecode.iris.internal.read.CanonicalJson;
import dev.gecode.iris.internal.read.Dzn;
import dev.gecode.iris.internal.read.Stored;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.SequencedMap;
import org.jspecify.annotations.Nullable;

/** The entries that a read or a catalog produced, in definition order (§3.1). */
public final class Data {
  private final SequencedMap<String, Stored> stored;
  private final List<Entry> entries;
  private final Map<String, Value> values;

  Data(SequencedMap<String, Stored> stored) {
    this.stored = stored;
    List<Entry> list = new ArrayList<>(stored.size());
    Map<String, Value> byName = new HashMap<>();
    stored.forEach((name, value) -> {
      Value wrapped = Value.ArrayValue.of(value);
      list.add(new Entry(name, wrapped));
      byName.put(name, wrapped);
    });
    this.entries = Collections.unmodifiableList(list);
    this.values = byName;
  }

  /**
   * An entry.
   *
   * @param name the name
   * @param value the value
   */
  public record Entry(String name, Value value) {}

  /** {@return the entries, in definition order} */
  public List<Entry> entries() {
    return entries;
  }

  /**
   * Finds an entry.
   *
   * @param name the name of the entry
   * @return its value, or null if there is none
   */
  public @Nullable Value find(String name) {
    return values.get(name);
  }

  /**
   * Gets an entry.
   *
   * @param name the name of the entry
   * @return its value
   * @throws IllegalStateException if there is no such entry
   */
  public Value get(String name) {
    Value value = values.get(name);
    if (value == null) {
      throw new IllegalStateException("no value named '" + name + "'");
    }
    return value;
  }

  /** {@return the canonical data document (§3.3), with its final newline} */
  public String toJson() {
    return CanonicalJson.data(stored);
  }

  /** {@return the dzn document (appendix B), which is empty for data without entries} */
  public String toDzn() {
    return Dzn.data(stored);
  }
}
