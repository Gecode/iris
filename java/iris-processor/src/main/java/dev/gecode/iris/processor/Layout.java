package dev.gecode.iris.processor;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import org.jspecify.annotations.Nullable;

/**
 * The layouts that google-java-format gives the generated classes, so that it leaves them as they
 * are, ported from {@code rust/iris-gen/src/java} where they are the same:
 *
 * <ul>
 *   <li>A line comment wider than a line breaks at the last space that leaves it narrow enough.
 *   <li>A Javadoc comment is on one line if it fits, and else fills lines between {@code /**} and
 *       {@code *}{@code /}, with every run of spaces one space.
 *   <li>A call is on one line if it fits. Otherwise it breaks after the opening parenthesis and
 *       puts the arguments on one line, four columns in, or, if that is too wide, fills lines with
 *       them when every one is at most {@link #SHORT_ARGUMENT} columns wide and else puts each on a
 *       line of its own, laid out in turn.
 *   <li>An assignment breaks after {@code =} before it breaks the value, unless its declaration
 *       broke before the name and the name, {@code =} and value fit on a line together.
 *   <li>A record's components follow the rules of arguments, without filling lines.
 *   <li>A declaration, a type and a qualified name break as {@link Doc} describes.
 *   <li>A member of a name breaks before its dot.
 *   <li>The header of a class, record or constructor breaks after its modifiers if it is too wide
 *       up to its first break.
 * </ul>
 *
 * <p>A literal or name too wide for its line stays whole, as google-java-format leaves it.
 */
final class Layout {
  /** The widest line that google-java-format leaves whole. */
  static final int LINE_WIDTH = 100;

  /** The indent of a continuation line. */
  static final int CONTINUATION = 4;

  /** The column of a class's members. */
  static final int MEMBER_COLUMN = 2;

  /** The widest argument that google-java-format fills lines with. */
  private static final int SHORT_ARGUMENT = 9;

  private Layout() {}

  /** An expression of generated code: text that is never broken, or a call. */
  sealed interface Expression {
    /** {@return the expression on one line} */
    String flat();
  }

  /**
   * Text that is never broken, such as a literal.
   *
   * @param flat the text
   */
  record Text(String flat) implements Expression {}

  /**
   * A member of a name, such as {@code Data.class}, which breaks before its dot, a continuation in.
   *
   * @param target the name, which never breaks
   * @param member the member
   */
  record Select(String target, String member) implements Expression {
    @Override
    public String flat() {
      return target + "." + member;
    }
  }

  /**
   * A call.
   *
   * @param callee what is called, such as {@code Spec.parse}, which never breaks
   * @param arguments the arguments
   */
  record Call(String callee, List<Expression> arguments) implements Expression {
    @Override
    public String flat() {
      return callee
          + "("
          + String.join(", ", arguments.stream().map(Expression::flat).toList())
          + ")";
    }
  }

  /** {@return {@code text} at {@code column}} */
  static String at(int column, String text) {
    return " ".repeat(column) + text;
  }

  private static boolean fits(int column, String text) {
    return column + text.length() <= LINE_WIDTH;
  }

  /**
   * {@return the lines of {@code modifiers rest} at {@code column}, where {@code modifiers} is
   * empty or ends with a space: on one line if it fits, else broken after the modifiers}
   */
  static List<String> modified(int column, String modifiers, String rest) {
    if (modifiers.isEmpty() || fits(column, modifiers + rest)) {
      return List.of(at(column, modifiers + rest));
    }
    return List.of(at(column, modifiers.strip()), at(column, rest));
  }

  /** {@return the lines of the line comment {@code text}, which starts with {@code //}} */
  static List<String> lineComment(String text) {
    List<String> lines = new ArrayList<>();
    String rest = text;
    while (rest.length() > LINE_WIDTH) {
      int space = LINE_WIDTH;
      while (space >= 2 && rest.charAt(space) != ' ') {
        space--;
      }
      if (space <= 2) {
        break;
      }
      // A break inside a run of spaces leaves some at the end of the line, which
      // google-java-format removes from its output
      lines.add(rest.substring(0, space).stripTrailing());
      rest = "//" + rest.substring(space);
    }
    lines.add(rest.stripTrailing());
    return lines;
  }

