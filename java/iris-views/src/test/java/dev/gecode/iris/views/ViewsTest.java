package dev.gecode.iris.views;

import static java.nio.charset.StandardCharsets.UTF_8;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.gecode.iris.Data;
import dev.gecode.iris.Field;
import dev.gecode.iris.Params;
import dev.gecode.iris.RecordReader;
import dev.gecode.iris.Spec;
import dev.gecode.iris.Value;
import dev.gecode.iris.internal.syntax.Ast;
import dev.gecode.iris.internal.syntax.LargeStack;
import dev.gecode.iris.internal.syntax.Lexer;
import dev.gecode.iris.internal.syntax.OutputEntries;
import dev.gecode.iris.internal.syntax.OutputEntry;
import dev.gecode.iris.internal.syntax.OutputEntry.Leaf;
import dev.gecode.iris.internal.syntax.OutputEntry.Shape;
import dev.gecode.iris.internal.syntax.Parser;
import dev.gecode.iris.internal.syntax.Resolution;
import dev.gecode.iris.internal.syntax.Resolver;
import java.io.IOException;
import java.io.InputStream;
import java.lang.reflect.Modifier;
import java.lang.reflect.RecordComponent;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.stream.Stream;
import org.jspecify.annotations.Nullable;
import org.junit.jupiter.api.Test;

/**
 * Holds the views that {@code iris gen --lang java} and {@code --lang kotlin} print, and the
 * classes that iris-processor generates, to what they promise, for every format and the specs of
 * {@code specs/}:
 *
 * <ul>
 *   <li>each view has a handle for every entry, in the order of the entries, with the type that the
 *       Java entries of the spec give, written here without the printer's help;
 *   <li>the processor's handles have the identifiers and types of the CLI's Java view, which
 *       compares the processor's Java port of the naming with the Rust printer;
 *   <li>every case reads the same through every handle of the three as through untyped access, and
 *       the processor's record holds the same values, so a handle that promised the wrong value
 *       type, depth or presence would fail, since {@code data.get} checks them.
 * </ul>
 */
final class ViewsTest {
  private static final Path REPOSITORY = Path.of(System.getProperty("iris.repository"));
  private static final String NULLABLE = "@org.jspecify.annotations.Nullable() ";
  private static final int DEEPEST_VIEW_TYPE = 3;
  private static final Pattern PARAM = Pattern.compile("\"((?:[^\"\\\\]|\\\\.)*)\"");

  /** A spec with a view, and its classes. */
  private record View(String name, Path spec) {
    Class<?> java() throws ClassNotFoundException {
      return Class.forName("dev.gecode.iris.views.java." + name);
    }

    Class<?> kotlin() throws ClassNotFoundException {
      return Class.forName("dev.gecode.iris.views.kotlin." + name);
    }

    Class<?> handles() throws ClassNotFoundException {
      return Class.forName("dev.gecode.iris.views.processed." + name + "Handles");
    }

    String origin() {
      return REPOSITORY.relativize(spec).toString().replace('\\', '/');
    }
  }

  /** One read of a view's spec. */
  private record Read(Path input, Params params) {}

  private static List<View> views() throws IOException {
    try (InputStream manifest = ViewsTest.class.getResourceAsStream("/views.tsv")) {
      assertNotNull(manifest, "the build lists the views in views.tsv");
      List<View> views = new ArrayList<>();
      for (String line : new String(manifest.readAllBytes(), UTF_8).lines().toList()) {
        String[] fields = line.split("\t", 2);
        views.add(new View(fields[0], Path.of(fields[1])));
      }
      return views;
    }
  }

  /**
   * The reads of a spec: each case of a format with expected data, or the input beside a spec of
   * {@code specs/}, with its parameters in a {@code .params} file of {@code name=value} lines.
   */
  private static List<Read> reads(View view) throws IOException {
    if (view.spec().startsWith(REPOSITORY.resolve("formats"))) {
      List<Read> reads = new ArrayList<>();
      try (Stream<Path> cases = Files.list(view.spec().resolveSibling("cases"))) {
        for (Path path : cases.sorted().toList()) {
          if (Files.exists(path.resolve("expected.json"))) {
            reads.add(new Read(path.resolve("input.txt"), caseParams(path.resolve("params.json"))));
          }
        }
      }
      return reads;
    }
    String stem = view.spec().getFileName().toString().replaceFirst("\\.iris$", "");
    Path paramsFile = view.spec().resolveSibling(stem + ".params");
    Map<String, String> params = new LinkedHashMap<>();
    if (Files.exists(paramsFile)) {
      for (String line : Files.readAllLines(paramsFile, UTF_8)) {
        String[] pair = line.split("=", 2);
        params.put(pair[0], pair[1]);
      }
    }
    return List.of(new Read(view.spec().resolveSibling(stem + ".input"), Params.of(params)));
  }

