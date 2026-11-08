package dev.gecode.iris.processor;

import static dev.gecode.iris.processor.Layout.MEMBER_COLUMN;
import static dev.gecode.iris.processor.Layout.at;

import dev.gecode.iris.processor.Layout.Call;
import dev.gecode.iris.processor.Layout.Component;
import dev.gecode.iris.processor.Layout.Expression;
import dev.gecode.iris.processor.Layout.Select;
import dev.gecode.iris.processor.Layout.Text;
import java.util.ArrayList;
import java.util.List;
import java.util.SequencedSet;
import java.util.TreeSet;
import org.jspecify.annotations.Nullable;

/**
 * Writes the classes that {@link IrisSpec} generates for one annotated type: the handles class and
 * the record, laid out as google-java-format lays them out, through {@link Layout}.
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

  /** The method that joins the literals of the spec's text. */
  private static final String JOIN = "java.lang.String.join";

  /**
   * The most JVM parameter slots that the components of a record may take: a method takes at most
   * 255 slots, and a constructor spends one on the object it constructs. {@link
   * dev.gecode.iris.RecordReader} constructs a record of so many through reflection.
   */
  static final int MOST_RECORD_SLOTS = 254;

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
    List<String> out = header();
    if (!handles.isEmpty()) {
      out.add("import static dev.gecode.iris.build.Iris.*;");
      out.add("");
    }
    imports.forEach(name -> out.add("import " + name + ";"));
    out.add("");
    out.addAll(
        Layout.javadoc(
            0,
            "The reader spec {@code "
                + javadocText(path)
                + "} and a typed handle for each of its entries, generated for {@link "
                + simpleName(annotated)
                + "}."));
    out.addAll(Layout.modified(0, visibility() + "final ", "class " + handlesClass + " {"));
    out.addAll(
        Layout.javadoc(
            MEMBER_COLUMN,
            "The reader spec, parsed from the text that the file had at compile time."));
    List<String> literals = Literals.lines(text);
    Expression specText;
    if (literals.size() <= LITERALS_PER_METHOD) {
      specText = joined(literals);
    } else {
      List<Expression> parts = new ArrayList<>();
      parts.add(new Text("\"\""));
      for (int part = 0; part * LITERALS_PER_METHOD < literals.size(); part++) {
        parts.add(new Text("text" + part + "()"));
      }
      specText = new Call(JOIN, parts);
    }
    out.addAll(
        Layout.field(
            "public static final ",
            "Spec",
            "spec",
            new Call("Spec.parse", List.of(specText, new Text(Literals.quoted(path))))));
    for (Handle handle : handles) {
      out.add("");
      out.addAll(
          Layout.javadoc(MEMBER_COLUMN, "The entry {@code " + javadocText(handle.entry()) + "}."));
      out.addAll(
          Layout.handle(
              handle.handleType(),
              handle.identifier(),
              handle.factory(),
              handle.arguments(),
              handle.optional()));
    }
    if (recordClass != null) {
      out.add("");
      out.addAll(Layout.javadoc(MEMBER_COLUMN, "Reads data into {@link " + recordClass + "}."));
      out.addAll(
          Layout.field(
              "public static final ",
              "RecordReader<" + recordClass + ">",
              "reader",
              new Call(
                  "spec.reader",
                  List.of(new Select(recordClass, "class"), new Text("MethodHandles.lookup()")))));
    }
    out.add("");
    out.addAll(Layout.modified(MEMBER_COLUMN, "private ", handlesClass + "() {}"));
    if (literals.size() > LITERALS_PER_METHOD) {
      for (int part = 0; part * LITERALS_PER_METHOD < literals.size(); part++) {
        int from = part * LITERALS_PER_METHOD;
        int to = Math.min(from + LITERALS_PER_METHOD, literals.size());
        out.add("");
        out.add(at(MEMBER_COLUMN, "private static java.lang.String text" + part + "() {"));
        out.addAll(
            Layout.expression(
                "return ", joined(literals.subList(from, to)), ";", 2 * MEMBER_COLUMN));
        out.add(at(MEMBER_COLUMN, "}"));
      }
    }
    out.add("}");
    return lines(out);
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
    List<String> out = header();
    imports.forEach(name -> out.add("import " + name + ";"));
    if (!imports.isEmpty()) {
      out.add("");
    }
    out.addAll(
        Layout.javadoc(
            0,
            "The entries of the reader spec {@code "
                + javadocText(path)
                + "}, as {@link "
                + handlesClass
                + "#reader} reads them."));
    out.add("@java.lang.SuppressWarnings(\"ArrayRecordComponent\")");
    List<Component> components = new ArrayList<>();
    for (Handle handle : handles) {
      components.add(
          new Component(handle.entryAnnotation(), handle.componentType(), handle.identifier()));
    }
    out.addAll(Layout.recordHeader(visibility(), recordClass, components));
    return lines(out);
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

  /** {@return the comment that names the generator, and the package} */
  private List<String> header() {
    List<String> out =
        new ArrayList<>(
            Layout.lineComment(
                "// Generated by iris-processor for "
                    + annotated
                    + " from "
                    + Literals.quoted(path)
                    + "."));
    if (!packageName.isEmpty()) {
      out.add("package " + packageName + ";");
    }
    out.add("");
    return out;
  }

  private String visibility() {
    return publicClasses ? "public " : "";
  }

  /** {@return {@code literals} as one expression: a literal alone, or joined} */
  private static Expression joined(List<String> literals) {
    if (literals.isEmpty()) {
      return new Text("\"\"");
    }
    if (literals.size() == 1) {
      return new Text(literals.getFirst());
    }
    List<Expression> arguments = new ArrayList<>();
    arguments.add(new Text("\"\""));
    literals.forEach(literal -> arguments.add(new Text(literal)));
    return new Call(JOIN, arguments);
  }

  /** {@return {@code lines}, each ended by a line break} */
  private static String lines(List<String> lines) {
    var out = new StringBuilder();
    lines.forEach(line -> out.append(line).append('\n'));
    return out.toString();
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
