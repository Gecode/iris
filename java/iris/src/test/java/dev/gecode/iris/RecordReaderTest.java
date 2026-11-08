package dev.gecode.iris;

import static java.nio.charset.StandardCharsets.UTF_8;
import static java.util.Objects.requireNonNull;
import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;

import java.io.IOException;
import java.io.UncheckedIOException;
import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;
import java.lang.invoke.MethodHandles;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import org.jspecify.annotations.Nullable;
import org.junit.jupiter.api.Test;

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
}
