package dev.gecode.iris.driver;

import static java.nio.charset.StandardCharsets.UTF_8;
import static org.junit.jupiter.api.Assertions.assertEquals;

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

  @BeforeEach
  void writeSpecs() throws IOException {
    valid = Files.writeString(directory.resolve("valid.iris"), "n: int\n").toString();
    invalid = Files.writeString(directory.resolve("invalid.iris"), "\n\nn int\n").toString();
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

  @ParameterizedTest
  @ValueSource(strings = {"read", "catalog", "dzn"})
  void reportsAnUnimplementedOperationAsAHostFailure(String operation) {
    assertEquals(new Result(3, ""), run(operation, valid, "input-that-does-not-exist"));
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
    assertEquals(new Result(2, ""), run("catalog", valid, "in", "--format", "xml"));
  }
}
