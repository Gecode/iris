package dev.gecode.iris;

import static java.nio.charset.StandardCharsets.UTF_8;
import static java.util.Objects.requireNonNull;
import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.UncheckedIOException;
import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;
import java.lang.invoke.MethodHandles;
import java.lang.module.Configuration;
import java.lang.module.ModuleFinder;
import java.lang.reflect.RecordComponent;
import java.net.URL;
import java.net.URLClassLoader;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;
import java.util.stream.Collectors;
import java.util.stream.IntStream;
import javax.tools.ToolProvider;
import org.jspecify.annotations.Nullable;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/** Reading data into records through {@link Spec#reader}. */
class RecordReaderTest {
  private static final Path JOBSHOP =
      Path.of(System.getProperty("iris.repository"), "formats", "jobshop");

  record Shop(
      @Nullable String name,
      @Entry("n_jobs") long nJobs,
      @Entry("n_machines") Long nMachines,
      List<long[]> machine,
      List<long[]> duration) {}

  private static Spec jobshop() {
    try {
      return Spec.parse(Files.readAllBytes(JOBSHOP.resolve("jobshop.iris")), "jobshop.iris");
    } catch (IOException e) {
      throw new UncheckedIOException(e);
    }
  }

  @Test
  void readsTheJobshopFormatIntoARecord() {
    Spec spec = jobshop();
    Path input = JOBSHOP.resolve("cases/jsplib/input.txt");
    Shop shop = spec.reader(Shop.class, MethodHandles.lookup()).read(input);
    assertNull(shop.name());
    assertEquals(3, shop.nJobs());
    assertEquals(3, shop.nMachines());
    assertEquals(3, shop.machine().size());
    assertArrayEquals(new long[] {0, 2, 1}, shop.machine().get(0));
    assertArrayEquals(new long[] {3, 2, 4}, shop.duration().get(0));
    assertArrayEquals(new long[] {4, 3, 2}, shop.duration().get(2));
    assertEquals(spec.read(input).get("machine").get(1).get(2).asLong(), shop.machine().get(1)[2]);
  }

  @Test
  void readsAPresentOptionalEntry() {
    Spec spec = jobshop();
    Shop shop = spec.reader(Shop.class, MethodHandles.lookup()).read("instance ft01\n1 1\n0 5\n");
    assertEquals("ft01", shop.name());
    assertArrayEquals(new long[] {5}, shop.duration().get(0));
  }

  record Scalars(
      long count, Double ratio, String label, Number amount, double scale, Value whole) {}

  @Test
  void readsEachValueTypeAtDepthZero() {
    var reader =
        Spec.parse("count:int ratio:float label:word amount:num scale:float whole:int")
            .reader(Scalars.class, MethodHandles.lookup());
    assertEquals(
        new Scalars(4, 0.5, "x", 7L, 2.0, new Value.IntValue(9)), reader.read("4 0.5 x 7 2 9"));
    assertEquals(2.5, reader.read("4 0.5 x 2.5 2 9").amount());
  }

  // Arrays compare by identity, which these tests do not rely on.
  @SuppressWarnings("ArrayRecordComponent")
  record Columns(
      long[] counts,
      double[] ratios,
      List<String> labels,
      List<Number> amounts,
      List<List<long[]>> cube) {}

  @Test
  void readsArraysAsArraysAndLists() {
    var reader =
        Spec.parse(
                """
                repeat 2 {counts:int ratios:float labels:word amounts:num}
                repeat 1 {repeat 2 {repeat 2 {cube:int}}}
                """)
            .reader(Columns.class, MethodHandles.lookup());
    Columns columns = reader.read("1 0.5 a 2 3 1.5 b 2.5 7 8 9 10");
    assertArrayEquals(new long[] {1, 3}, columns.counts());
    assertArrayEquals(new double[] {0.5, 1.5}, columns.ratios());
    assertEquals(List.of("a", "b"), columns.labels());
    assertEquals(List.of(2.0, 2.5), columns.amounts());
    assertArrayEquals(new long[] {9, 10}, columns.cube().get(0).get(1));
    assertEquals(List.of(2L, 4L), reader.read("1 0.5 a 2 3 1.5 b 4 7 8 9 10").amounts());
  }

  // Arrays compare by identity, which these tests do not rely on.
  @SuppressWarnings("ArrayRecordComponent")
  record Empty(List<String> words, double[] weights) {}

