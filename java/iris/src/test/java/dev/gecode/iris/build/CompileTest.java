package dev.gecode.iris.build;

import static java.nio.charset.StandardCharsets.UTF_8;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.sun.source.tree.CompilationUnitTree;
import com.sun.source.tree.MemberSelectTree;
import com.sun.source.tree.MethodInvocationTree;
import com.sun.source.util.JavacTask;
import com.sun.source.util.TreePathScanner;
import com.sun.source.util.Trees;
import java.io.File;
import java.io.IOException;
import java.net.URI;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import java.util.concurrent.TimeUnit;
import javax.tools.Diagnostic;
import javax.tools.DiagnosticCollector;
import javax.tools.JavaCompiler;
import javax.tools.JavaFileObject;
import javax.tools.SimpleJavaFileObject;
import javax.tools.ToolProvider;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/**
 * Compiles user code against the builder: what {@code get} returns for each handle, the code that
 * javac rejects, and the absence that NullAway reports in JSpecify mode.
 */
class CompileTest {
  private static final String IMPORTS =
      """
      import static dev.gecode.iris.build.Iris.*;

      import dev.gecode.iris.Data;
      import dev.gecode.iris.Value;
      import dev.gecode.iris.build.*;
      import java.util.List;
      import org.jspecify.annotations.NullMarked;
      import org.jspecify.annotations.Nullable;

      """;

  private static final class Source extends SimpleJavaFileObject {
    private final String text;

    Source(String name, String text) {
      super(URI.create("string:///" + name + ".java"), Kind.SOURCE);
      this.text = text;
    }

    @Override
    public CharSequence getCharContent(boolean ignoreEncodingErrors) {
      return text;
    }
  }

  private static JavacTask task(DiagnosticCollector<JavaFileObject> diagnostics, Source source) {
    JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
    List<String> options =
        List.of(
            "-proc:none", "--release", "27", "-classpath", System.getProperty("java.class.path"));
    return (JavacTask) compiler.getTask(null, null, diagnostics, options, null, List.of(source));
  }

  /** The type of each {@code get(...)} call in {@code body}, by the text of its argument. */
  private static Map<String, String> typesOfGet(String body) throws IOException {
    Source source = new Source("Types", IMPORTS + "class Types {\n" + body + "\n}\n");
    DiagnosticCollector<JavaFileObject> diagnostics = new DiagnosticCollector<>();
    JavacTask task = task(diagnostics, source);
    Iterable<? extends CompilationUnitTree> units = task.parse();
    task.analyze();
    assertEquals(List.of(), diagnostics.getDiagnostics());
    Trees trees = Trees.instance(task);
    Map<String, String> types = new LinkedHashMap<>();
    for (CompilationUnitTree unit : units) {
      new TreePathScanner<Void, Void>() {
        @Override
        public Void visitMethodInvocation(MethodInvocationTree call, Void unused) {
          if (call.getMethodSelect() instanceof MemberSelectTree select
              && select.getIdentifier().contentEquals("get")
              && select.getExpression().toString().equals("data")) {
            types.put(
                call.getArguments().getFirst().toString(),
                String.valueOf(trees.getTypeMirror(getCurrentPath())));
          }
          return super.visitMethodInvocation(call, null);
        }
      }.scan(unit, null);
    }
    return types;
  }

  /** The error messages that javac reports for {@code body}, by line of the body. */
  private static Map<Long, String> errors(String body) {
    String prefix = IMPORTS + "class Errors {\n";
    long offset = prefix.lines().count();
    Source source = new Source("Errors", prefix + body + "\n}\n");
    DiagnosticCollector<JavaFileObject> diagnostics = new DiagnosticCollector<>();
    task(diagnostics, source).call();
    Map<Long, String> errors = new TreeMap<>();
    for (Diagnostic<? extends JavaFileObject> diagnostic : diagnostics.getDiagnostics()) {
      if (diagnostic.getKind() == Diagnostic.Kind.ERROR) {
        errors.merge(
            diagnostic.getLineNumber() - offset,
            diagnostic.getMessage(null),
            (a, b) -> a + "\n" + b);
      }
    }
    return errors;
  }

