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

  /** {@return no parameters} */
  public static Params of() {
    return NONE;
  }

  /**
   * Makes one parameter.
   *
   * @param name the name
   * @param value the text of the value
   * @return the parameter
   * @throws IllegalArgumentException if either has an unpaired surrogate, which is not text
   */
  public static Params of(String name, String value) {
    return NONE.with(name, value);
  }

  /**
   * Makes parameters from a map.
   *
   * @param values the names with their values, in the order of the parameters
   * @return the parameters
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
   * Sets a parameter. A name already present keeps its position.
   *
   * @param name the name
   * @param value the text of the value
   * @return these parameters with the change, which leaves this object as it is
   * @throws IllegalArgumentException if either has an unpaired surrogate, which is not text
   */
  public Params with(String name, String value) {
    requireText(name);
    requireText(value);
    var copy = new LinkedHashMap<>(values);
    copy.put(name, value);
    return new Params(copy);
  }

  /** {@return the parameters as an unmodifiable map, in the order they were given} */
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