  @Test
  void readsTheColumnsOfARepeatWithoutIterationsAsEmpty() {
    var reader =
        Spec.parse("n:int repeat n {let words=\"text\" weights:float}")
            .reader(Empty.class, MethodHandles.lookup());
    Empty empty = reader.read("0");
    assertEquals(List.of(), empty.words());
    assertArrayEquals(new double[0], empty.weights());
    assertEquals(List.of("text"), reader.read("1 2.5").words());
  }

  record Values(Value mixed, @Nullable Value maybe) {}

  @Test
  void readsAnyEntryAsAValue() {
    var reader =
        Spec.parse("c:int let mixed=\"none\" if c {mixed:int} if c {maybe:float}")
            .reader(Values.class, MethodHandles.lookup());
    assertEquals(
        new Values(new Value.IntValue(5), new Value.FloatValue(1.5)), reader.read("1 5 1.5"));
    assertEquals(new Values(new Value.StringValue("none"), null), reader.read("0"));
  }

  /**
   * Holds a {@code Nullable} declaration annotation like Spring's, which no record component takes.
   */
  interface Declaration {
    @Retention(RetentionPolicy.RUNTIME)
    @Target({ElementType.FIELD, ElementType.METHOD, ElementType.PARAMETER})
    @interface Nullable {}
  }

  /** Holds a {@code Nullable} annotation that only a parameter takes. */
  interface Parameter {
    @Retention(RetentionPolicy.RUNTIME)
    @Target(ElementType.PARAMETER)
    @interface Nullable {}
  }

  record Legacy(@Declaration.Nullable Long x, @Parameter.Nullable Long y) {}

  @Test
  void acceptsANullableDeclarationAnnotationOnTheAccessorFieldOrParameter() {
    var reader =
        Spec.parse("c:int if c {x:int y:int}").reader(Legacy.class, MethodHandles.lookup());
    assertEquals(new Legacy(7L, 8L), reader.read("1 7 8"));
    assertEquals(new Legacy(null, null), reader.read("0"));
  }

  record Sized(long n) {}

  @Test
  void readsEveryKindOfInput() {
    var reader = Spec.parse("param size=1 let n=size").reader(Sized.class, MethodHandles.lookup());
    Params params = Params.of("size", "3");
    assertEquals(new Sized(1), reader.read(new byte[0]));
    assertEquals(new Sized(1), reader.read(new byte[0], "input"));
    assertEquals(new Sized(3), reader.read(new byte[0], "input", params));
    assertEquals(new Sized(1), reader.read(""));
    assertEquals(new Sized(1), reader.read("", "input"));
    assertEquals(new Sized(3), reader.read("", "input", params));
  }

  @Test
  void reportsReadErrorsAsTheSpecDoes() {
    var reader = Spec.parse("n:int").reader(Sized.class, MethodHandles.lookup());
    IrisException error =
        assertThrows(IrisException.class, () -> reader.read("x".getBytes(UTF_8), "input"));
    IrisException expected =
        assertThrows(IrisException.class, () -> Spec.parse("n:int").read("x", "input"));
    assertEquals(expected.error(), error.error());
  }

  private static String creationError(String spec, Class<? extends Record> type) {
    return requireNonNull(
        assertThrows(
                IllegalArgumentException.class,
                () -> Spec.parse(spec).reader(type, MethodHandles.lookup()))
            .getMessage());
  }

  record Count(long count) {}

  record Rows(List<long[]> count) {}

  record Label(String count) {}

  @Test
  void reportsAShapeMismatchWhenTheReaderIsCreated() {
    assertEquals(
        "the component 'count' of Count has type long, but the entry 'count' is int at depth 1,"
            + " so it reads as long[] or Value",
        creationError("repeat 2 {count:int}", Count.class));
    assertEquals(
        "the component 'count' of Rows has type List<long[]>, but the entry 'count' is int at"
            + " depth 1, so it reads as long[] or Value",
        creationError("repeat 2 {count:int}", Rows.class));
    assertEquals(
        "the component 'count' of Label has type String, but the entry 'count' is int at depth 0,"
            + " so it reads as long, Long or Value",
        creationError("count:int", Label.class));
    assertEquals(
        "the component 'count' of Count has type long, but the entry 'count' is num at depth 0,"
            + " so it reads as Number or Value",
        creationError("count:num", Count.class));
  }