  /** The parameters of a case, whose names and values the format cases spell without escapes. */
  private static Params caseParams(Path file) throws IOException {
    if (!Files.exists(file)) {
      return Params.of();
    }
    Matcher strings = PARAM.matcher(Files.readString(file, UTF_8));
    Map<String, String> params = new LinkedHashMap<>();
    while (strings.find()) {
      String name = strings.group(1);
      assertTrue(strings.find(), "a parameter without a value in " + file);
      params.put(name, strings.group(1));
    }
    return Params.of(params);
  }

  private static List<OutputEntry> entries(View view) throws IOException {
    byte[] source = Files.readAllBytes(view.spec());
    String origin = view.origin();
    Ast.Program program = LargeStack.call(() -> Parser.parse(Lexer.lex(source, origin), origin));
    Resolution resolution = LargeStack.call(() -> Resolver.resolve(program, origin));
    return LargeStack.call(() -> OutputEntries.of(program, resolution));
  }

  /** The handle fields of a class, in the order of its declarations. */
  private static List<java.lang.reflect.Field> handleFields(Class<?> type) {
    return Arrays.stream(type.getDeclaredFields())
        .filter(field -> Modifier.isStatic(field.getModifiers()))
        .filter(field -> Field.class.isAssignableFrom(field.getType()))
        .toList();
  }

  private static Field<?> handle(java.lang.reflect.Field field) {
    try {
      field.setAccessible(true);
      return (Field<?>) field.get(null);
    } catch (IllegalAccessException e) {
      throw new AssertionError(e);
    }
  }

  /**
   * The type of the handle of {@code entry}, as {@link java.lang.reflect.AnnotatedType} prints it:
   * {@code IntField<Long>} with {@code int0} to {@code int3} and their kin up to depth 3, {@code
   * Value} beyond, {@code DynField} for an entry without one value type and depth, and JSpecify's
   * {@code @Nullable} on the view type of an entry that may be absent, if {@code annotated}.
   */
  private static String javaType(OutputEntry entry, boolean annotated) {
    if (!(entry.shape() instanceof Shape.Typed typed)) {
      return "dev.gecode.iris.build.DynField";
    }
    Leaf leaf = typed.leaf();
    String handle =
        switch (leaf) {
          case INT -> "IntField";
          case FLOAT -> "FloatField";
          case NUM -> "NumField";
          case STRING -> "StringField";
        };
    String nullable = annotated && entry.possiblyAbsent() ? NULLABLE : "";
    String view;
    if (typed.depth() > DEEPEST_VIEW_TYPE) {
      view = nullable + "dev.gecode.iris.Value";
    } else if (typed.depth() == 0) {
      view =
          nullable
              + switch (leaf) {
                case INT -> "java.lang.Long";
                case FLOAT -> "java.lang.Double";
                case NUM -> "java.lang.Number";
                case STRING -> "java.lang.String";
              };
    } else {
      String row =
          switch (leaf) {
            case INT -> "long[]";
            case FLOAT -> "double[]";
            case NUM -> "java.util.List<java.lang.Number>";
            case STRING -> "java.util.List<java.lang.String>";
          };
      if (typed.depth() == 1) {
        view =
            row.endsWith("[]") && !nullable.isEmpty()
                ? row.replace("[]", " " + nullable.strip() + "[]")
                : nullable + row;
      } else {
        view =
            nullable
                + "java.util.List<".repeat(typed.depth() - 1)
                + row
                + ">".repeat(typed.depth() - 1);
      }
    }
    return "dev.gecode.iris.build." + handle + "<" + view + ">";
  }

  @Test
  void viewsHaveAHandleOfThePromisedTypeForEveryEntry() throws Exception {
    List<View> views = views();
    assertEquals(
        6, views.stream().filter(view -> view.origin().startsWith("formats/")).count(), "formats");
    int handles = 0;
    for (View view : views) {
      List<OutputEntry> entries = entries(view);
      List<String> names = entries.stream().map(OutputEntry::name).toList();
      List<String> types = entries.stream().map(entry -> javaType(entry, true)).toList();
      List<java.lang.reflect.Field> java = handleFields(view.java());
      assertEquals(names, java.stream().map(field -> handle(field).name()).toList(), view.name());
      assertEquals(
          types,
          java.stream().map(field -> field.getAnnotatedType().toString()).toList(),
          view.name());
      // Kotlin keeps a type's nullability in its own metadata, not in the field's signature.
      List<java.lang.reflect.Field> kotlin = handleFields(view.kotlin());
      assertEquals(names, kotlin.stream().map(field -> handle(field).name()).toList(), view.name());
      assertEquals(
          entries.stream().map(entry -> javaType(entry, false)).toList(),
          kotlin.stream().map(field -> field.getGenericType().getTypeName()).toList(),
          view.name());
      handles += java.size() + kotlin.size();
    }
    System.out.printf("%d views, %d handles pinned%n", views.size() * 2, handles);
  }

