package dev.gecode.iris.driver;

import static java.nio.charset.StandardCharsets.UTF_8;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.PrintStream;
import java.nio.file.Files;
import java.nio.file.Path;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.ValueSource;

class DriverTest {
  @TempDir Path directory;
  private String valid = "";
  private String invalid = "";
  private String input = "";

  @BeforeEach
  void writeSpecs() throws IOException {
    valid = Files.writeString(directory.resolve("valid.iris"), "n: int\n").toString();
    invalid = Files.writeString(directory.resolve("invalid.iris"), "\n\nn int\n").toString();
    input = Files.writeString(directory.resolve("input"), "5\n").toString();
  }

  /** The exit status of a request and what it wrote to standard output. */
  private record Result(int status, String document) {}

  private static Result run(String... args) {
    var out = new ByteArrayOutputStream();
    var err = new PrintStream(new ByteArrayOutputStream(), true, UTF_8);
    int status = Driver.run(args, out, err);
    return new Result(status, out.toString(UTF_8));
  }

  @Test
  void answersAValidSpec() {
    assertEquals(new Result(0, "{\"iris\":\"0.1\",\"parsed\":true}\n"), run("parse", valid));
    assertEquals(0, run("parse", valid, "--params", " { } ", "--version", "0.1").status());
  }

  @Test
  void writesTheCanonicalErrorDocument() {
    String document =
        "{\"code\":\"E-SPEC-SYNTAX\",\"message\":\"unknown statement 'n'\",\"input_location\":null,"
            + "\"spec_location\":{\"origin\":\""
            + invalid
            + "\",\"line\":3,\"column\":null},\"context\":null,\"record_path\":null}\n";
    assertEquals(new Result(1, document), run("parse", invalid));
  }

  @Test
  void escapesTheErrorDocument() throws IOException {
    Path spec = directory.resolve("escapes.iris");
    Files.writeString(spec, "line \"{\\t\\\"å\\\"\"\n");
    String document = run("parse", spec.toString()).document();
    assertEquals(true, document.contains("\"message\":\"unterminated capture in pattern"));
    assertEquals(true, document.contains("\\\"{\\t\\\"å\\\"\\\""));
  }

  @Test
  void writesTheDataDocument() {
    String document =
        "{\"iris\":\"0.1\",\"entries\":[{\"name\":\"n\",\"type\":\"int\",\"depth\":0,"
            + "\"value\":5}]}\n";
    assertEquals(new Result(0, document), run("read", valid, input));
  }

  @Test
  void writesTheErrorDocumentOfARead() throws IOException {
    String empty = Files.writeString(directory.resolve("empty"), "").toString();
    Result result = run("read", valid, empty);
    assertEquals(1, result.status());
    assertEquals(true, result.document().startsWith("{\"code\":\"E-"), result.document());
    assertEquals(true, result.document().contains("\"origin\":\"" + empty + "\""));
  }

  @Test
  void passesParamsInOrder() throws IOException {
    Path spec = directory.resolve("params.iris");
    Files.writeString(spec, "param k = 1\nn: int\nlet s = n + k\n");
    String path = spec.toString();
    assertEquals(true, run("read", path, input).document().contains("\"value\":6"));
    Result given = run("read", path, input, "--params", "{\"k\":\"\\u0037\"}");
    assertEquals(true, given.document().contains("\"value\":12"), given.document());
    String unknown = "{\"z\":\"1\",\"a\":\"2\"}";
    Result rejected = run("read", path, input, "--params", unknown);
    assertEquals(1, rejected.status());
    assertEquals(true, rejected.document().contains("E-PARAM-UNKNOWN"));
    assertEquals(true, rejected.document().contains("'z'"), rejected.document());
  }

  @Test
  void refusesParamsThatTheLocaleCannotCarry() {
    Driver.requireIntact("{\"p\":\"é\"}", "UTF-8");
    Driver.requireIntact("{\"p\":\"\\u00e9\"}", "ANSI_X3.4-1968");
    assertThrows(
        IllegalStateException.class, () -> Driver.requireIntact("{\"p\":\"é\"}", "ANSI_X3.4-1968"));
    assertThrows(
        IllegalStateException.class, () -> Driver.requireIntact("{\"p\":\"é\"}", "ISO-8859-1"));
  }