  @Test
  void reportsAnEntryWithoutOneShapeWhenTheReaderIsCreated() {
    assertEquals(
        "the component 'count' of Count has type long, but the entry 'count' has no single value"
            + " type and depth, so it reads as Value",
        creationError("count:int let count=\"x\"", Count.class));
  }

  @Test
  void reportsAPossiblyAbsentEntryWithoutNullableWhenTheReaderIsCreated() {
    assertEquals(
        "the component 'count' of Count has type long, but the entry 'count' may be absent, so the"
            + " type needs @Nullable",
        creationError("c:int if c {count:int}", Count.class));
    assertEquals(
        "the component 'count' of Label has type String, but the entry 'count' may be absent, so"
            + " the type needs @Nullable",
        creationError("c:int if c {let count=\"x\"}", Label.class));
  }

  record Renamed(@Entry("n_jobs") long count) {}

  record Camel(long nJobs) {}

  @Test
  void reportsAComponentWithoutAnEntryWhenTheReaderIsCreated() {
    assertEquals(
        "the component 'count' of Count reads the entry 'count', which no read of the spec"
            + " produces",
        creationError("repeat 2 as count {x:int}", Count.class));
    // A component's name is never changed to match an entry.
    assertEquals(
        "the component 'nJobs' of Camel reads the entry 'nJobs', which no read of the spec"
            + " produces",
        creationError("n_jobs:int", Camel.class));
    assertEquals(
        "the component 'count' of Renamed reads the entry 'n_jobs', which no read of the spec"
            + " produces",
        creationError("count:int", Renamed.class));
  }

  private record Hidden(long count) {}

  @Test
  void reportsALookupWithoutAccessWhenTheReaderIsCreated() {
    Spec spec = Spec.parse("count:int");
    var error =
        assertThrows(
            IllegalArgumentException.class,
            () -> spec.reader(Hidden.class, MethodHandles.publicLookup()));
    assertEquals(
        "the lookup cannot access the canonical constructor of " + Hidden.class.getName(),
        error.getMessage());
    assertInstanceOf(IllegalAccessException.class, error.getCause());
    assertEquals(new Hidden(2), spec.reader(Hidden.class, MethodHandles.lookup()).read("2"));
  }

  @Test
  void rejectsAClassThatIsNotARecord() {
    assertEquals(
        "java.lang.Record is not a record class", creationError("count:int", Record.class));
  }

  /**
   * Compiles {@code source} into {@code directory}, which a loader of its own then loads from, and
   * returns the class {@code name}, or fails with javac's messages.
   */
  private static Class<?> compile(Path directory, String name, String source) throws Exception {
    Path file = directory.resolve(name + ".java");
    Files.writeString(file, source);
    var messages = new ByteArrayOutputStream();
    int status = javac(directory, file, messages);
    assertEquals(0, status, messages.toString(UTF_8));
    var loader =
        new URLClassLoader(
            new URL[] {directory.toUri().toURL()}, RecordReaderTest.class.getClassLoader());
    return Class.forName(name, true, loader);
  }

  private static int javac(Path directory, Path file, ByteArrayOutputStream messages) {
    return requireNonNull(ToolProvider.getSystemJavaCompiler())
        .run(
            null,
            messages,
            messages,
            "-proc:none",
            "--release",
            "27",
            "-classpath",
            System.getProperty("java.class.path"),
            "-d",
            directory.toString(),
            file.toString());
  }

  /** {@code count} components {@code long c0}, {@code long c1} and so on, each two slots. */
  private static String longs(int count) {
    return IntStream.range(0, count).mapToObj(k -> "long c" + k).collect(Collectors.joining(", "));
  }

  /**
   * Records whose components take 253 parameter slots, the most a method handle can pass to a
   * constructor, and 254, the most a constructor can take, with a static method that returns a
   * lookup of the nest.
   */
  private static final String WIDE =
      """
      public final class Wide {
        public static java.lang.invoke.MethodHandles.Lookup lookup() {
          return java.lang.invoke.MethodHandles.lookup();
        }

        public record Longs253(%1$s, String s0) {}

        public record Longs254(%2$s) {
          public Longs254 {
            if (c0 < 0) {
              throw new IllegalArgumentException("c0 is negative");
            }
          }
        }

        public record Mixed254(%1$s, String s0, String s1) {}

        private record Hidden254(%2$s) {}
      }
      """
          .formatted(longs(126), longs(127));