  @Test
  void getReturnsTheViewTypeOfTheHandle() throws IOException {
    Map<String, String> types =
        typesOfGet(
            """
            void m(Data data) {
              data.get(int0("a"));
              data.get(int1("a"));
              data.get(int2("a"));
              data.get(int3("a"));
              data.get(intN("a", 5));
              data.get(float0("a"));
              data.get(float1("a"));
              data.get(float2("a"));
              data.get(num0("a"));
              data.get(num1("a"));
              data.get(num2("a"));
              data.get(string0("a"));
              data.get(string1("a"));
              data.get(string3("a"));
              data.get(string0("a").optional());
              data.get(int1("a").optional());
              data.get(dynamic("a"));
              builder(() -> {
                data.get(forward("i", Int.class));
                data.get(forward("r", Real.class));
                data.get(forward("n", Num.class));
                data.get(forward("s", Str.class));
                data.get(forward("d", Dyn.class));
              });
            }
            """);
    Map<String, String> expected = new LinkedHashMap<>();
    expected.put("int0(\"a\")", "java.lang.Long");
    expected.put("int1(\"a\")", "long[]");
    expected.put("int2(\"a\")", "java.util.List<long[]>");
    expected.put("int3(\"a\")", "java.util.List<java.util.List<long[]>>");
    expected.put("intN(\"a\", 5)", "dev.gecode.iris.Value");
    expected.put("float0(\"a\")", "java.lang.Double");
    expected.put("float1(\"a\")", "double[]");
    expected.put("float2(\"a\")", "java.util.List<double[]>");
    expected.put("num0(\"a\")", "java.lang.Number");
    expected.put("num1(\"a\")", "java.util.List<java.lang.Number>");
    expected.put("num2(\"a\")", "java.util.List<java.util.List<java.lang.Number>>");
    expected.put("string0(\"a\")", "java.lang.String");
    expected.put("string1(\"a\")", "java.util.List<java.lang.String>");
    expected.put(
        "string3(\"a\")", "java.util.List<java.util.List<java.util.List<java.lang.String>>>");
    expected.put("string0(\"a\").optional()", "java.lang.String");
    expected.put("int1(\"a\").optional()", "long[]");
    expected.put("dynamic(\"a\")", "dev.gecode.iris.Value");
    expected.put("forward(\"i\", Int.class)", "java.lang.Long");
    expected.put("forward(\"r\", Real.class)", "java.lang.Double");
    expected.put("forward(\"n\", Num.class)", "java.lang.Number");
    expected.put("forward(\"s\", Str.class)", "java.lang.String");
    expected.put("forward(\"d\", Dyn.class)", "dev.gecode.iris.Value");
    assertEquals(expected, types);
  }

  @Test
  void javacRejectsAWrongTypeOrDepth() {
    Map<Long, String> errors =
        errors(
            """
            void m(Data data) {
              long fine = data.get(int0("a"));
              String type = data.get(int0("a"));
              long[] depth = data.get(int2("a"));
              long scalar = data.get(int1("a"));
              List<long[]> rows = data.get(float2("a"));
              List<String> strings = data.get(string2("a"));
              IntField<Long> kind = forward("x", Str.class);
              var notAKind = forward("x", String.class);
              IntExpr text = lit("a").plus(1);
              IntExpr mixed = lit(1).plus(lit(1.5));
            }
            """);
    assertEquals(
        List.of(3L, 4L, 5L, 6L, 7L, 8L, 9L, 10L, 11L),
        List.copyOf(errors.keySet()),
        errors.toString());
    Map<Long, List<String>> expected = new LinkedHashMap<>();
    expected.put(
        3L, List.of("equality constraints: java.lang.Long", "upper bounds: java.lang.String"));
    expected.put(
        4L, List.of("equality constraints: java.util.List<long[]>", "upper bounds: long[]"));
    expected.put(5L, List.of("equality constraints: long[]", "upper bounds: java.lang.Long"));
    expected.put(
        6L,
        List.of(
            "equality constraints: java.util.List<double[]>",
            "upper bounds: java.util.List<long[]>"));
    expected.put(
        7L,
        List.of(
            "equality constraints: java.util.List<java.util.List<java.lang.String>>",
            "upper bounds: java.util.List<java.lang.String>"));
    expected.put(
        8L, List.of("equality constraints: dev.gecode.iris.build.StringField<java.lang.String>"));
    expected.put(9L, List.of("cannot infer type-variable(s) H"));
    expected.put(10L, List.of("symbol:   method plus(int)"));
    expected.put(
        11L, List.of("no suitable method found for plus(dev.gecode.iris.build.FloatExpr)"));
    expected.forEach((line, parts) -> {
      String message = String.valueOf(errors.get(line));
      parts.forEach(part -> assertTrue(message.contains(part), line + ": " + message));
    });
  }

