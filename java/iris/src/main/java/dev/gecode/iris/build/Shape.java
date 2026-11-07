package dev.gecode.iris.build;

import org.jspecify.annotations.Nullable;

/**
 * What a handle promises about the definitions of its name.
 *
 * @param value the value type
 * @param levels the total depth, or null when it is open
 * @param required whether some definition always executes
 */
record Shape(Tag value, @Nullable Integer levels, boolean required) {
  /**
   * Whether a definition certainly has this shape. A value type or depth that is not known agrees
   * only with an open promise.
   */
  boolean agrees(Tag definition, @Nullable Integer definitionLevels) {
    boolean valueAgrees =
        switch (value) {
          case DYN -> true;
          case NUM -> definition == Tag.INT || definition == Tag.FLOAT || definition == Tag.NUM;
          default -> value == definition;
        };
    return valueAgrees && (levels == null || levels.equals(definitionLevels));
  }

  static String describe(Tag value, @Nullable Integer levels, String openDepth) {
    return levels == null
        ? value.description() + " at " + openDepth + " depth"
        : value.description() + " at depth " + levels;
  }
}