  /** The spec of a wide record: an int for each long component and a word for each string. */
  private static String wideSpec(int longs, int strings) {
    return IntStream.range(0, longs)
            .mapToObj(k -> "c" + k + ":int")
            .collect(Collectors.joining(" "))
        + IntStream.range(0, strings)
            .mapToObj(k -> " s" + k + ":word")
            .collect(Collectors.joining());
  }

  /** The input of {@link #wideSpec}: each long component its index, and each string its name. */
  private static String wideInput(int longs, int strings) {
    return IntStream.range(0, longs).mapToObj(Integer::toString).collect(Collectors.joining(" "))
        + IntStream.range(0, strings).mapToObj(k -> " s" + k).collect(Collectors.joining());
  }

  private static List<@Nullable Object> components(Record record) throws Exception {
    List<@Nullable Object> values = new ArrayList<>();
    for (RecordComponent component : record.getClass().getRecordComponents()) {
      component.getAccessor().setAccessible(true);
      values.add(component.getAccessor().invoke(record));
    }
    return values;
  }

  private static List<Object> expected(int longs, int strings) {
    List<Object> values = new ArrayList<>();
    IntStream.range(0, longs).forEach(k -> values.add((long) k));
    IntStream.range(0, strings).forEach(k -> values.add("s" + k));
    return values;
  }

  private static Class<? extends Record> nested(Class<?> outer, String name)
      throws ClassNotFoundException {
    return Class.forName(outer.getName() + "$" + name, true, outer.getClassLoader())
        .asSubclass(Record.class);
  }

  private static MethodHandles.Lookup lookupOf(Class<?> outer) throws Exception {
    return (MethodHandles.Lookup) requireNonNull(outer.getMethod("lookup").invoke(null));
  }

  @Test
  void readsRecordsAtTheMostSlotsOfAMethodHandleAndOfAConstructor(@TempDir Path directory)
      throws Exception {
    Class<?> wide = compile(directory, "Wide", WIDE);
    MethodHandles.Lookup lookup = lookupOf(wide);
    record Case(String name, int longs, int strings) {}
    for (Case wideCase :
        List.of(
            new Case("Longs253", 126, 1),
            new Case("Longs254", 127, 0),
            new Case("Mixed254", 126, 2))) {
      Record record =
          Spec.parse(wideSpec(wideCase.longs(), wideCase.strings()))
              .reader(nested(wide, wideCase.name()), lookup)
              .read(wideInput(wideCase.longs(), wideCase.strings()));
      assertEquals(expected(wideCase.longs(), wideCase.strings()), components(record));
    }
  }

  @Test
  void passesOnWhatTheConstructorOfAWideRecordThrows(@TempDir Path directory) throws Exception {
    Class<?> wide = compile(directory, "Wide", WIDE);
    var reader = Spec.parse(wideSpec(127, 0)).reader(nested(wide, "Longs254"), lookupOf(wide));
    var error =
        assertThrows(
            IllegalArgumentException.class,
            () -> reader.read(wideInput(127, 0).replaceFirst("0", "-1")));
    assertEquals("c0 is negative", error.getMessage());
  }

  @Test
  void constructsAWideRecordWithTheAccessOfTheLookupClass(@TempDir Path directory)
      throws Exception {
    Class<?> wide = compile(directory, "Wide", WIDE);
    Class<? extends Record> hidden = nested(wide, "Hidden254");
    Spec spec = Spec.parse(wideSpec(127, 0));
    assertEquals(
        expected(127, 0), components(spec.reader(hidden, lookupOf(wide)).read(wideInput(127, 0))));
    var denied =
        assertThrows(
            IllegalArgumentException.class, () -> spec.reader(hidden, MethodHandles.lookup()));
    assertEquals(
        "the lookup cannot access the canonical constructor of Wide$Hidden254",
        denied.getMessage());
    assertInstanceOf(IllegalAccessException.class, denied.getCause());
    var restricted =
        assertThrows(
            IllegalArgumentException.class,
            () -> spec.reader(nested(wide, "Longs254"), MethodHandles.publicLookup()));
    assertEquals(
        "the components of Wide$Longs254 take 254 parameter slots, more than the 253 that a method"
            + " handle can pass, so the lookup needs original access, as MethodHandles.lookup()"
            + " has",
        restricted.getMessage());
  }