  @ParameterizedTest
  @ValueSource(
      strings = {
        "",
        "[]",
        "{\"a\":1}",
        "{\"a\":\"1\",\"a\":\"2\"}",
        "{\"a\":\"\\ud83d\"}",
        "{\"a\":\"\\ude00\\ud83d\"}",
        "{\"a\":\"1\"} x",
        "{\"a\":\"1\",}",
        "{\"a\":\"\\q\"}",
        "{\"a\":\"\\u００４１\"}",
        "{\"a\":\"\\u٠٠٤١\"}",
      })
  void rejectsMalformedParams(String params) {
    assertEquals(new Result(2, ""), run("read", valid, input, "--params", params));
  }

  @Test
  void reportsAnUnreadableInputAsAHostFailure() {
    assertEquals(new Result(3, ""), run("read", valid, directory.resolve("missing").toString()));
  }

  @Test
  void writesTheCatalogDocument() {
    String document =
        "{\"iris\":\"0.1\",\"entries\":[{\"name\":\"instances\",\"type\":\"int\",\"depth\":0,"
            + "\"value\":1}]}\n";
    assertEquals(new Result(0, document), run("catalog", valid, input));
    assertEquals(new Result(0, document), run("catalog", valid, input, "--format", "json"));
  }

  @Test
  void writesTheCatalogInDzn() {
    assertEquals(
        new Result(0, "instances = 1;\n"), run("catalog", valid, input, "--format", "dzn"));
  }

  @Test
  void writesDzn() {
    assertEquals(new Result(0, "n = 5;\n"), run("dzn", valid, input));
  }

  @ParameterizedTest
  @ValueSource(strings = {"catalog", "dzn"})
  void writesTheErrorDocumentOfAnyOperation(String operation) throws IOException {
    String word = Files.writeString(directory.resolve("word"), "a\n").toString();
    Result result = run(operation, valid, word);
    assertEquals(1, result.status());
    assertEquals(true, result.document().startsWith("{\"code\":\"E-"), result.document());
  }

  @ParameterizedTest
  @ValueSource(strings = {"catalog", "dzn"})
  void reportsAnUnreadableInputOfAnyOperationAsAHostFailure(String operation) {
    String missing = directory.resolve("missing").toString();
    assertEquals(new Result(3, ""), run(operation, valid, missing));
  }

  @ParameterizedTest
  @ValueSource(strings = {"read", "catalog", "dzn"})
  void reportsAnInvalidSpecBeforeAnyOperation(String operation) {
    assertEquals(1, run(operation, invalid, "input-that-does-not-exist").status());
  }

  @Test
  void reportsAnUnreadableSpecAsAHostFailure() {
    assertEquals(new Result(3, ""), run("parse", directory.resolve("missing.iris").toString()));
  }

  @Test
  void rejectsMisuse() {
    assertEquals(new Result(2, ""), run());
    assertEquals(new Result(2, ""), run("parse"));
    assertEquals(new Result(2, ""), run("check", valid));
    assertEquals(new Result(2, ""), run("parse", "--version"));
    assertEquals(new Result(2, ""), run("read", valid));
    assertEquals(new Result(2, ""), run("read", valid, "--version", "0.1"));
    assertEquals(new Result(2, ""), run("parse", valid, "extra"));
    assertEquals(new Result(2, ""), run("parse", valid, "--unknown", "1"));
    assertEquals(new Result(2, ""), run("parse", valid, "--version"));
    assertEquals(new Result(2, ""), run("parse", valid, "--version", "0.2"));
    assertEquals(new Result(2, ""), run("parse", valid, "--version", "0.1", "--version", "0.1"));
    assertEquals(new Result(2, ""), run("parse", valid, "--params", "{\"a\":1}"));
    assertEquals(new Result(2, ""), run("parse", valid, "--format", "json"));
    assertEquals(new Result(2, ""), run("read", valid, "in", "--format", "json"));
    assertEquals(new Result(2, ""), run("dzn", valid, "in", "--format", "dzn"));
    assertEquals(new Result(2, ""), run("catalog", valid, "in", "--format", "xml"));
    assertEquals(new Result(2, ""), run("catalog", valid, "in", "--format"));
  }
}