  @Test
  void theProcessorsHandlesAreTheCliView() throws Exception {
    int handles = 0;
    for (View view : views()) {
      List<java.lang.reflect.Field> cli = handleFields(view.java());
      List<java.lang.reflect.Field> processed = handleFields(view.handles());
      assertEquals(
          cli.stream().map(java.lang.reflect.Field::getName).toList(),
          processed.stream().map(java.lang.reflect.Field::getName).toList(),
          view.name());
      assertEquals(
          cli.stream().map(field -> field.getAnnotatedType().toString()).toList(),
          processed.stream().map(field -> field.getAnnotatedType().toString()).toList(),
          view.name());
      handles += processed.size();
    }
    System.out.printf("%d processed classes, %d handles compared%n", views().size(), handles);
  }

  @Test
  void everyCaseReadsTheSameThroughEveryHandle() throws Exception {
    int reads = 0;
    int comparisons = 0;
    int records = 0;
    for (View view : views()) {
      Spec spec = Spec.parse(Files.readAllBytes(view.spec()), view.origin());
      Spec processed = (Spec) view.handles().getField("spec").get(null);
      List<java.lang.reflect.Field> fields = new ArrayList<>(handleFields(view.java()));
      fields.addAll(handleFields(view.kotlin()));
      fields.addAll(handleFields(view.handles()));
      RecordReader<?> reader = recordReader(view);
      List<Read> viewReads = reads(view);
      assertTrue(!viewReads.isEmpty(), "no reads of " + view.name());
      for (Read read : viewReads) {
        byte[] input = Files.readAllBytes(read.input());
        String origin = read.input().getFileName().toString();
        Data data = spec.read(input, origin, read.params());
        assertEquals(
            data.toJson(), processed.read(input, origin, read.params()).toJson(), view.name());
        Set<String> named = new HashSet<>();
        for (java.lang.reflect.Field field : fields) {
          Field<?> handle = handle(field);
          named.add(handle.name());
          Value untyped = data.find(handle.name());
          assertEquals(
              untyped == null ? null : normal(untyped),
              normal(data.get(handle)),
              view.name() + "." + field.getName() + " on " + read.input());
          comparisons++;
        }
        for (Data.Entry entry : data.entries()) {
          assertTrue(named.contains(entry.name()), "no handle for " + entry.name());
        }
        if (reader != null) {
          Record record = reader.read(input, origin, read.params());
          for (RecordComponent component : record.getClass().getRecordComponents()) {
            Field<?> handle = handle(view.handles().getDeclaredField(component.getName()));
            assertEquals(
                normal(data.get(handle)),
                normal(component.getAccessor().invoke(record)),
                view.name() + "Data." + component.getName() + " on " + read.input());
            comparisons++;
          }
          records++;
        }
        reads++;
      }
    }
    System.out.printf(
        "%d reads, %d through records, %d values compared with untyped access%n",
        reads, records, comparisons);
  }

  private static @Nullable RecordReader<?> recordReader(View view)
      throws ReflectiveOperationException {
    try {
      return (RecordReader<?>) view.handles().getField("reader").get(null);
    } catch (NoSuchFieldException e) {
      return null;
    }
  }

  /** A value as lists, longs, doubles and strings, which compare by {@code equals}. */
  private static @Nullable Object normal(@Nullable Object value) {
    return switch (value) {
      case null -> null;
      case Value v -> normal(v);
      case long[] longs -> Arrays.stream(longs).boxed().toList();
      case double[] doubles -> Arrays.stream(doubles).boxed().toList();
      case List<?> list -> list.stream().map(ViewsTest::normal).toList();
      case Long number -> number;
      case Double number -> number;
      case String text -> text;
      default -> throw new AssertionError("unexpected " + value.getClass());
    };
  }

  private static Object normal(Value value) {
    if (value.depth() > 0) {
      List<@Nullable Object> items = new ArrayList<>();
      for (int index = 0; index < value.size(); index++) {
        items.add(normal(value.get(index)));
      }
      return items;
    }
    return switch (value.type()) {
      case INT -> value.asLong();
      case FLOAT -> value.asDouble();
      case STRING -> value.asString();
    };
  }
}