  /** {@return the lines of a Javadoc comment of {@code text} at {@code column}} */
  static List<String> javadoc(int column, String text) {
    List<String> words = Arrays.stream(text.strip().split(" +")).toList();
    String oneLine = "/** " + String.join(" ", words) + " */";
    if (fits(column, oneLine)) {
      return List.of(at(column, oneLine));
    }
    List<String> lines = new ArrayList<>();
    lines.add(at(column, "/**"));
    String prefix = at(column, " * ");
    var line = new StringBuilder();
    for (String word : words) {
      if (line.isEmpty()) {
        line.append(word);
      } else if (prefix.length() + line.length() + 1 + word.length() <= LINE_WIDTH) {
        line.append(' ').append(word);
      } else {
        lines.add(prefix + line);
        line.setLength(0);
        line.append(word);
      }
    }
    lines.add(prefix + line);
    lines.add(at(column, " */"));
    return lines;
  }

  /**
   * {@return the lines of the field {@code modifiers type name = value;} at {@link #MEMBER_COLUMN},
   * where {@code modifiers} ends with a space}
   */
  static List<String> field(String modifiers, String type, String name, Expression value) {
    String oneLine = modifiers + type + " " + name + " = " + value.flat() + ";";
    if (fits(MEMBER_COLUMN, oneLine)) {
      return List.of(at(MEMBER_COLUMN, oneLine));
    }
    List<String> lines = declaration(modifiers, type, name);
    if (lines.size() > 1 && fits(0, lines.getLast() + " = " + value.flat() + ";")) {
      lines.set(lines.size() - 1, lines.getLast() + " = " + value.flat() + ";");
      return lines;
    }
    lines.set(lines.size() - 1, lines.getLast() + " =");
    lines.addAll(expression("", value, ";", valueColumn(lines)));
    return lines;
  }

  /**
   * {@return the lines of the field declaration {@code modifiers type name} at {@link
   * #MEMBER_COLUMN}, before an {@code =} that ends its last line}
   */
  private static List<String> declaration(String modifiers, String type, String name) {
    Doc declaration =
        new Doc.Group(
            CONTINUATION,
            List.of(
                new Doc.Token(modifiers), Doc.type(type), new Doc.Break(" "), new Doc.Token(name)));
    return declaration.lines(MEMBER_COLUMN, " =".length());
  }

  /**
   * {@return the column of the value of a field whose declaration is {@code lines}: a continuation
   * in, and one more if the declaration broke before its name}
   */
  private static int valueColumn(List<String> lines) {
    return MEMBER_COLUMN + (lines.size() > 1 ? 2 : 1) * CONTINUATION;
  }

  /**
   * {@return the lines of the handle declaration {@code public static final type name =
   * factory(arguments);} at {@link #MEMBER_COLUMN}, with {@code .optional()} after the call if
   * {@code optional}, as {@code iris gen --lang java} lays it out: on one line, broken after {@code
   * =}, then with the call and {@code .optional()} apart, then with the arguments on a line of
   * their own or each on one}
   */
  static List<String> handle(
      String type, String name, String factory, List<String> arguments, boolean optional) {
    String modifiers = "public static final ";
    String call = factory + "(" + String.join(", ", arguments) + ")";
    String tail = optional ? ".optional()" : "";
    String oneLine = modifiers + type + " " + name + " = " + call + tail + ";";
    if (fits(MEMBER_COLUMN, oneLine)) {
      return List.of(at(MEMBER_COLUMN, oneLine));
    }
    List<String> lines = declaration(modifiers, type, name);
    if (lines.size() > 1 && fits(0, lines.getLast() + " = " + call + tail + ";")) {
      lines.set(lines.size() - 1, lines.getLast() + " = " + call + tail + ";");
      return lines;
    }
    int valueColumn = valueColumn(lines);
    lines.set(lines.size() - 1, lines.getLast() + " =");
    if (fits(valueColumn, call + tail + ";")) {
      lines.add(at(valueColumn, call + tail + ";"));
      return lines;
    }
    if (!optional) {
      lines.addAll(brokenCall(factory, arguments, ");", valueColumn, valueColumn + CONTINUATION));
      return lines;
    }
    // A name no longer than the indent before a chain's arguments gains nothing from a break after
    // it, so google-java-format keeps a single argument beside it.
    boolean shortName = arguments.size() == 1 && factory.length() <= CONTINUATION;
    if (shortName || fits(valueColumn, call)) {
      lines.add(at(valueColumn, call));
    } else {
      // In a chain, the arguments go two levels in.
      lines.addAll(
          brokenCall(factory, arguments, ")", valueColumn, valueColumn + 2 * CONTINUATION));
    }
    lines.add(at(valueColumn + CONTINUATION, ".optional();"));
    return lines;
  }

