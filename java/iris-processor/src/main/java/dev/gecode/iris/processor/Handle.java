package dev.gecode.iris.processor;

import dev.gecode.iris.internal.syntax.OutputEntry;
import dev.gecode.iris.internal.syntax.OutputEntry.Leaf;
import dev.gecode.iris.internal.syntax.OutputEntry.Shape;
import java.util.ArrayList;
import java.util.List;
import org.jspecify.annotations.Nullable;

/**
 * The handle and record component of one entry, typed as {@code iris gen --lang java} types the
 * handle: {@code IntField<java.lang.Long>} with {@code int0} to {@code int3} and their kin up to
 * depth 3, {@code intN} reading a {@code Value} beyond, and {@code DynField} for an entry without
 * one value type and depth. A record component has the handle's view type, with a {@code List} for
 * each level beyond depth 3, a primitive for a required single number, and {@code Value} for an
 * open entry.
 *
 * @param identifier the Java identifier of the handle and the component
 * @param entry the entry's name
 * @param handleType the handle's type, such as {@code IntField<List<long[]>>}
 * @param factory the factory, such as {@code int2}
 * @param arguments the factory's arguments: the entry's name as a literal, and the depth of an
 *     {@code N} factory
 * @param optional whether the entry may be absent, so the handle calls {@code .optional()}
 * @param componentType the record component's type
 * @param slots the JVM parameter slots of the component
 * @param imports the imports of the handle and the component
 */
record Handle(
    String identifier,
    String entry,
    String handleType,
    String factory,
    List<String> arguments,
    boolean optional,
    String componentType,
    int slots,
    List<String> imports) {
  /** The deepest entry with a factory for its view type; deeper ones read a {@code Value}. */
  private static final int DEEPEST_VIEW_TYPE = 3;

  private static final String VALUE = "dev.gecode.iris.Value";
  private static final String LIST = "java.util.List";
  private static final String NULLABLE = "org.jspecify.annotations.Nullable";

  /**
   * The package of the platform types that the generated classes qualify, so that classes of the
   * same simple names in the annotated type's package cannot take their place.
   */
  private static final String JAVA_LANG = "java.lang.";

  /** Returns the handle of {@code entry}, named {@code identifier}. */
  static Handle of(OutputEntry entry, String identifier) {
    String name = Literals.quoted(entry.name());
    boolean optional = entry.possiblyAbsent();
    List<String> imports = new ArrayList<>();
    if (!(entry.shape() instanceof Shape.Typed typed)) {
      imports.add("dev.gecode.iris.build.DynField");
      imports.add(VALUE);
      if (optional) {
        imports.add(NULLABLE);
      }
      return new Handle(
          identifier,
          entry.name(),
          "DynField",
          "dynamic",
          List.of(name),
          false,
          nullable("Value", optional),
          1,
          imports);
    }
    Leaf leaf = typed.leaf();
    int depth = typed.depth();
    String handleClass =
        switch (leaf) {
          case INT -> "IntField";
          case FLOAT -> "FloatField";
          case NUM -> "NumField";
          case STRING -> "StringField";
        };
    imports.add("dev.gecode.iris.build." + handleClass);
    String prefix = leaf == Leaf.STRING ? "string" : leaf.text();
    String viewType;
    String factory;
    List<String> arguments;
    if (depth > DEEPEST_VIEW_TYPE) {
      imports.add(VALUE);
      viewType = nullable("Value", optional);
      factory = prefix + "N";
      arguments = List.of(name, String.valueOf(depth));
    } else {
      viewType = nullable(viewType(leaf, depth), optional);
      factory = prefix + depth;
      arguments = List.of(name);
    }
    String componentType = componentType(leaf, depth, optional);
    if (componentType.contains("List<") || viewType.contains("List<")) {
      imports.add(LIST);
    }
    if (optional) {
      imports.add(NULLABLE);
    }
    boolean wide = componentType.equals("long") || componentType.equals("double");
    return new Handle(
        identifier,
        entry.name(),
        handleClass + "<" + viewType + ">",
        factory,
        arguments,
        optional,
        componentType,
        wide ? 2 : 1,
        imports);
  }

  /** {@return whether the component needs {@code @Entry} to name its entry} */
  boolean renamed() {
    return !identifier.equals(entry);
  }

  /**
   * The type that {@code data.get} returns for {@code leaf} at {@code depth}, as the factories type
   * it: {@code Long}, {@code long[]} and a {@code List} for each further level for integers, and
   * {@code List<String>} and {@code List<Number>} at depth 1, with {@code java.lang} types
   * qualified.
   */
  private static String viewType(Leaf leaf, int depth) {
    String single =
        switch (leaf) {
          case INT -> JAVA_LANG + "Long";
          case FLOAT -> JAVA_LANG + "Double";
          case NUM -> JAVA_LANG + "Number";
          case STRING -> JAVA_LANG + "String";
        };
    if (depth == 0) {
      return single;
    }
    return "List<".repeat(depth - 1) + row(leaf) + ">".repeat(depth - 1);
  }

  private static String row(Leaf leaf) {
    return switch (leaf) {
      case INT -> "long[]";
      case FLOAT -> "double[]";
      case NUM -> "List<" + JAVA_LANG + "Number>";
      case STRING -> "List<" + JAVA_LANG + "String>";
    };
  }

  /**
   * The record component's type: {@link #viewType} at any depth, with a primitive for a required
   * single integer or float.
   */
  private static String componentType(Leaf leaf, int depth, boolean optional) {
    if (depth == 0 && !optional && leaf == Leaf.INT) {
      return "long";
    }
    if (depth == 0 && !optional && leaf == Leaf.FLOAT) {
      return "double";
    }
    return nullable(viewType(leaf, depth), optional);
  }

  /**
   * {@code type} with JSpecify's {@code @Nullable} where it applies: before a class's simple name,
   * after its package, and between the element type and {@code []} of an array.
   */
  private static String nullable(String type, boolean optional) {
    if (!optional) {
      return type;
    }
    if (type.endsWith("[]")) {
      return type.substring(0, type.length() - 2) + " @Nullable []";
    }
    int packageEnd = type.startsWith(JAVA_LANG) ? JAVA_LANG.length() : 0;
    return type.substring(0, packageEnd) + "@Nullable " + type.substring(packageEnd);
  }

  /** Returns the handles of {@code entries}, with identifiers from one {@link Identifiers}. */
  static List<Handle> of(List<OutputEntry> entries) {
    var identifiers = new Identifiers();
    List<Handle> handles = new ArrayList<>(entries.size());
    for (OutputEntry entry : entries) {
      handles.add(of(entry, identifiers.of(entry.name())));
    }
    return handles;
  }

  /** {@return the component's {@code @Entry} annotation and type, or null if none is needed} */
  @Nullable String entryAnnotation() {
    return renamed() ? "@Entry(" + Literals.quoted(entry) + ")" : null;
  }
}
