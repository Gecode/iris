package dev.gecode.iris;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;

import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/** The public catalog API: what the conformance cases, which see only documents, cannot show. */
class CatalogTest {
  private static final String SPEC = "instances {\n n:int\n check n>0 \"positive\"\n name:word\n}";

  private static List<String> names(Data data) {
    return data.entries().stream().map(Data.Entry::name).toList();
  }

  @Test
  void returnsDataOfTheEntriesInOrder() {
    Data data = Spec.parse(SPEC).catalog("0 a 1 b");
    assertEquals(List.of("instances", "name", "failed"), names(data));
    assertEquals(2, data.get("instances").asLong());
    assertArrayEquals(new String[] {"a", "b"}, data.get("name").strings());
    assertArrayEquals(new String[] {"positive", ""}, data.get("failed").strings());
  }

  @Test
  void leavesOutWhatNoInstanceHas() {
    assertEquals(List.of("instances", "name"), names(Spec.parse(SPEC).catalog("1 a")));
    Data empty = Spec.parse(SPEC).catalog("");
    assertEquals(List.of("instances"), names(empty));
    assertEquals(0, empty.get("instances").asLong());
  }

  @Test
  void countsOneInstanceWithoutABlock() {
    Data data = Spec.parse("let name=\"outside\"\ncheck 1").catalog("");
    assertEquals(List.of("instances"), names(data));
    assertEquals(1, data.get("instances").asLong());
  }

  @Test
  void keepsNoInstanceData() {
    Data data = Spec.parse("let a=1\n" + SPEC).catalog("1 a");
    assertNull(data.find("a"));
    assertNull(data.find("n"));
  }

  @Test
  void writesTheSameDataAsDocuments() {
    Data data = Spec.parse(SPEC).catalog("0 a");
    assertEquals(
        "{\"iris\":\"0.1\",\"entries\":[{\"name\":\"instances\",\"type\":\"int\",\"depth\":0,"
            + "\"value\":1},{\"name\":\"name\",\"type\":\"string\",\"depth\":1,\"value\":[\"a\"]},"
            + "{\"name\":\"failed\",\"type\":\"string\",\"depth\":1,\"value\":[\"positive\"]}]}\n",
        data.toJson());
    assertEquals("instances = 1;\nname = [\"a\"];\nfailed = [\"positive\"];\n", data.toDzn());
  }

  @Test
  void takesTheFormsOfRead(@TempDir Path directory) throws IOException {
    Spec spec = Spec.parse("param p=1\ninstances {\nn:int\ncheck n==p\n}");
    Params params = Params.of("p", "2");
    Path file = Files.writeString(directory.resolve("input"), "2\n");
    byte[] bytes = {'2', '\n'};
    assertEquals("", failed(spec.catalog(bytes, "input", params)));
    assertEquals("", failed(spec.catalog("2\n", "input", params)));
    assertEquals("", failed(spec.catalog(file, params)));
    assertEquals("check on line 4 of the spec", failed(spec.catalog(bytes)));
    assertEquals("check on line 4 of the spec", failed(spec.catalog(bytes, "input")));
    assertEquals("check on line 4 of the spec", failed(spec.catalog("2\n")));
    assertEquals("check on line 4 of the spec", failed(spec.catalog("2\n", "input")));
    assertEquals("check on line 4 of the spec", failed(spec.catalog(file)));
  }

  private static String failed(Data data) {
    Value failed = data.find("failed");
    return failed == null ? "" : failed.get(0).asString();
  }

  @Test
  void namesItsInputInErrors(@TempDir Path directory) throws IOException {
    Spec spec = Spec.parse("x:int");
    Path file = Files.writeString(directory.resolve("input"), "a");
    Location location =
        assertThrows(IrisException.class, () -> spec.catalog(file)).error().inputLocation();
    assertEquals(new Location(file.toString(), 1, 1), location);
    assertNull(
        assertThrows(IrisException.class, () -> spec.catalog("a"))
            .error()
            .inputLocation()
            .origin());
  }

  @Test
  void wrapsTheFailureToReadAFile(@TempDir Path directory) {
    Spec spec = Spec.parse("x:int");
    assertThrows(UncheckedIOException.class, () -> spec.catalog(directory.resolve("missing")));
  }

  @Test
  void refusesTextWithoutAUtf8Form() {
    Spec spec = Spec.parse("x:int");
    assertThrows(IllegalArgumentException.class, () -> spec.catalog("\ud83d"));
    assertThrows(IllegalArgumentException.class, () -> spec.catalog("\ud83d", "input"));
    assertThrows(
        IllegalArgumentException.class, () -> spec.catalog("\ud83d", "input", Params.of()));
  }

  @Test
  void runsDeepSpecsOnALargeStack() {
    String spec = "if 1 {\n".repeat(200) + "x:int\n" + "}\n".repeat(200);
    assertEquals(1, Spec.parse(spec).catalog("1").get("instances").asLong());
  }

  @Test
  void aReadAfterACatalogOfTheSameSpecIsItsOwn() {
    Spec spec = Spec.parse(SPEC);
    spec.catalog("0 a 1 b");
    assertEquals(1, spec.read("1 a").get("n").asLong());
  }
}