  /**
   * {@return {@code factory(} at {@code column} and its arguments at {@code inner}, on one line if
   * they fit and else one on each line, followed by {@code close}}
   */
  private static List<String> brokenCall(
      String factory, List<String> arguments, String close, int column, int inner) {
    List<String> lines = new ArrayList<>();
    lines.add(at(column, factory + "("));
    String together = String.join(", ", arguments) + close;
    if (fits(inner, together)) {
      lines.add(at(inner, together));
      return lines;
    }
    for (int index = 0; index < arguments.size(); index++) {
      lines.add(at(inner, arguments.get(index) + (index + 1 == arguments.size() ? close : ",")));
    }
    return lines;
  }

  /** {@return the lines of {@code first value tail} at {@code column}} */
  static List<String> expression(String first, Expression value, String tail, int column) {
    String oneLine = first + value.flat() + tail;
    if (fits(column, oneLine)) {
      return List.of(at(column, oneLine));
    }
    if (value instanceof Select select) {
      return List.of(
          at(column, first + select.target()),
          at(column + CONTINUATION, "." + select.member() + tail));
    }
    if (!(value instanceof Call call) || call.arguments().isEmpty()) {
      return List.of(at(column, oneLine));
    }
    List<String> lines = new ArrayList<>();
    lines.add(at(column, first + call.callee() + "("));
    lines.addAll(arguments(call.arguments(), ")" + tail, column + CONTINUATION));
    return lines;
  }

  /**
   * {@return the lines of {@code arguments} at {@code column}, followed by {@code close}: on one
   * line if they fit, else filled if every one is short, else one on each line}
   */
  private static List<String> arguments(List<Expression> arguments, String close, int column) {
    List<String> texts = arguments.stream().map(Expression::flat).toList();
    String together = String.join(", ", texts) + close;
    if (fits(column, together)) {
      return List.of(at(column, together));
    }
    if (texts.stream().allMatch(text -> text.length() <= SHORT_ARGUMENT)) {
      return filled(texts, close, column);
    }
    List<String> lines = new ArrayList<>();
    for (int index = 0; index < arguments.size(); index++) {
      String end = index + 1 == arguments.size() ? close : ",";
      lines.addAll(expression("", arguments.get(index), end, column));
    }
    return lines;
  }

  /** {@return {@code texts} separated by {@code ", "} and packed greedily at {@code column}} */
  private static List<String> filled(List<String> texts, String close, int column) {
    List<String> lines = new ArrayList<>();
    var line = new StringBuilder();
    for (int index = 0; index < texts.size(); index++) {
      String own = texts.get(index) + (index + 1 == texts.size() ? close : ",");
      if (line.isEmpty()) {
        line.append(own);
      } else if (column + line.length() + 1 + own.length() <= LINE_WIDTH) {
        line.append(' ').append(own);
      } else {
        lines.add(at(column, line.toString()));
        line.setLength(0);
        line.append(own);
      }
    }
    lines.add(at(column, line.toString()));
    return lines;
  }

  /**
   * A record component.
   *
   * @param annotation the component's declaration annotation, such as {@code @Entry("n")}, or null
   *     if it has none
   * @param type the component's type
   * @param name the component's name
   */
  record Component(@Nullable String annotation, String type, String name) {
    String flat() {
      return doc().flat();
    }

    /**
     * {@return the component as google-java-format sees it: the type and name, which break before
     * the name, after the annotation, which breaks before them and after its opening parenthesis}
     */
    Doc doc() {
      Doc declaration =
          new Doc.Group(
              CONTINUATION, List.of(Doc.type(type), new Doc.Break(" "), new Doc.Token(name)));
      if (annotation == null) {
        return declaration;
      }
      int open = annotation.indexOf('(');
      Doc annotationDoc =
          open < 0
              ? new Doc.Token(annotation)
              : new Doc.Group(
                  CONTINUATION,
                  List.of(
                      new Doc.Token(annotation.substring(0, open + 1)),
                      new Doc.Break(""),
                      new Doc.Token(annotation.substring(open + 1))));
      return new Doc.Group(CONTINUATION, List.of(annotationDoc, new Doc.Break(" "), declaration));
    }
  }

  /**
   * {@return the lines of the record header {@code modifiers record name(components) {}} at column
   * 0, where {@code modifiers} is empty or ends with a space: broken after the modifiers if the
   * header up to its {@code (} is too wide for a line, then on one line, else with the components
   * on the next line, else each on a line of its own, laid out as {@link Component#doc} describes}
   */
  static List<String> recordHeader(String modifiers, String name, List<Component> components) {
    String flat = String.join(", ", components.stream().map(Component::flat).toList());
    String close = ") {}";
    List<String> lines =
        new ArrayList<>(
            modified(0, modifiers, "record " + name + "(" + (components.isEmpty() ? close : "")));
    String opening = lines.removeLast();
    if (components.isEmpty()) {
      lines.add(opening);
      return lines;
    }
    if (fits(0, opening + flat + close)) {
      lines.add(opening + flat + close);
      return lines;
    }
    lines.add(opening);
    if (fits(CONTINUATION, flat + close)) {
      lines.add(at(CONTINUATION, flat + close));
      return lines;
    }
    for (int index = 0; index < components.size(); index++) {
      String end = index + 1 == components.size() ? close : ",";
      List<String> component = components.get(index).doc().lines(CONTINUATION, end.length());
      component.set(component.size() - 1, component.getLast() + end);
      lines.addAll(component);
    }
    return lines;
  }

