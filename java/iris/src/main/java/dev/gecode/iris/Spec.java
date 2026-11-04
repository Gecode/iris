package dev.gecode.iris;

import dev.gecode.iris.internal.syntax.Ast;
import dev.gecode.iris.internal.syntax.LargeStack;
import dev.gecode.iris.internal.syntax.Lexer;
import dev.gecode.iris.internal.syntax.Parser;
import dev.gecode.iris.internal.syntax.Resolver;
import dev.gecode.iris.internal.syntax.SpecErrors;
import dev.gecode.iris.internal.syntax.Utf8;
import java.nio.charset.StandardCharsets;
import org.jspecify.annotations.Nullable;

/** A parsed reader spec, checked against every parse-time rule of the Specification. */
public final class Spec {
  final Ast.Program program;

  private Spec(Ast.Program program) {
    this.program = program;
  }

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
    return new Spec(LargeStack.call(() -> parseValid(source, origin)));
  }

  private static Ast.Program parseValid(byte[] source, @Nullable String origin) {
    Ast.Program program = Parser.parse(Lexer.lex(source, origin), origin);
    Resolver.check(program, origin);
    return program;
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
