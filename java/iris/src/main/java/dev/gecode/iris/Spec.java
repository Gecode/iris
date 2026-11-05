package dev.gecode.iris;

import dev.gecode.iris.internal.read.Interpreter;
import dev.gecode.iris.internal.syntax.Ast;
import dev.gecode.iris.internal.syntax.LargeStack;
import dev.gecode.iris.internal.syntax.Lexer;
import dev.gecode.iris.internal.syntax.Parser;
import dev.gecode.iris.internal.syntax.Resolution;
import dev.gecode.iris.internal.syntax.Resolver;
import dev.gecode.iris.internal.syntax.SpecErrors;
import dev.gecode.iris.internal.syntax.Utf8;
import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import org.jspecify.annotations.Nullable;

/** A parsed reader spec, checked against every parse-time rule of the Specification. */
public final class Spec {
  private final Parsed parsed;
  private final @Nullable String origin;

  private Spec(Parsed parsed, @Nullable String origin) {
    this.parsed = parsed;
    this.origin = origin;
  }

  private record Parsed(Ast.Program program, Resolution resolution) {}

  /**
   * Parses a reader spec without an origin.
   *
   * @param source the UTF-8 bytes of the reader spec
   * @return the parsed reader spec
   * @throws IrisException for the first parse-time error
   */
  public static Spec parse(byte[] source) {
    return parseBytes(source, null);
  }

  /**
   * Parses a reader spec.
   *
   * @param source the UTF-8 bytes of the reader spec
   * @param origin the name of the source in errors
   * @return the parsed reader spec
   * @throws IrisException for the first parse-time error
   */
  public static Spec parse(byte[] source, String origin) {
    return parseBytes(source, origin);
  }

  /**
   * Parses a reader spec without an origin.
   *
   * @param source the text of the reader spec
   * @return the parsed reader spec
   * @throws IrisException for the first parse-time error
   */
  public static Spec parse(String source) {
    return parseText(source, null);
  }

  /**
   * Parses a reader spec.
   *
   * @param source the text of the reader spec
   * @param origin the name of the source in errors
   * @return the parsed reader spec
   * @throws IrisException for the first parse-time error
   */
  public static Spec parse(String source, String origin) {
    return parseText(source, origin);
  }

  private static Spec parseBytes(byte[] source, @Nullable String origin) {
    int invalid = Utf8.firstInvalid(source);
    if (invalid >= 0) {
      throw SpecErrors.invalidUtf8(source, invalid, origin);
    }
    return new Spec(LargeStack.call(() -> parseValid(source, origin)), origin);
  }

  private static Parsed parseValid(byte[] source, @Nullable String origin) {
    Ast.Program program = Parser.parse(Lexer.lex(source, origin), origin);
    return new Parsed(program, Resolver.resolve(program, origin));
  }

  /** Returns the tree of the reader spec, for the tests of this package. */
  Ast.Program program() {
    return parsed.program();
  }

  /** Returns the name of the format that the reader spec reads, or null if it names none. */
  public @Nullable String format() {
    return parsed.program().format();
  }

  /**
   * Reads an input without an origin or parameters.
   *
   * @param input the bytes of the input
   * @return the data read
   * @throws IrisException for the first error
   */
  public Data read(byte[] input) {
    return readBytes(input, null, Params.of());
  }

  /**
   * Reads an input without parameters.
   *
   * @param input the bytes of the input
   * @param origin the name of the input in errors
   * @return the data read
   * @throws IrisException for the first error
   */
  public Data read(byte[] input, String origin) {
    return readBytes(input, origin, Params.of());
  }

  /**
   * Reads an input.
   *
   * @param input the bytes of the input
   * @param origin the name of the input in errors
   * @param params the parameters
   * @return the data read
   * @throws IrisException for the first error
   */
  public Data read(byte[] input, String origin, Params params) {
    return readBytes(input, origin, params);
  }

  /**
   * Reads an input without an origin or parameters.
   *
   * @param input the text of the input
   * @return the data read
   * @throws IrisException for the first error
   * @throws IllegalArgumentException if the text has an unpaired surrogate, which has no UTF-8 form
   */
  public Data read(String input) {
    return readBytes(encode(input), null, Params.of());
  }

  /**
   * Reads an input without parameters.
   *
   * @param input the text of the input
   * @param origin the name of the input in errors
   * @return the data read
   * @throws IrisException for the first error
   * @throws IllegalArgumentException if the text has an unpaired surrogate, which has no UTF-8 form
   */
  public Data read(String input, String origin) {
    return readBytes(encode(input), origin, Params.of());
  }

  /**
   * Reads an input.
   *
   * @param input the text of the input
   * @param origin the name of the input in errors
   * @param params the parameters
   * @return the data read
   * @throws IrisException for the first error
   * @throws IllegalArgumentException if the text has an unpaired surrogate, which has no UTF-8 form
   */
  public Data read(String input, String origin, Params params) {
    return readBytes(encode(input), origin, params);
  }

  /**
   * Reads a file without parameters. Its path is the origin of the input.
   *
   * @param input the file
   * @return the data read
   * @throws IrisException for the first error
   * @throws UncheckedIOException if the file cannot be read
   */
  public Data read(Path input) {
    return read(input, Params.of());
  }

  /**
   * Reads a file. Its path is the origin of the input.
   *
   * @param input the file
   * @param params the parameters
   * @return the data read
   * @throws IrisException for the first error
   * @throws UncheckedIOException if the file cannot be read
   */
  public Data read(Path input, Params params) {
    try {
      return readBytes(Files.readAllBytes(input), input.toString(), params);
    } catch (IOException e) {
      throw new UncheckedIOException(e);
    }
  }

  private Data readBytes(byte[] input, @Nullable String inputOrigin, Params params) {
    Interpreter interpreter =
        new Interpreter(
            parsed.program(), parsed.resolution(), origin, input, inputOrigin, params.values());
    return new Data(LargeStack.call(interpreter::run));
  }

  private static byte[] encode(String input) {
    int surrogate = firstUnpairedSurrogate(input);
    if (surrogate >= 0) {
      throw new IllegalArgumentException("unpaired surrogate at index " + surrogate);
    }
    return input.getBytes(StandardCharsets.UTF_8);
  }

  /**
   * Parses text whose encoding would hide an unpaired surrogate: {@link String#getBytes} replaces
   * one with {@code ?}.
   */
  private static Spec parseText(String source, @Nullable String origin) {
    int surrogate = firstUnpairedSurrogate(source);
    if (surrogate < 0) {
      return parseBytes(source.getBytes(StandardCharsets.UTF_8), origin);
    }
    byte[] prefix = source.substring(0, surrogate).getBytes(StandardCharsets.UTF_8);
    throw SpecErrors.invalidUtf8(prefix, prefix.length, origin);
  }

  /** Returns the index of the first surrogate in {@code text} without its pair, or -1. */
  private static int firstUnpairedSurrogate(String text) {
    int index = 0;
    while (index < text.length()) {
      int codePoint = text.codePointAt(index);
      if (codePoint >= Character.MIN_SURROGATE && codePoint <= Character.MAX_SURROGATE) {
        return index;
      }
      index += Character.charCount(codePoint);
    }
    return -1;
  }
}