  /**
   * A part of a declaration as google-java-format's documents model it, laid out at a line width: a
   * token, which never breaks, a break, or a group of them, which is on one line if it fits there
   * together with the text that follows it up to the next break, and else breaks at every break of
   * its own, each at the group's indent. A group's indent is that of the group around it, however
   * that is laid out, plus its own.
   *
   * <p>A type breaks after the {@code @Nullable} before it, after the {@code <} of each of its
   * arguments and before the {@code []} of an array, a continuation in; a qualified name such as
   * {@code java.lang.String} breaks before its last dot, a continuation in, then before the others,
   * at the same indent.
   */
  sealed interface Doc {
    /**
     * A token.
     *
     * @param text its text
     */
    record Token(String text) implements Doc {}

    /**
     * A break.
     *
     * @param flat the text in its place when its group is on one line
     */
    record Break(String flat) implements Doc {}

    /**
     * A group.
     *
     * @param indent its indent beyond that of the group around it
     * @param docs its parts
     */
    record Group(int indent, List<Doc> docs) implements Doc {}

    /** {@return the document of {@code type}, written as {@link Handle} writes types} */
    static Doc type(String type) {
      String nullable = "@Nullable ";
      if (type.startsWith(nullable)) {
        return new Group(
            0,
            List.of(
                new Token(nullable.strip()),
                new Break(" "),
                type(type.substring(nullable.length()))));
      }
      int open = type.indexOf('<');
      if (open > 0 && type.endsWith(">")) {
        return new Group(
            CONTINUATION,
            List.of(
                new Token(type.substring(0, open + 1)),
                new Break(""),
                type(type.substring(open + 1, type.length() - 1)),
                new Token(">")));
      }
      String array = "[]";
      if (type.endsWith(array) && type.indexOf(' ') < 0) {
        return new Group(
            CONTINUATION,
            List.of(
                new Token(type.substring(0, type.length() - array.length())),
                new Break(""),
                new Token(array)));
      }
      int dot = type.lastIndexOf('.');
      if (dot > 0 && type.chars().noneMatch(c -> c == ' ' || c == '@')) {
        return qualified(type, CONTINUATION);
      }
      return new Token(type);
    }

    private static Doc qualified(String name, int indent) {
      int dot = name.lastIndexOf('.');
      if (dot < 0) {
        return new Token(name);
      }
      return new Group(
          indent,
          List.of(
              qualified(name.substring(0, dot), 0), new Break(""), new Token(name.substring(dot))));
    }

    /** {@return the document on one line} */
    default String flat() {
      return switch (this) {
        case Token token -> token.text();
        case Break space -> space.flat();
        case Group group -> String.join("", group.docs().stream().map(Doc::flat).toList());
      };
    }

    /**
     * {@return the lines of the document at {@code column}, which is also the indent around it,
     * where {@code trailing} columns of text follow it before the next break}
     */
    default List<String> lines(int column, int trailing) {
      List<String> lines = new ArrayList<>();
      var line = new StringBuilder(at(column, ""));
      layOut(lines, line, column, trailing);
      lines.add(line.toString());
      return lines;
    }

    private void layOut(List<String> lines, StringBuilder line, int indent, int trailing) {
      if (!(this instanceof Group group)
          || line.length() + flat().length() + trailing <= LINE_WIDTH) {
        line.append(flat());
        return;
      }
      int inner = indent + group.indent();
      List<Doc> docs = group.docs();
      for (int index = 0; index < docs.size(); index++) {
        if (docs.get(index) instanceof Break) {
          lines.add(line.toString());
          line.setLength(0);
          line.append(at(inner, ""));
          continue;
        }
        // The text after this part up to the next break, or past the group's end.
        int after = 0;
        int next = index + 1;
        while (next < docs.size() && !(docs.get(next) instanceof Break)) {
          after += docs.get(next).flat().length();
          next++;
        }
        docs.get(index).layOut(lines, line, inner, next == docs.size() ? after + trailing : after);
      }
    }
  }
}
