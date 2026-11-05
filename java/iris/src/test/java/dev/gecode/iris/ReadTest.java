package dev.gecode.iris;

import static java.nio.charset.StandardCharsets.UTF_8;
import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/** The public reading API: what the conformance cases, which see only documents, cannot show. */
class ReadTest {
  /** The levels of a left spine of 32,385 operators, which §5.12 allows as it ends each chain. */
  private static final int SPINE_LEVELS = 255;

  private static Data read(String spec, String input) {
    return Spec.parse(spec).read(input);
  }

  private static IrisError errorOf(String spec, String input, Params params) {
    Spec parsed = Spec.parse(spec, "spec");
    return assertThrows(IrisException.class, () -> parsed.read(input, "input", params)).error();
  }

  @Test
  void keepsTheEntriesInDefinitionOrder() {
    Data data = read("b: int\na: float\nc: word\nb: int", "1 2.5 x 3");
    assertEquals(List.of("b", "a", "c"), data.entries().stream().map(Data.Entry::name).toList());
    assertEquals(new Value.IntValue(3), data.get("b"));
    assertEquals(new Value.FloatValue(2.5), data.get("a"));
    assertEquals(new Value.StringValue("x"), data.get("c"));
  }

  @Test
  void excludesWhatTheReaderKeepsForItself() {
    Data data = read("repeat 2 {\nx: int\n}", "1 2");
    assertEquals(List.of("x"), data.entries().stream().map(Data.Entry::name).toList());
  }

  @Test
  void findsAndGetsEntries() {
    Data data = read("a: int", "1");
    assertEquals(new Value.IntValue(1), data.find("a"));
    assertNull(data.find("b"));
    assertThrows(IllegalStateException.class, () -> data.get("b"));
  }

  @Test
  void entriesCannotBeChanged() {
    List<Data.Entry> entries = read("a: int", "1").entries();
    assertThrows(UnsupportedOperationException.class, entries::removeFirst);
  }

  @Test
  void writesTheCanonicalDocument() {
    assertEquals(
        "{\"iris\":\"0.1\",\"entries\":[{\"name\":\"a\",\"type\":\"int\",\"depth\":1,"
            + "\"value\":[1,2]}]}\n",
        read("a[2]: int", "1 2").toJson());
  }

  @Test
  void readsScalars() {
    Data data = read("i: int\nf: float\ns: word", "7 1.5 å");
    assertEquals(ValueType.INT, data.get("i").type());
    assertEquals(0, data.get("i").depth());
    assertEquals(7, data.get("i").asLong());
    assertEquals(7.0, data.get("i").asDouble());
    assertEquals(1.5, data.get("f").asDouble());
    assertEquals("å", data.get("s").asString());
  }

  @Test
  void readsColumns() {
    Data data = read("i[2]: int\nf[2]: float\ns[2]: word", "1 2 0.5 1.5 a b");
    assertArrayEquals(new long[] {1, 2}, data.get("i").longs());
    assertArrayEquals(new int[] {1, 2}, data.get("i").ints());
    assertArrayEquals(new double[] {1, 2}, data.get("i").doubles());
    assertArrayEquals(new double[] {0.5, 1.5}, data.get("f").doubles());
    assertEquals(List.of("a", "b"), data.get("s").strings());
  }

  @Test
  void walksRaggedArrays() {
    Value rows = read("repeat 3 {\nn: int\na[n]: int\n}", "2 1 2 0 1 3").get("a");
    assertEquals(2, rows.depth());
    assertEquals(3, rows.size());
    assertEquals(
        List.of(2, 0, 1),
        List.of(rows.get(0), rows.get(1), rows.get(2)).stream().map(Value::size).toList());
    assertEquals(new Value.IntValue(3), rows.at(2, 0));
    assertEquals(rows, rows.at());
    assertInstanceOf(Value.ArrayValue.class, rows.get(1));
    assertArrayEquals(new long[0], rows.get(1).longs());
    assertArrayEquals(new long[] {3}, rows.get(2).longs());
    assertEquals("[[1,2],[],[3]]", rows.toString());
  }

  @Test
  void aColumnIsACopy() {
    Value column = read("a[2]: int", "1 2").get("a");
    column.longs()[0] = 9;
    column.ints()[0] = 9;
    column.doubles()[0] = 9;
    assertEquals(1, column.longs()[0]);
    assertEquals(new Value.IntValue(1), column.get(0));
    Value floats = read("a[2]: float", "0.5 1.5").get("a");
    floats.doubles()[0] = 9;
    assertEquals(new Value.FloatValue(0.5), floats.get(0));
  }

  @Test
  void aListOfStringsCannotBeChanged() {
    List<String> words = read("a[2]: word", "x y").get("a").strings();
    assertThrows(UnsupportedOperationException.class, () -> words.set(0, "z"));
    assertThrows(UnsupportedOperationException.class, () -> words.add("z"));
    assertEquals(List.of("x", "y"), words);
  }

