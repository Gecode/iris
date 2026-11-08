package dev.gecode.iris.processor;

import java.util.HashMap;
import java.util.HashSet;
import java.util.Locale;
import java.util.Map;
import java.util.Set;

/**
 * Gives each entry of a reader spec a Java identifier that nothing else has, as {@code iris gen
 * --lang java} does: lower camel case, cut to 24 characters, with the first numeric suffix from 2
 * that leaves it free of keywords, of the generated class's own members and of what a record
 * component may not be named.
 *
 * <p>This ports the handles mode of {@code rust/iris-gen/src/java/names.rs}; the tests of {@code
 * java/iris-views} compare the two on every format.
 */
final class Identifiers {
  /** Java's keywords, literals and contextual keywords, as of Java 27, and {@code _}. */
  private static final Set<String> KEYWORDS =
      Set.of(
          "_",
          "abstract",
          "assert",
          "boolean",
          "break",
          "byte",
          "case",
          "catch",
          "char",
          "class",
          "const",
          "continue",
          "default",
          "do",
          "double",
          "else",
          "enum",
          "exports",
          "extends",
          "false",
          "final",
          "finally",
          "float",
          "for",
          "goto",
          "if",
          "implements",
          "import",
          "instanceof",
          "int",
          "interface",
          "long",
          "module",
          "native",
          "new",
          "null",
          "open",
          "opens",
          "package",
          "permits",
          "private",
          "protected",
          "provides",
          "public",
          "record",
          "requires",
          "return",
          "sealed",
          "short",
          "static",
          "strictfp",
          "super",
          "switch",
          "synchronized",
          "this",
          "throw",
          "throws",
          "to",
          "transient",
          "transitive",
          "true",
          "try",
          "uses",
          "var",
          "void",
          "volatile",
          "when",
          "while",
          "with",
          "yield");

  /**
   * The members that the generated class adds beside the handles, the names that a record component
   * may not have, and {@code java}, which would hide the package of the qualified names that the
   * generated class uses in expressions.
   */
  private static final Set<String> VIEW_NAMES =
      Set.of(
          "clone",
          "finalize",
          "getClass",
          "hashCode",
          "java",
          "notify",
          "notifyAll",
          "reader",
          "spec",
          "toString",
          "wait");

  /** The widest identifier before any suffix. */
  private static final int WIDEST_NAME = 24;

  private final Set<String> taken = new HashSet<>();

  /** The first suffix of each base that {@link #unused} has not tried. */
  private final Map<String, Integer> nextSuffixes = new HashMap<>();

  /** Returns the identifier of the entry {@code name}, which no earlier call has returned. */
  String of(String name) {
    String base = camelCase(name, "name");
    return unused(base.length() > WIDEST_NAME ? base.substring(0, WIDEST_NAME) : base);
  }

  private String unused(String base) {
    if (!reserved(base) && taken.add(base)) {
      return base;
    }
    int number = nextSuffixes.getOrDefault(base, 2);
    String candidate = base + number;
    while (reserved(candidate) || taken.contains(candidate)) {
      number++;
      candidate = base + number;
    }
    nextSuffixes.put(base, number + 1);
    taken.add(candidate);
    return candidate;
  }

  private static boolean reserved(String name) {
    return KEYWORDS.contains(name) || VIEW_NAMES.contains(name);
  }

  /**
   * Returns {@code name} in lower camel case: other characters separate words, each word after the
   * first starts with a capital, and a name that starts with a digit gets {@code fallback} first.
   */
  static String camelCase(String name, String fallback) {
    var words = new java.util.ArrayList<String>();
    var word = new StringBuilder();
    for (int index = 0; index < name.length(); index++) {
      char character = name.charAt(index);
      if (!isAsciiAlphanumeric(character)) {
        if (!word.isEmpty()) {
          words.add(word.toString());
          word.setLength(0);
        }
        continue;
      }
      if (isAsciiUppercase(character) && index > 0 && !word.isEmpty()) {
        char previous = name.charAt(index - 1);
        boolean nextIsLower = index + 1 < name.length() && isAsciiLowercase(name.charAt(index + 1));
        boolean boundary =
            isAsciiLowercase(previous)
                || isAsciiDigit(previous)
                || (isAsciiUppercase(previous) && nextIsLower);
        if (boundary) {
          words.add(word.toString());
          word.setLength(0);
        }
      }
      word.append(Character.toLowerCase(character));
    }
    if (!word.isEmpty()) {
      words.add(word.toString());
    }
    var text = new StringBuilder();
    for (int index = 0; index < words.size(); index++) {
      String next = words.get(index);
      text.append(
          index == 0 ? next : next.substring(0, 1).toUpperCase(Locale.ROOT) + next.substring(1));
    }
    if (text.isEmpty()) {
      return fallback;
    }
    return isAsciiDigit(text.charAt(0)) ? fallback + text : text.toString();
  }

  private static boolean isAsciiAlphanumeric(char character) {
    return isAsciiLowercase(character) || isAsciiUppercase(character) || isAsciiDigit(character);
  }

  private static boolean isAsciiLowercase(char character) {
    return character >= 'a' && character <= 'z';
  }

  private static boolean isAsciiUppercase(char character) {
    return character >= 'A' && character <= 'Z';
  }

  private static boolean isAsciiDigit(char character) {
    return character >= '0' && character <= '9';
  }
}
