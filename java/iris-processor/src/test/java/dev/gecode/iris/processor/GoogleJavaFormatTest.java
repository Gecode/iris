package dev.gecode.iris.processor;

import static java.nio.charset.StandardCharsets.UTF_8;
import static java.util.stream.Collectors.joining;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;

import com.google.googlejavaformat.java.Formatter;
import com.google.googlejavaformat.java.FormatterException;
import com.google.googlejavaformat.java.ImportOrderer;
import com.google.googlejavaformat.java.JavaFormatterOptions;
import com.google.googlejavaformat.java.RemoveUnusedImports;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.stream.IntStream;
import org.junit.jupiter.api.AfterAll;
import org.junit.jupiter.api.Test;

/**
 * The classes that the processor writes are as google-java-format lays them out, across the widths
 * where a layout changes: names of entries, types and paths, the number and width of a record's
 * components, and the number and width of the lines of a spec. {@code java/iris-views} checks the
 * classes of every format and of its specs with google-java-format through Spotless.
 */
class GoogleJavaFormatTest {
  private static final Formatter FORMATTER = new Formatter();

  private static final AtomicInteger CHECKED = new AtomicInteger();

  @AfterAll
  static void printCount() {
    System.out.printf("%d generated classes match google-java-format%n", CHECKED.get());
  }

  /**
   * Formats {@code source} as Spotless's google-java-format step does, without reflowing strings.
   */
  private static String formatted(String source) throws FormatterException {
    String imports =
        RemoveUnusedImports.removeUnusedImports(
            ImportOrderer.reorderImports(source, JavaFormatterOptions.Style.GOOGLE));
    return FORMATTER.formatSource(imports);
  }

  /**
   * Fails with the lines around the first difference if google-java-format changes {@code source}.
   */
  private static void assertFormatted(String source) throws FormatterException {
    List<String> expected = formatted(source).lines().toList();
    List<String> actual = source.lines().toList();
    int line = 0;
    while (line < expected.size()
        && line < actual.size()
        && expected.get(line).equals(actual.get(line))) {
      line++;
    }
    if (line < expected.size() || line < actual.size()) {
      int from = Math.max(0, line - 3);
      assertEquals(
          String.join("\n", expected.subList(from, Math.min(expected.size(), line + 4))),
          String.join("\n", actual.subList(from, Math.min(actual.size(), line + 4))),
          "google-java-format changes line " + (line + 1) + " of\n" + source);
    }
  }

  /** The generator of the classes that {@code type} annotated with {@code path} gets. */
  private static Generator generator(String packageName, String type, String path, String text) {
    List<Handle> handles = Handle.of(IrisSpecProcessor.entries(text.getBytes(UTF_8), path));
    boolean fits = Generator.slots(handles) <= Generator.MOST_RECORD_SLOTS;
    return new Generator(
        packageName,
        type + "Handles",
        fits ? type + "Data" : null,
        true,
        path,
        packageName.isEmpty() ? type : packageName + "." + type,
        text,
        handles);
  }

  /** Checks both classes of {@code generator}, and returns it. */
  private Generator check(Generator generator) throws FormatterException {
    assertFormatted(generator.handlesSource());
    CHECKED.incrementAndGet();
    if (generator.recordClass() != null) {
      assertFormatted(generator.recordSource());
      CHECKED.incrementAndGet();
    }
    return generator;
  }

  private Generator check(String text) throws FormatterException {
    return check(generator("formats", "Format", "format.iris", text));
  }

  private static String repeat(int count, String text) {
    return IntStream.range(0, count).mapToObj(index -> text).collect(joining());
  }

  @Test
  void entriesOfEveryKindAndWidth() throws FormatterException {
    var spec = new StringBuilder("c: int\n");
    for (int length = 1; length <= 130; length++) {
      String name = "n".repeat(length);
      spec.append(name).append("_i: int\n");
      spec.append(name).append("_f[1][1][1]: float\n");
      spec.append("if c {\n  ").append(name).append("_s: word\n}\n");
      spec.append("if c {\n  ").append(name).append("_d[1][1][1][1][1][1][1][1]: num\n}\n");
      spec.append(name).append("_x: int\nlet ").append(name).append("_x = \"s\"\n");
    }
    // So many entries take more slots than a record has: the handles class gets a note instead.
    assertNull(check(spec.toString()).recordClass());
  }

  @Test
  void recordsOfEveryWidth() throws FormatterException {
    String[] kinds = {": int", "[1][1]: float", "[1]: word", ": num"};
    for (int count = 1; count <= 4; count++) {
      for (int length = 1; length <= 60; length++) {
        var spec = new StringBuilder("c: int\n");
        for (int index = 0; index < count; index++) {
          String name = "v" + index + "_" + "r".repeat(length);
          spec.append(index % 2 == 0 ? "" : "if c {\n").append(name).append(kinds[index]);
          spec.append(index % 2 == 0 ? "\n" : "\n}\n");
        }
        assertNotNull(check(spec.toString()).recordClass());
      }
    }
  }

  /** The value types of entries, as a spec writes them. */
  private static final List<String> VALUE_TYPES = List.of("int", "float", "num", "word");

