package dev.gecode.iris.processor;

import java.util.List;
import java.util.SequencedSet;
import java.util.TreeSet;
import org.jspecify.annotations.Nullable;

/**
 * Writes the classes that {@link IrisSpec} generates for one annotated type: the handles class and
 * the record, as google-java-format would lay out their declarations where they fit on a line.
 *
 * <p>The classes name the types of {@code java.lang} by their qualified names, so that a class of
 * the same simple name in the annotated type's package cannot take their place. No handle is named
 * {@code java}, which would hide the package in the expressions that build the spec's text.
 *
 * @param packageName the package of the annotated type, empty for the unnamed package
 * @param handlesClass the simple name of the handles class
 * @param recordClass the simple name of the record, or null if the entries do not fit one
 * @param publicClasses whether the annotated type, and so the generated classes, are public
 * @param path the path of the reader spec as the annotation gives it, its origin in messages
 * @param annotated the qualified name of the annotated type
 * @param text the text of the reader spec
 * @param handles the handles of its entries
 */
record Generator(
    String packageName,
    String handlesClass,
    @Nullable String recordClass,
    boolean publicClasses,
    String path,
    String annotated,
    String text,
    List<Handle> handles) {
  /** The most literals in one call of {@code String.join}, which keeps a method's code small. */
  private static final int LITERALS_PER_METHOD = 1000;

  /**
   * The most JVM parameter slots that the components of a record may take for {@link
   * dev.gecode.iris.RecordReader} to construct it. A method takes at most 255 slots; a constructor
   * spends one on the object it constructs, and the method handle that the reader invokes it
   * through spends one on itself, as {@link java.lang.invoke.MethodHandle} documents.
   */
  static final int MOST_RECORD_SLOTS = 253;

  /** {@return the source of the handles class} */
  String handlesSource() {
    SequencedSet<String> imports = new TreeSet<>();
    imports.add("dev.gecode.iris.Spec");
    for (Handle handle : handles) {
      handle.imports().stream()
          .filter(name -> names(handle.handleType(), name))
          .forEach(imports::add);
    }
    if (recordClass != null) {
      imports.add("dev.gecode.iris.RecordReader");
      imports.add("java.lang.invoke.MethodHandles");
    }
    var out = header();
    if (!handles.isEmpty()) {
      out.append("import static dev.gecode.iris.build.Iris.*;\n\n");
    }
    imports.forEach(name -> out.append("import ").append(name).append(";\n"));
    out.append("\n/**\n * The reader spec {@code ")
        .append(javadocText(path))
        .append("} and a typed handle for each of its\n * entries, generated for {@link ")
        .append(simpleName(annotated))
        .append("}.\n */\n");
    out.append(visibility()).append("final class ").append(handlesClass).append(" {\n");
    out.append(
        "  /** The reader spec, parsed from the text that the file had at compile time. */\n");
    List<String> literals = Literals.lines(text);
    out.append("  public static final Spec spec =\n      Spec.parse(");
    if (literals.size() <= LITERALS_PER_METHOD) {
      join(out, literals, "          ");
    } else {
      out.append("java.lang.String.join(\"\"");
      for (int part = 0; part * LITERALS_PER_METHOD < literals.size(); part++) {
        out.append(", text").append(part).append("()");
      }
      out.append(")");
    }
    out.append(", ").append(Literals.quoted(path)).append(");\n");
    for (Handle handle : handles) {
      out.append("\n  /** The entry {@code ").append(javadocText(handle.entry())).append("}. */\n");
      out.append("  public static final ")
          .append(handle.handleType())
          .append(' ')
          .append(handle.identifier())
          .append(" = ")
          .append(handle.call())
          .append(";\n");
    }
    if (recordClass != null) {
      out.append("\n  /** Reads data into {@link ").append(recordClass).append("}. */\n");
      out.append("  public static final RecordReader<")
          .append(recordClass)
          .append("> reader =\n      spec.reader(")
          .append(recordClass)
          .append(".class, MethodHandles.lookup());\n");
    }
    out.append("\n  private ").append(handlesClass).append("() {}\n");
    if (literals.size() > LITERALS_PER_METHOD) {
      for (int part = 0; part * LITERALS_PER_METHOD < literals.size(); part++) {
        int from = part * LITERALS_PER_METHOD;
        int to = Math.min(from + LITERALS_PER_METHOD, literals.size());
        out.append("\n  private static java.lang.String text")
            .append(part)
            .append("() {\n    return ");
        join(out, literals.subList(from, to), "        ");
        out.append(";\n  }\n");
      }
    }
    return out.append("}\n").toString();
  }

  /** {@return the source of the record} */
  String recordSource() {
    if (recordClass == null) {
      throw new IllegalStateException("the entries do not fit a record");
    }
    SequencedSet<String> imports = new TreeSet<>();
    for (Handle handle : handles) {
      handle.imports().stream()
          .filter(name -> names(handle.componentType(), name))
          .forEach(imports::add);
      if (handle.entryAnnotation() != null) {
        imports.add("dev.gecode.iris.Entry");
      }
    }
    var out = header();
    imports.forEach(name -> out.append("import ").append(name).append(";\n"));
    if (!imports.isEmpty()) {
      out.append('\n');
    }
    out.append("/**\n * The entries of the reader spec {@code ")
        .append(javadocText(path))
        .append("}, as {@link ")
        .append(handlesClass)
        .append("#reader}\n * reads them.\n */\n");
    out.append("@java.lang.SuppressWarnings(\"ArrayRecordComponent\")\n");
    out.append(visibility()).append("record ").append(recordClass).append('(');
    for (int index = 0; index < handles.size(); index++) {
      Handle handle = handles.get(index);
      out.append(index == 0 ? "\n    " : ",\n    ");
      String annotation = handle.entryAnnotation();
      if (annotation != null) {
        out.append(annotation).append(' ');
      }
      out.append(handle.componentType()).append(' ').append(handle.identifier());
    }
    return out.append(") {}\n").toString();
  }

  /** {@return the JVM parameter slots that a record of the entries takes} */
  static int slots(List<Handle> handles) {
    return handles.stream().mapToInt(Handle::slots).sum();
  }

  /** {@return whether {@code type} names the class that {@code qualified} imports} */
  private static boolean names(String type, String qualified) {
    return java.util.regex.Pattern.compile("\\b" + simpleName(qualified) + "\\b")
        .matcher(type)
        .find();
  }

  private StringBuilder header() {
    var out = new StringBuilder();
    out.append("// Generated by iris-processor for ")
        .append(annotated)
        .append(" from ")
        .append(Literals.quoted(path))
        .append(".\n");
    if (!packageName.isEmpty()) {
      out.append("package ").append(packageName).append(";\n");
    }
    return out.append('\n');
  }

  private String visibility() {
    return publicClasses ? "public " : "";
  }

  /** Writes {@code literals} as one expression, each literal on a line of its own. */
  private static void join(StringBuilder out, List<String> literals, String indent) {
    if (literals.size() == 1) {
      out.append(literals.getFirst());
      return;
    }
    out.append("java.lang.String.join(\n").append(indent).append("\"\"");
    for (String literal : literals) {
      out.append(",\n").append(indent).append(literal);
    }
    out.append(")");
  }

  /** {@return {@code text} safe in a Javadoc code span and in a comment} */
  private static String javadocText(String text) {
    String quoted = Literals.quoted(text);
    return quoted
        .substring(1, quoted.length() - 1)
        .replace("*/", "*&#47;")
        .replace("@", "&#64;")
        .replace("{", "&#123;")
        .replace("}", "&#125;");
  }

  private static String simpleName(String qualified) {
    return qualified.substring(qualified.lastIndexOf('.') + 1);
  }
}
