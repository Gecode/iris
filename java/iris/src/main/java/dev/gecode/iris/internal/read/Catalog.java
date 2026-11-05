package dev.gecode.iris.internal.read;

import dev.gecode.iris.ValueType;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.SequencedMap;
import java.util.function.Supplier;
import org.jspecify.annotations.Nullable;

/** What a catalog learns about the instances of an input (§9.6). */
final class Catalog {
  /** The number of instances. A reader spec without an instances block reads a single one. */
  private int instances = 1;

  private final List<String> names = new ArrayList<>();

  /** The first rejection of each finished instance, or the empty string. */
  private final List<String> rejections = new ArrayList<>();

  /** The first rejection of the instance being read, or null. */
  private @Nullable String rejection = null;

  /** Records a rejection of the instance being read, unless it already has one. */
  void reject(Supplier<String> summary) {
    if (rejection == null) {
      rejection = summary.get();
    }
  }

  /** Finishes the instance being read, whose derived name is {@code name}. */
  void finishInstance(String name) {
    names.add(name);
    rejections.add(rejection == null ? "" : rejection);
    rejection = null;
  }

  /** Sets the number of instances of an instances block that has ended. */
  void setInstances(int count) {
    instances = count;
  }

  /** Returns the entries of the result. */
  SequencedMap<String, Stored> entries() {
    SequencedMap<String, Stored> entries = new LinkedHashMap<>();
    entries.put("instances", new Stored.Int(instances));
    if (!names.isEmpty() && names.stream().noneMatch(String::isEmpty)) {
      entries.put("name", strings(names));
    }
    if (rejections.stream().anyMatch(summary -> !summary.isEmpty())) {
      entries.put("failed", strings(rejections));
    }
    return entries;
  }

  private static Stored strings(List<String> values) {
    Leaves leaves = new Leaves(ValueType.STRING);
    values.forEach(leaves::add);
    return Table.row(leaves);
  }
}