  @Test
  void rowsOfRaggedStringsAreLists() {
    Value rows = read("repeat 2 {\nn: int\na[n]: word\n}", "2 x y 1 z").get("a");
    assertEquals(List.of("x", "y"), rows.get(0).strings());
    assertEquals(List.of("z"), rows.get(1).strings());
  }

  @Test
  void rejectsAnIntegerThatDoesNotFitAnInt() {
    Value column = read("a[1]: int", "4294967296").get("a");
    assertThrows(ArithmeticException.class, column::ints);
    assertEquals(4294967296L, column.longs()[0]);
  }

  @Test
  void misusingAnAccessorIsNotAReadError() {
    Data data = read("i: int\ns: word\na[1]: int\nb[1][1]: int\nw[1]: word", "1 x 2 3 y");
    Value integer = data.get("i");
    assertThrows(IllegalStateException.class, integer::asString);
    assertThrows(IllegalStateException.class, integer::size);
    assertThrows(IllegalStateException.class, () -> integer.get(0));
    assertThrows(IllegalStateException.class, () -> integer.at(0));
    assertThrows(IllegalStateException.class, integer::longs);
    assertThrows(IllegalStateException.class, integer::ints);
    assertThrows(IllegalStateException.class, integer::doubles);
    assertThrows(IllegalStateException.class, integer::strings);
    assertThrows(IllegalStateException.class, data.get("s")::asLong);
    assertThrows(IllegalStateException.class, data.get("s")::asDouble);
    assertThrows(IllegalStateException.class, data.get("a")::asLong);
    assertThrows(IllegalStateException.class, data.get("a")::asDouble);
    assertThrows(IllegalStateException.class, data.get("a")::asString);
    assertThrows(IllegalStateException.class, data.get("a")::strings);
    assertThrows(IllegalStateException.class, data.get("b")::longs);
    assertThrows(IllegalStateException.class, data.get("b")::ints);
    assertThrows(IllegalStateException.class, data.get("w")::doubles);
    assertThrows(IndexOutOfBoundsException.class, () -> data.get("a").get(1));
    assertThrows(IndexOutOfBoundsException.class, () -> data.get("a").get(-1));
  }

  @Test
  void everyOverloadReadsTheSameData(@TempDir Path directory) throws IOException {
    Spec spec = Spec.parse("param k = 1\na: int\nlet b = a + k");
    Path file = Files.writeString(directory.resolve("input"), "2");
    Params params = Params.of("k", "5");
    String plain = spec.read(file).toJson();
    String offset = spec.read(file, params).toJson();
    assertEquals(plain, spec.read("2").toJson());
    assertEquals(plain, spec.read("2", "origin").toJson());
    assertEquals(plain, spec.read("2".getBytes(UTF_8)).toJson());
    assertEquals(plain, spec.read("2".getBytes(UTF_8), "origin").toJson());
    assertEquals(offset, spec.read("2", "origin", params).toJson());
    assertEquals(offset, spec.read("2".getBytes(UTF_8), "origin", params).toJson());
    assertEquals(7, spec.read(file, params).get("b").asLong());
  }

  @Test
  void aPathIsTheOriginOfItsInput(@TempDir Path directory) throws IOException {
    Path file = Files.writeString(directory.resolve("input"), "x");
    var failure = assertThrows(IrisException.class, () -> Spec.parse("a: int").read(file));
    assertEquals(new Location(file.toString(), 1, 1), failure.error().inputLocation());
  }

  @Test
  void anUnreadableFileIsNotAReadError(@TempDir Path directory) {
    Spec spec = Spec.parse("a: int");
    Path missing = directory.resolve("missing");
    assertThrows(UncheckedIOException.class, () -> spec.read(missing));
    assertThrows(UncheckedIOException.class, () -> spec.read(missing, Params.of()));
  }

  @Test
  void rejectsAnInputOfUnpairedSurrogates() {
    Spec spec = Spec.parse("a: word");
    assertThrows(IllegalArgumentException.class, () -> spec.read("\uD83D"));
    assertEquals("😀", spec.read("😀").get("a").asString());
  }

  @Test
  void reportsAReadError() {
    IrisError error = errorOf("repeat 2 {\na: int\n}", "1 x", Params.of());
    assertEquals(ErrorCode.EXPECTED_VALUE, error.code());
    assertEquals(new Location("input", 1, 3), error.inputLocation());
    assertEquals(new Location("spec", 2, null), error.specLocation());
    assertEquals("repeat [2 of 2] > a", error.context());
    assertNull(error.recordPath());
    assertInstanceOf(IrisError.ReadError.class, error);
  }

