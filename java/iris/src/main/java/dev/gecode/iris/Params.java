package dev.gecode.iris;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.SequencedMap;

/**
 * The parameters of a read, as names with text values, in the order they were given (§9.1).
 *
 * <p>The order decides which unknown parameter a read reports. A {@code Params} is immutable.
 */
public final class Params {
  private static final Params NONE = new Params(new LinkedHashMap<>());

  private final SequencedMap<String, String> values;

  private Params(SequencedMap<String, String> values) {
    this.values = Collections.unmodifiableSequencedMap(values);
  }

  /** Returns no parameters. */
  public static Params of() {
    return NONE;
  }

  /**
   * Returns the one parameter {@code name} with {@code value}.
   *
   * @throws IllegalArgumentException if either has an unpaired surrogate, which is not text
   */
  public static Params of(String name, String value) {
    return NONE.with(name, value);
  }

  /**
   * Returns the parameters {@code values}, in its iteration order.
   *
   * @throws IllegalArgumentException if a name or value has an unpaired surrogate, which is not
   *     text
   */
  public static Params of(Map<String, String> values) {
    var copy = new LinkedHashMap<String, String>();
    values.forEach((name, value) -> {
      requireText(name);
      requireText(value);
      copy.put(name, value);
    });
    return new Params(copy);
  }

  /**
   * Returns these parameters with {@code name} set to {@code value}. A name already present keeps
   * its position.
   *
   * @throws IllegalArgumentException if either has an unpaired surrogate, which is not text
   */
  public Params with(String name, String value) {
    requireText(name);
    requireText(value);
    var copy = new LinkedHashMap<>(values);
    copy.put(name, value);
    return new Params(copy);
  }

  /** Returns the parameters as an unmodifiable map, in the order they were given. */
  public SequencedMap<String, String> values() {
    return values;
  }

  private static void requireText(String text) {
    int index = 0;
    while (index < text.length()) {
      int codePoint = text.codePointAt(index);
      if (codePoint >= Character.MIN_SURROGATE && codePoint <= Character.MAX_SURROGATE) {
        throw new IllegalArgumentException("unpaired surrogate at index " + index);
      }
      index += Character.charCount(codePoint);
    }
  }
}
