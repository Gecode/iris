package dev.gecode.iris.internal.syntax;

import static java.nio.charset.StandardCharsets.UTF_8;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.gecode.iris.IrisException;
import dev.gecode.iris.internal.syntax.OutputEntry.Shape;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.stream.Stream;
import org.junit.jupiter.api.Test;

/**
 * Compares the output entries of every reader spec in the conformance cases and the format library
 * with the typed handles that {@code iris gen --lang rust} prints from the Rust semantic model.
 *
 * <p>{@code -PirisCli=PATH} names an {@code iris} binary to run instead of the one Cargo builds.
 */
class OutputEntriesDifferentialTest {
  /** The deepest entry that Rust gives a typed handle; deeper ones are dynamic. */
  private static final int DEEPEST_TYPED = 8;

  private static final Pattern DECLARATION =
      Pattern.compile(
          "pub const [^:]+: (DynField|Field<([^>]*)>) =\\s*Field::declared\\(\\s*"
              + "\"((?:[^\"\\\\]|\\\\.)*)\",?\\s*\\);");

  private static final Pattern ESCAPE = Pattern.compile("\\\\(u\\{([0-9a-f]+)\\}|.)");

  private static final Pattern EXECUTABLE = Pattern.compile("\"executable\":\"([^\"]+)\"");

  @Test
  void javaAndRustGiveTheSameHandles() throws IOException, InterruptedException {
    Path repository = Path.of(System.getProperty("iris.repository"));
    String cli = cli(repository);
    List<String> differences = new ArrayList<>();
    int compared = 0;
    int rejected = 0;
    for (Path spec : specs(repository)) {
      String java = javaHandles(Files.readAllBytes(spec));
      Process rust = new ProcessBuilder(cli, "gen", "--lang", "rust", spec.toString()).start();
      String printed = new String(rust.getInputStream().readAllBytes(), UTF_8);
      rust.getErrorStream().readAllBytes();
      boolean accepted = rust.waitFor() == 0;
      String expected = accepted ? rustHandles(printed) : null;
      if (java == null && expected == null) {
        rejected++;
      } else if (java == null || !java.equals(expected)) {
        differences.add(spec + "\n  Java: " + java + "\n  Rust: " + expected);
      } else {
        compared++;
      }
    }
    System.out.printf("Compared %d specs, %d rejected by both%n", compared, rejected);
    assertEquals(List.of(), differences);
    assertTrue(compared > 700, "only " + compared + " specs compared");
  }

  private static List<Path> specs(Path repository) throws IOException {
    List<Path> specs = new ArrayList<>();
    for (String root : List.of("conformance/cases/0.1", "formats")) {
      try (Stream<Path> files = Files.walk(repository.resolve(root))) {
        files.filter(file -> file.toString().endsWith(".iris")).sorted().forEach(specs::add);
      }
    }
    return specs;
  }

  /** Returns the handles of a spec, one per line as Rust names their types, or null if invalid. */
  private static String javaHandles(byte[] source) {
    Ast.Program program;
    Resolution resolution;
    try {
      program = LargeStack.call(() -> Parser.parse(Lexer.lex(source, "spec"), "spec"));
      resolution = LargeStack.call(() -> Resolver.resolve(program, "spec"));
    } catch (IrisException e) {
      return null;
    }
    var handles = new StringBuilder();
    for (OutputEntry entry : LargeStack.call(() -> OutputEntries.of(program, resolution))) {
      handles.append(entry.name()).append(": ").append(rustType(entry)).append('\n');
    }
    return handles.toString();
  }

  private static String rustType(OutputEntry entry) {
    if (!(entry.shape() instanceof Shape.Typed typed) || typed.depth() > DEEPEST_TYPED) {
      return "DynField";
    }
    String value =
        switch (typed.leaf()) {
          case INT -> "Int";
          case FLOAT -> "Float";
          case STRING -> "Str";
          case NUM -> "Num";
        };
    String depth = "D" + typed.depth();
    if (entry.possiblyAbsent()) {
      return "Field<" + value + ", " + depth + ", Maybe>";
    }
    return typed.depth() == 0 ? "Field<" + value + ">" : "Field<" + value + ", " + depth + ">";
  }

  /** Returns the handles that {@code iris gen --lang rust} printed, in the form of Java's. */
  private static String rustHandles(String printed) {
    var handles = new StringBuilder();
    Matcher declaration = DECLARATION.matcher(printed);
    while (declaration.find()) {
      handles.append(unescape(declaration.group(3))).append(": ");
      handles.append(declaration.group(1).replaceAll("\\s+", " ")).append('\n');
    }
    return handles.toString();
  }

  private static String unescape(String literal) {
    return ESCAPE
        .matcher(literal)
        .replaceAll(escape ->
            Matcher.quoteReplacement(
                escape.group(2) != null
                    ? Character.toString(Integer.parseInt(escape.group(2), 16))
                    : escape.group(1)));
  }

  /** Returns the {@code iris} binary of {@code -PirisCli}, or builds one with Cargo. */
  private static String cli(Path repository) throws IOException, InterruptedException {
    String cli = System.getProperty("iris.cli");
    if (cli != null) {
      return cli;
    }
    var builder =
        new ProcessBuilder(
            "cargo",
            "build",
            "--quiet",
            "--locked",
            "-p",
            "gecode-iris-cli",
            "--bin",
            "iris",
            "--message-format=json");
    builder.directory(repository.resolve("rust").toFile());
    builder.redirectError(ProcessBuilder.Redirect.INHERIT);
    Process cargo = builder.start();
    String messages = new String(cargo.getInputStream().readAllBytes(), UTF_8);
    assertEquals(0, cargo.waitFor(), "cargo build failed");
    String executable = null;
    Matcher artifact = EXECUTABLE.matcher(messages);
    while (artifact.find()) {
      executable = artifact.group(1);
    }
    assertTrue(executable != null, "cargo built no executable");
    return executable;
  }
}