  @Test
  void aFailedCheckHasTheRecordPath() {
    IrisError error = errorOf("repeat 2 {\na: int\n}\ncheck a > 0", "1 0", Params.of());
    assertEquals(ErrorCode.CHECK_FAILED, error.code());
    assertEquals(List.of(1), error.recordPath());
    assertInstanceOf(IrisError.CheckError.class, error);
  }

  @Test
  void writesTheErrorDocument() {
    assertEquals(
        "{\"code\":\"E-EXPECTED-VALUE\",\"message\":\"expected int, found 'x'\","
            + "\"input_location\":{\"origin\":\"input\",\"line\":1,\"column\":1},"
            + "\"spec_location\":{\"origin\":\"spec\",\"line\":1,\"column\":null},"
            + "\"context\":\"a\",\"record_path\":null}\n",
        errorOf("a: int", "x", Params.of()).toJson());
  }

  @Test
  void anExceptionPrefersTheInputLocation() {
    Spec spec = Spec.parse("a: int", "spec");
    var failure = assertThrows(IrisException.class, () -> spec.read("x", "input"));
    assertEquals("input:1:1: expected int, found 'x'", failure.getMessage());
  }

  @Test
  void paramsKeepTheirOrder() {
    Params params = Params.of("b", "1").with("a", "2").with("b", "3");
    assertEquals(List.of("b", "a"), List.copyOf(params.values().keySet()));
    assertEquals("3", params.values().get("b"));
    var map = new LinkedHashMap<String, String>();
    map.put("z", "1");
    map.put("y", "2");
    assertEquals(List.of("z", "y"), List.copyOf(Params.of(map).values().keySet()));
  }

  @Test
  void theFirstUnknownParamIsTheOneReported() {
    Params params = Params.of("z", "1").with("a", "2");
    IrisError error = errorOf("param k = 1", "", params);
    assertEquals(ErrorCode.PARAM_UNKNOWN, error.code());
    assertTrue(error.message().contains("'z'"), error.message());
    assertNull(error.inputLocation());
    assertEquals(new Location("spec", null, null), error.specLocation());
  }

  @Test
  void paramsCannotBeChanged() {
    Params first = Params.of("a", "1");
    Params second = first.with("a", "2");
    assertEquals("1", first.values().get("a"));
    assertEquals("2", second.values().get("a"));
    assertThrows(UnsupportedOperationException.class, () -> first.values().put("b", "2"));
    var map = new LinkedHashMap<>(Map.of("a", "1"));
    Params copied = Params.of(map);
    map.put("a", "2");
    assertEquals("1", copied.values().get("a"));
  }

  @Test
  void paramsMustBeText() {
    assertThrows(IllegalArgumentException.class, () -> Params.of("a", "\uD83D"));
    assertThrows(IllegalArgumentException.class, () -> Params.of("\uDE00", "a"));
    assertThrows(IllegalArgumentException.class, () -> Params.of(Map.of("a", "\uD83Dx")));
    assertThrows(IllegalArgumentException.class, () -> Params.of().with("a", "\uDE00\uD83D"));
    assertEquals("😀", Params.of("a", "😀").values().get("a"));
  }

  @Test
  void readsAtTheLegalNestingOfBlocksAndExpressions() {
    String negations = "- ".repeat(255) + "1";
    String operands = "1+(".repeat(127) + "1" + ")".repeat(127);
    String calls = "min(".repeat(255) + "1" + ")".repeat(255);
    assertEquals(-1, readNested("let a = " + negations).get("a").asLong());
    assertEquals(128, readNested("let a = " + operands).get("a").asLong());
    assertEquals(1, readNested("let a = " + calls).get("a").asLong());
  }

  private static Data readNested(String statement) {
    return read("if 1 {\n".repeat(256) + statement + "\n" + "}\n".repeat(256), "");
  }

  @Test
  void readsAVeryLongLeftSpine() {
    var spine = new StringBuilder("(".repeat(SPINE_LEVELS)).append('1');
    int operators = 0;
    for (int count = 0; count < SPINE_LEVELS; count++) {
      spine.append(')').append("+1".repeat(count));
      operators += count;
    }
    assertEquals(operators + 1, readNested("let a = " + spine).get("a").asLong());
    Data data = read("repeat " + spine + " {\n}\nlet a = 0\ncheck " + spine + " > 0", "");
    assertEquals(0, data.get("a").asLong());
  }

  @Test
  void walksAValueOfManyDimensions() {
    int depth = 50_000;
    Value value = read("a" + "[1]".repeat(depth) + ": int", "7").get("a");
    assertEquals(depth, value.depth());
    assertEquals(new Value.IntValue(7), value.at(new int[depth]));
    String json = value.toString();
    assertEquals("[".repeat(depth) + "7" + "]".repeat(depth), json);
  }
}