  /**
   * NullAway in JSpecify mode reports a dereference of an optional entry and of a dynamic one, and
   * nothing for a required entry or a checked optional one.
   */
  @Test
  void nullAwayReportsAnAbsentEntry(@TempDir Path directory)
      throws IOException, InterruptedException {
    String body =
        """
        @NullMarked
        class Optional {
          static final StringField<@Nullable String> name = string0("name").optional();
          static final StringField<String> label = string0("label");
          static final DynField open = dynamic("open");

          int readOptional(Data data) {
            return data.get(name).length();
          }

          int readRequired(Data data) {
            return data.get(label).length();
          }

          int readChecked(Data data) {
            String value = data.get(name);
            return value == null ? 0 : value.length();
          }

          int readDynamic(Data data) {
            return data.get(open).depth();
          }
        }
        """;
    long offset = IMPORTS.lines().count();
    Path file = directory.resolve("Optional.java");
    Files.writeString(file, IMPORTS + body, UTF_8);
    String errorProne = System.getProperty("iris.errorprone");
    assertTrue(errorProne != null && !errorProne.isEmpty(), "iris.errorprone is not set");
    List<String> command = new ArrayList<>();
    command.add(javac());
    for (String pkg :
        List.of("api", "file", "main", "model", "parser", "processing", "tree", "util")) {
      command.add("-J--add-exports=jdk.compiler/com.sun.tools.javac." + pkg + "=ALL-UNNAMED");
    }
    for (String pkg : List.of("code", "comp")) {
      command.add("-J--add-opens=jdk.compiler/com.sun.tools.javac." + pkg + "=ALL-UNNAMED");
    }
    command.addAll(
        List.of(
            "-XDcompilePolicy=simple",
            "--should-stop=ifError=FLOW",
            "-processorpath",
            errorProne,
            "-Xplugin:ErrorProne -XepDisableAllChecks -Xep:NullAway:ERROR"
                + " -XepOpt:NullAway:OnlyNullMarked=true -XepOpt:NullAway:JSpecifyMode=true",
            "-classpath",
            System.getProperty("java.class.path"),
            "-d",
            directory.resolve("classes").toString(),
            file.toString()));
    Process process = new ProcessBuilder(command).redirectErrorStream(true).start();
    String output = new String(process.getInputStream().readAllBytes(), UTF_8);
    assertTrue(process.waitFor(5, TimeUnit.MINUTES));
    assertEquals(1, process.exitValue(), output);
    List<Long> lines = new ArrayList<>();
    String prefix = file + ":";
    for (String line : output.lines().toList()) {
      if (line.startsWith(prefix) && line.contains("error: [NullAway]")) {
        lines.add(
            Long.parseLong(line.substring(prefix.length(), line.indexOf(':', prefix.length())))
                - offset);
      }
    }
    assertEquals(List.of(8L, 21L), lines, output);
  }

  private static String javac() {
    String name = File.separatorChar == '\\' ? "javac.exe" : "javac";
    return Path.of(System.getProperty("java.home"), "bin", name).toString();
  }
}