  /**
   * The deepest entry that the declarations are checked at. A component's type grows by six columns
   * with each level, and google-java-format breaks it after each {@code <}, four columns further
   * in: from depth 23 on, every {@code <} of a component starts beyond column 100, so every level
   * breaks, and a deeper type adds only more of the same lines.
   */
  private static final int DEEPEST = 32;

  private static String dimensions(int depth) {
    return "[1]".repeat(depth);
  }

  /**
   * {@return {@code entry}, of {@code valueType} at {@code depth}, inside a condition if {@code
   * optional}}
   */
  private static String entry(String entry, String valueType, int depth, boolean optional) {
    String declaration = entry + dimensions(depth) + ": " + valueType + "\n";
    return optional ? "if c {\n  " + declaration + "}\n" : declaration;
  }

  @Test
  void declarationsOfEveryDepthAndNameWidth() throws FormatterException {
    // Every value type, required and optional, with names from one character to the 24 that
    // identifiers are cut to: components, handle fields, and both across the width of a line.
    for (int depth = 0; depth <= DEEPEST; depth++) {
      var spec = new StringBuilder("c: int\n");
      for (int kind = 0; kind < 2 * VALUE_TYPES.size(); kind++) {
        char first = (char) ('p' + kind);
        for (int length = 1; length <= 24; length++) {
          String name = first + "x".repeat(length - 1);
          spec.append(entry(name, VALUE_TYPES.get(kind / 2), depth, kind % 2 == 1));
        }
      }
      assertNotNull(check(spec.toString()).recordClass(), "depth " + depth);
    }
  }

  @Test
  void renamedDeclarationsOfEveryDepth() throws FormatterException {
    // Entries that need @Entry, with entry names up to 120 characters wide, whose identifiers are
    // cut to 24 characters and then take numeric suffixes of up to three digits.
    for (int depth = 0; depth <= DEEPEST; depth++) {
      var spec = new StringBuilder("c: int\n");
      for (int length = 0; length <= 120; length++) {
        int kind = (length + depth) % (2 * VALUE_TYPES.size());
        String name = "r_" + "y".repeat(length);
        spec.append(entry(name, VALUE_TYPES.get(kind / 2), depth, kind % 2 == 1));
      }
      assertNotNull(check(spec.toString()).recordClass(), "depth " + depth);
    }
  }

  @Test
  void aSingleDeepComponent() throws FormatterException {
    // A record of one component, which has the whole line after the record's name to itself.
    for (int depth = 0; depth <= DEEPEST; depth++) {
      for (String valueType : VALUE_TYPES) {
        for (int length : new int[] {1, 12, 24}) {
          String name = "v".repeat(length);
          check(entry(name, valueType, depth, false));
          check("c: int\n" + entry(name, valueType, depth, true));
          check("c: int\n" + entry("v_" + name, valueType, depth, true));
        }
      }
    }
  }

  @Test
  void recordsOfTheMostSlots() throws FormatterException {
    String ints =
        IntStream.range(0, 126).mapToObj(index -> "v" + index + ": int\n").collect(joining());
    assertNotNull(check(ints + "w0: word\n").recordClass());
    assertNotNull(check(ints + "w0: word\nw1: word\n").recordClass());
    assertNull(check(ints + "w0: word\nw1: word\nw2: word\n").recordClass());
  }

  @Test
  void specsOfEveryLineWidthAndCount() throws FormatterException {
    for (int length = 1; length <= 110; length++) {
      check("n".repeat(length) + ": int\n");
      check("a: int\n" + "#".repeat(length) + "\n");
      check(repeat(3, "#\n") + "#".repeat(length) + "\na: int\n");
    }
    // Lines whose literals are short enough to fill lines with.
    for (int count = 0; count <= 40; count++) {
      check(repeat(count, "#\n") + "a:int\n");
      check(repeat(count, "#\n") + "a:int");
    }
    check("");
    check("# only a comment");
  }

  @Test
  void specsOfMoreLinesThanOneMethodHolds() throws FormatterException {
    for (int count : new int[] {999, 1000, 1001, 2001, 2002}) {
      check(repeat(count - 1, "#\n") + "a: int\n");
    }
  }

  @Test
  void pathsAndTypesOfEveryWidth() throws FormatterException {
    for (int length = 1; length <= 150; length++) {
      String path = "p".repeat(length) + ".iris";
      String spaced =
          IntStream.range(0, length).mapToObj(index -> "d" + index).collect(joining(" "));
      // Runs of spaces, where a comment may wrap between two of them
      String doubled = "a  b ".repeat(length / 5 + 1) + "file.iris";
      check(generator("formats", "Format", path, "a: int\n"));
      check(generator("formats", "Format", spaced + "/f.iris", "a: int\n"));
      check(generator("", "Format", spaced, "a: int\n"));
      check(generator("probe", "PathProbe", doubled, "a: word\n"));
    }
    for (int length = 1; length <= 130; length++) {
      String type = "T" + "t".repeat(length - 1);
      check(generator("formats", type, "format.iris", "a: int\n"));
      check(generator("dev.gecode.formats", type, "format.iris", "a_b: int\n"));
    }
    for (String path :
        List.of(
            "a  b/c\td\"e\\f.iris",
            "*/ @see {@code x} <pre>.iris",
            "été/😀.iris",
            "x ".repeat(60) + "y.iris")) {
      check(generator("formats", "Format", path, "a: int\n"));
    }
  }
}