  /**
   * Compiles a module {@code records} of records whose components take 253 and 254 slots, in the
   * exported package {@code records} and the closed package {@code closed}, and modules {@code
   * readers}, which reads {@code records}, and {@code strangers}, which does not, each with a class
   * {@code Lookups} whose static method {@code lookup} returns a lookup of the module, and returns
   * the layer of the three.
   */
  private static ModuleLayer modules(Path directory) throws IOException {
    Path sources = directory.resolve("src");
    var files = new ArrayList<Path>();
    var texts = new ArrayList<String>();
    files.add(Path.of("records", "module-info.java"));
    texts.add("module records { exports records; }");
    for (String packageName : List.of("records", "closed")) {
      files.add(Path.of("records", packageName, "Wide253.java"));
      texts.add(
          "package %s; public record Wide253(%s, String s0) {}".formatted(packageName, longs(126)));
      files.add(Path.of("records", packageName, "Wide254.java"));
      texts.add(
          "package %s; public record Wide254(%s, String s0, String s1) {}"
              .formatted(packageName, longs(126)));
    }
    for (String module : List.of("readers", "strangers")) {
      files.add(Path.of(module, "module-info.java"));
      texts.add(
          "module %s { %s exports %1$s; }"
              .formatted(module, module.equals("readers") ? "requires records;" : ""));
      files.add(Path.of(module, module, "Lookups.java"));
      texts.add(
          """
          package %s;

          public final class Lookups {
            public static java.lang.invoke.MethodHandles.Lookup lookup() {
              return java.lang.invoke.MethodHandles.lookup();
            }
          }
          """
              .formatted(module));
    }
    Path classes = directory.resolve("classes");
    var arguments =
        new ArrayList<String>(
            List.of(
                "-proc:none",
                "--module-source-path",
                sources.toString(),
                "-d",
                classes.toString()));
    for (int index = 0; index < files.size(); index++) {
      Path file = sources.resolve(files.get(index));
      Files.createDirectories(file.getParent());
      Files.writeString(file, texts.get(index));
      arguments.add(file.toString());
    }
    var messages = new ByteArrayOutputStream();
    int status =
        requireNonNull(ToolProvider.getSystemJavaCompiler())
            .run(null, messages, messages, arguments.toArray(String[]::new));
    assertEquals(0, status, messages.toString(UTF_8));
    ModuleLayer boot = ModuleLayer.boot();
    Configuration configuration =
        boot.configuration()
            .resolve(
                ModuleFinder.of(classes),
                ModuleFinder.of(),
                Set.of("records", "readers", "strangers"));
    return boot.defineModulesWithOneLoader(configuration, ClassLoader.getSystemClassLoader());
  }

  @Test
  void checksTheModuleAccessOfTheLookupAtEitherNumberOfSlots(@TempDir Path directory)
      throws Exception {
    ModuleLayer layer = modules(directory);
    ClassLoader records = layer.findLoader("records");
    for (String module : List.of("readers", "strangers")) {
      MethodHandles.Lookup lookup =
          lookupOf(Class.forName(module + ".Lookups", true, layer.findLoader(module)));
      for (String packageName : List.of("records", "closed")) {
        for (int strings = 1; strings <= 2; strings++) {
          Class<? extends Record> type =
              Class.forName(packageName + ".Wide" + (252 + strings), true, records)
                  .asSubclass(Record.class);
          Spec spec = Spec.parse(wideSpec(126, strings));
          // Only a module that reads the record's module, which exports the record's package, can
          // construct it, whether through a method handle or through reflection.
          if (module.equals("readers") && packageName.equals("records")) {
            Record record = spec.reader(type, lookup).read(wideInput(126, strings));
            assertEquals(expected(126, strings), components(record));
          } else {
            var denied =
                assertThrows(IllegalArgumentException.class, () -> spec.reader(type, lookup));
            assertEquals(
                "the lookup cannot access the canonical constructor of " + type.getName(),
                denied.getMessage(),
                module + " constructing " + type.getName());
            assertInstanceOf(IllegalAccessException.class, denied.getCause());
          }
        }
      }
    }
  }

  @Test
  void aRecordCannotTakeMoreThan254Slots(@TempDir Path directory) throws IOException {
    // The reader needs no case beyond 254 slots: javac rejects a record of 255.
    Path file = directory.resolve("Widest.java");
    Files.writeString(file, "public record Widest(" + longs(127) + ", int c127) {}\n");
    var messages = new ByteArrayOutputStream();
    assertEquals(1, javac(directory, file, messages));
    assertTrue(messages.toString(UTF_8).contains("too many parameters"), messages.toString(UTF_8));
  }
}
