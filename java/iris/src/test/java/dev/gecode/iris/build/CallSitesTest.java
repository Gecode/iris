package dev.gecode.iris.build;

import static dev.gecode.iris.build.ChecksTest.next;
import static dev.gecode.iris.build.Iris.*;
import static java.util.Objects.requireNonNull;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.gecode.iris.Data;
import dev.gecode.iris.ErrorCode;
import dev.gecode.iris.IrisException;
import dev.gecode.iris.Location;
import dev.gecode.iris.Spec;
import dev.gecode.iris.build.BuildException.Kind;
import dev.gecode.iris.internal.access.Access;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.ObjectInputStream;
import java.io.ObjectOutputStream;
import java.util.ArrayList;
import java.util.List;
import org.jspecify.annotations.Nullable;
import org.junit.jupiter.api.Test;

/**
 * Errors point at the host code (principle 8 in {@code docs/builders.md}): every statement records
 * the line that called it, and errors from parsing, checking and reading a built spec carry that
 * line beside their location in the emitted text. A port of {@code
 * rust/iris-build/tests/call_sites.rs}; the builder's own checks are in {@link ChecksTest}.
 */
class CallSitesTest {
  private static final String FILE = "CallSitesTest.java";

  private static void assertHere(StackTraceElement site, int line) {
    assertEquals(FILE, site.getFileName(), site.toString());
    assertEquals(line, site.getLineNumber(), site.toString());
  }

  private static void assertHere(IrisException failure, int line) {
    assertHere(failure.callSite().orElseThrow(() -> new AssertionError(failure)), line);
  }

  /** Builds the spec {@code body} writes and returns the language's rejection of its text. */
  private static BuildException rejected(Runnable body) {
    BuildException failure = assertThrows(BuildException.class, () -> spec(body));
    assertEquals(Kind.SPEC, failure.kind(), failure.getMessage());
    return failure;
  }

  private static IrisException language(BuildException failure) {
    return requireNonNull(failure.failure(), failure.getMessage());
  }

  private static IrisException readFails(Spec spec, String input) {
    return assertThrows(IrisException.class, () -> spec.read(input, "input.txt"));
  }

  private static String messageOf(IrisException failure) {
    return requireNonNull(failure.getMessage());
  }

  private static int specLine(IrisException failure) {
    Location location = requireNonNull(failure.error().specLocation(), failure.getMessage());
    return requireNonNull(location.line(), failure.getMessage());
  }

  // Errors of the language

  @Test
  void aParseErrorNamesTheStatementAfterInstances() {
    int[] at = new int[1];
    BuildException failure =
        rejected(() -> {
          instances(() -> {});
          at[0] = next();
          read(int0("after"), intType());
        });
    assertHere(requireNonNull(failure.callSite()), at[0]);
    IrisException language = language(failure);
    assertHere(language, at[0]);
    Location location = requireNonNull(language.error().specLocation());
    assertEquals("builder.iris", location.origin());
    assertEquals(2, location.line());
    assertEquals("after: int", failure.emittedLine());
    assertEquals("instances {}\nafter: int\n", failure.text());
  }

  @Test
  void theMessagesShowTheCodeMessageEmittedLineAndCallSite() {
    int[] at = new int[1];
    BuildException failure =
        rejected(() -> {
          instances(() -> {});
          at[0] = next();
          read(int0("after"), intType());
        });
    IrisException language = language(failure);
    String message = language.error().message();
    String site = requireNonNull(failure.callSite()).toString();
    assertEquals(
        language.code().code() + ": " + message + "\n     2 | after: int\ncall site: " + site,
        failure.getMessage());
    assertEquals("builder.iris:2: " + message + "\ncall site: " + site, language.getMessage());
  }

  @Test
  void aFailedCheckNamesTheCheckInItsBlock() {
    int[] at = new int[1];
    var n = int0("n");
    var x = int1("x");
    Spec spec =
        spec(
            "row",
            () -> {
              read(n, intType());
              repeat(
                  n,
                  () -> {
                    read(x, intType());
                    at[0] = next();
                    check(x.gt(0), "not positive");
                  });
            });
    IrisException failure = readFails(spec, "2 1 -1");
    assertEquals(ErrorCode.CHECK_FAILED, failure.code());
    assertHere(failure, at[0]);
    assertEquals(5, specLine(failure));
    assertTrue(
        messageOf(failure).endsWith("\ncall site: " + failure.callSite().orElseThrow()),
        failure.getMessage());
  }

  @Test
  void aReadFailureNamesTheDeclarationThatRead() {
    int at = next();
    Spec spec = spec(() -> read(int0("n"), intType()));
    assertHere(readFails(spec, "word"), at);
  }

  @Test
  void aCatalogFailureNamesTheStatement() {
    int[] at = new int[1];
    Spec spec =
        spec(() -> {
          at[0] = next();
          read(int0("n"), intType());
          instances(() -> {});
        });
    var failure = assertThrows(IrisException.class, () -> spec.catalog("word", "input.txt"));
    assertHere(failure, at[0]);
  }

  /** A rejection in a catalog is a summary in the data, not an error, so it has no call site. */
  @Test
  void aCatalogRejectionHasNoCallSite() {
    var n = int0("n");
    Spec spec =
        spec(() ->
            instances(() -> {
              read(n, intType());
              check(n.gt(0), "not positive");
            }));
    Data data = spec.catalog("0\n1\n", "input.txt");
    assertEquals(List.of("not positive", ""), data.get("failed").strings());
  }

  @Test
  void aDeferredRejectionKeepsItsCallSite() {
    int[] at = new int[1];
    var n = int0("n");
    Spec spec =
        spec(() ->
            instances(() -> {
              read(n, intType());
              at[0] = next();
              check(n.gt(0));
            }));
    IrisException failure = readFails(spec, "0");
    assertEquals(ErrorCode.CHECK_FAILED, failure.code());
    assertHere(failure, at[0]);
  }

  @Test
  void aFailureWithoutAStatementHasNoCallSite() {
    Spec spec = spec(() -> read(int0("n"), intType()));
    IrisException failure = readFails(spec, "1 2");
    assertEquals(ErrorCode.TRAILING_INPUT, failure.code());
    assertTrue(failure.callSite().isEmpty());
    assertFalse(messageOf(failure).contains("call site"), failure.getMessage());
  }

  /** The same text, parsed and built, differs only in the call site. */
  @Test
  void aSpecParsedFromTextHasNoCallSites() {
    var n = int0("n");
    Builder builder =
        builder(() -> {
          read(n, intType());
          check(n.gt(0), "not positive");
        });
    Spec built = builder.build();
    Spec parsed = Spec.parse(builder.source(), "builder.iris");
    assertNull(Access.callSites(parsed).at(1));
    assertNotNull(Access.callSites(built).at(1));

    IrisException fromText = readFails(parsed, "0");
    IrisException fromBuilder = readFails(built, "0");
    assertTrue(fromText.callSite().isEmpty());
    assertEquals(fromText.error(), fromBuilder.error());
    StackTraceElement site = fromBuilder.callSite().orElseThrow();
    assertEquals(fromText.getMessage() + "\ncall site: " + site, fromBuilder.getMessage());

    var rejected =
        assertThrows(
            IrisException.class, () -> Spec.parse("instances {}\nafter: int\n", "builder.iris"));
    assertTrue(rejected.callSite().isEmpty());
    assertFalse(messageOf(rejected).contains("call site"), rejected.getMessage());
  }

  @Test
  void aCallSiteSurvivesSerialization() throws IOException, ClassNotFoundException {
    Spec spec = spec(() -> read(int0("n"), intType()));
    IrisException failure = readFails(spec, "word");
    var bytes = new ByteArrayOutputStream();
    try (var out = new ObjectOutputStream(bytes)) {
      out.writeObject(failure);
    }
    try (var in = new ObjectInputStream(new ByteArrayInputStream(bytes.toByteArray()))) {
      IrisException copy = assertInstanceOf(IrisException.class, in.readObject());
      assertEquals(failure.callSite(), copy.callSite());
      assertEquals(failure.getMessage(), copy.getMessage());
    }
  }

  /** A helper's statements name the helper's own lines, as for any other method. */
  @Test
  void aHelperRecordsItsOwnLine() {
    int[] at = new int[1];
    Spec spec = spec(() -> readX(at));
    IrisException failure = readFails(spec, "word");
    assertHere(failure, at[0]);
    assertEquals("readX", failure.callSite().orElseThrow().getMethodName());
  }

  private static void readX(int[] at) {
    at[0] = next();
    read(int0("x"), intType());
  }

  // The call site of each statement

  /** The text that statements start with and the lines of their calls, in text order. */
  private static final class Expected {
    private final List<String> starts = new ArrayList<>();
    private final List<Integer> lines = new ArrayList<>();

    /** Expects the statement on the caller's next line to start with {@code start}. */
    void next(String start) {
      starts.add(start);
      lines.add(
          StackWalker.getInstance()
                  .walk(frames -> frames.skip(1).findFirst().orElseThrow())
                  .getLineNumber()
              + 1);
    }

    /** Checks the call site of the line on which each expected statement starts. */
    void check(Builder builder) {
      String text = builder.source();
      Spec spec = builder.build();
      int cursor = 0;
      for (int k = 0; k < starts.size(); k++) {
        String start = starts.get(k);
        int offset = text.indexOf(start, cursor);
        assertTrue(offset >= 0, "no `" + start + "` after offset " + cursor + " of:\n" + text);
        int line = (int) text.substring(0, offset).chars().filter(c -> c == '\n').count() + 1;
        StackTraceElement site = requireNonNull(Access.callSites(spec).at(line), text);
        assertEquals(
            start + " at " + FILE + ":" + lines.get(k),
            start + " at " + site.getFileName() + ":" + site.getLineNumber(),
            text);
        cursor = offset + start.length();
      }
    }
  }

  @Test
  void everySettingAndPlainStatementRecordsItsCall() {
    var e = new Expected();
    var n = int0("n");
    var v = int0("v");
    var m = string0("m");
    Runnable body =
        () -> {
          e.next("format second");
          format("second");
          e.next("comment");
          comment("#");
          e.next("stop");
          stop("END");
          e.next("tokens");
          tokens(":");
          e.next("separators");
          separators(",");
          e.next("ignore trailing");
          ignoreTrailing();
          e.next("param p");
          param(dynamic("p"));
          e.next("param q");
          param(int0("q"), lit(1));
          e.next("type t");
          defineType("t", intType());
          e.next("n: int");
          read(n, intType());
          e.next("list l");
          list(int1("l"), intType().until(0));
          e.next("let v");
          let(v, lit(1));
          e.next("let v");
          set(v, 2);
          e.next("check");
          check(n.ge(0));
          e.next("expect");
          expect("word");
          e.next("skip line");
          skipLine();
          e.next("skip 1 lines");
          skipLines(1);
          e.next("line");
          line("m ", m);
          e.next("find");
          find("x");
          e.next("index");
          index(0);
        };
    e.next("format first");
    Builder builder = builder("first", body);
    e.check(builder);
  }

  @Test
  void everyBlockStatementRecordsItsCall() {
    var e = new Expected();
    var n = int0("n");
    Builder builder =
        builder(() -> {
          e.next("n: int");
          read(n, intType());
          e.next("if n");
          ifThen(
              n.gt(0),
              () -> {
                e.next("a: int");
                read(int0("a").optional(), intType());
              });
          e.next("else if n");
          elseIf(
              n.lt(0),
              () -> {
                e.next("b: int");
                read(int0("b").optional(), intType());
              });
          e.next("else");
          otherwise(() -> {
            e.next("c: int");
            read(int0("c").optional(), intType());
          });
          e.next("repeat 2");
          repeat(
              2,
              () -> {
                e.next("d: int");
                read(int1("d"), intType());
              });
          e.next("repeat until \"END\"");
          repeatUntil(
              until("END"),
              () -> {
                e.next("f: int");
                read(int1("f"), intType());
              });
          e.next("sections");
          sections(() -> {
            e.next("\"one\"");
            arm(
                "one",
                () -> {
                  e.next("h: int");
                  read(int0("h"), intType());
                });
          });
          e.next("repeat until eof");
          repeatUntilEof(() -> {
            e.next("i: float");
            read(float1("i"), floatType());
          });
          e.next("instances");
          instances(() -> {
            e.next("j: int");
            read(int0("j"), intType());
          });
        });
    e.check(builder);
  }

  /**
   * Call sites are mapped by emitted line, so the branches have bodies: an empty block keeps the
   * next branch on its line, and joined statements share the first one's call site.
   */
  @Test
  void theGuardsOfIfRecordTheirCalls() {
    var e = new Expected();
    var n = int0("n");
    Builder builder =
        builder(() -> {
          read(n, intType());
          ifThen(n.gt(0), () -> read(int0("a").optional(), intType()));
          e.next("else if n");
          elseIf(n.lt(0), () -> read(int0("b").optional(), intType()));
          e.next("else if:");
          elseReadIf(int0("if").optional(), intType());
        });
    assertEquals(
        "n: int\nif n > 0 {\n  a: int\n} else if n < 0 {\n  b: int\n} else if: int\n",
        builder.source());
    e.check(builder);
  }

  @Test
  void aClosingBraceBelongsToTheCallThatWroteTheBlock() {
    var e = new Expected();
    Builder builder =
        builder(() -> {
          e.next("}");
          repeat(2, () -> read(int1("x"), intType()));
          read(int0("y"), intType());
        });
    assertEquals("repeat 2 {\n  x: int\n}\ny: int\n", builder.source());
    e.check(builder);
  }

  @Test
  void anEmptyBlockGivesItsBraceToTheCallThatWroteIt() {
    var e = new Expected();
    Builder builder =
        builder(() -> {
          e.next("repeat until");
          repeatUntil(until("END"), () -> {});
        });
    assertEquals("repeat until \"END\" {}\n", builder.source());
    e.check(builder);
  }

  // Errors that lie past their statement

  /**
   * A declaration's form is checked at the token after it, which may start another statement. The
   * location stays there; the call site does not. {@code body} sets {@code at[0]} to the line of
   * the declaration.
   */
  private static void assertOwner(
      int[] at, int emittedLine, @Nullable String emitted, Runnable body) {
    BuildException failure = rejected(body);
    IrisException language = language(failure);
    assertEquals(ErrorCode.SPEC_TYPE, language.code());
    assertEquals("'rest' values cannot be arrays", language.error().message());
    assertHere(requireNonNull(failure.callSite()), at[0]);
    assertHere(language, at[0]);
    assertEquals(emittedLine, specLine(language));
    assertEquals(emitted, failure.emittedLine());

    // The owner is the line of the declaration, in the text alone too.
    String text = requireNonNull(failure.text());
    int owner = Access.ownerLine(language);
    assertTrue(text.lines().toList().get(owner - 1).strip().startsWith("bad[2]: rest"), text);
    var fromText = assertThrows(IrisException.class, () -> Spec.parse(text, "builder.iris"));
    assertTrue(fromText.callSite().isEmpty());
    assertEquals(owner, Access.ownerLine(fromText));
    assertEquals(language.error(), fromText.error());
  }

  @Test
  void anErrorOnTheNextStatementNamesTheStatementItBelongsTo() {
    int[] at = new int[1];
    assertOwner(
        at,
        2,
        "next: int",
        () -> {
          at[0] = next();
          read(string1("bad"), rest().dim(2));
          read(int0("next"), intType());
        });
  }

  @Test
  void anErrorAtTheEndOfANestedBlockNamesTheStatementInIt() {
    int[] at = new int[1];
    assertOwner(
        at,
        4,
        "  }",
        () ->
            repeat(
                1,
                () ->
                    repeat(
                        1,
                        () -> {
                          at[0] = next();
                          read(string3("bad"), rest().dim(2));
                        })));
  }

  @Test
  void anErrorAtTheEndOfAnElseBlockNamesTheStatementInIt() {
    int[] at = new int[1];
    assertOwner(
        at,
        3,
        "}",
        () -> {
          ifThen(lit(1).eq(1), () -> {});
          otherwise(() -> {
            at[0] = next();
            read(string1("bad").optional(), rest().dim(2));
          });
        });
  }

  @Test
  void anErrorAtTheEndOfTheSpecNamesTheLastStatement() {
    int[] at = new int[1];
    assertOwner(
        at,
        3,
        null,
        () -> {
          read(int0("n"), intType());
          at[0] = next();
          read(string1("bad"), rest().dim(2));
        });
  }

  private static void nest(int depth, int[] lines) {
    if (depth == 0) {
      lines[0] = next();
      read(dynamic("x"), intType());
    } else {
      lines[1] = next();
      ifThen(lit(1).eq(1), () -> nest(depth - 1, lines));
    }
  }

  @Test
  void anErrorAfterABraceNamesTheBlockThatOpenedIt() {
    int[] lines = new int[2];
    BuildException failure = rejected(() -> ifThen(lit(1).eq(1), () -> nest(256, lines)));
    assertEquals(ErrorCode.SPEC_NESTING, language(failure).code());
    assertEquals("x: int", requireNonNull(failure.emittedLine()).strip());
    assertHere(requireNonNull(failure.callSite()), lines[1]);
  }

  @Test
  void anErrorInAnArmNamesTheArm() {
    int[] at = new int[1];
    var bad = int0("bad");
    BuildException failure =
        rejected(() ->
            sections(() -> {
              at[0] = next();
              arm(pattern(capture(bad, alphabet("ab"))));
            }));
    assertEquals(ErrorCode.SPEC_PATTERN, language(failure).code());
    assertHere(requireNonNull(failure.callSite()), at[0]);
  }

  @Test
  void aResolutionErrorInAnArmNamesTheArm() {
    int[] at = new int[1];
    var outside = int0("outside");
    var row = recordName();
    var cap = int0("cap");
    BuildException failure =
        rejected(() -> {
          read(outside, intType());
          repeat(2, row, () -> read(int1("inside"), intType()));
          sections(() -> {
            at[0] = next();
            arm(pattern(capture(cap, range(0).below(row.get(outside)))));
          });
        });
    assertEquals(ErrorCode.SPEC_RECORD_SCOPE, language(failure).code());
    assertEquals("sections {", failure.emittedLine());
    assertHere(requireNonNull(failure.callSite()), at[0]);
  }

  @Test
  void aRepeatThatReadsNothingNamesTheRepeat() {
    int at = next();
    Spec spec = spec(() -> repeatUntilEof(() -> let(int1("x"), lit(1))));
    IrisException failure = readFails(spec, "1");
    assertEquals(ErrorCode.NO_PROGRESS, failure.code());
    assertEquals(2, specLine(failure));
    assertHere(failure, at);
  }

  @Test
  void aColumnFailureNamesTheRepeat() {
    int[] at = new int[1];
    var x = int1("x");
    Spec spec =
        spec(() -> {
          at[0] = next();
          repeat(
              2,
              () -> {
                read(x, intType());
                ifThen(x.gt(0), () -> read(dynamic("y"), intType()));
                read(int1("z"), intType());
              });
        });
    IrisException failure = readFails(spec, "1 2 3 0 4");
    assertEquals(ErrorCode.REPEAT_MISSING, failure.code());
    assertEquals(6, specLine(failure));
    assertHere(failure, at[0]);
  }

  @Test
  void aReadFailureInAnArmNamesTheArm() {
    int[] at = new int[1];
    var n = int0("n");
    Spec spec =
        spec(() ->
            sections(() -> {
              at[0] = next();
              arm(pattern("n = ", capture(n, range(1).to(2))));
            }));
    IrisException failure = readFails(spec, "n = 3\n");
    assertEquals(ErrorCode.EXPECTED_VALUE, failure.code());
    assertEquals(1, specLine(failure));
    assertHere(failure, at[0]);
  }

  @Test
  void aLineALastArmLeftUnreadNamesItsArm() {
    int[] at = new int[1];
    Spec spec =
        spec(() ->
            sections(() -> {
              at[0] = next();
              arm("needed");
              arm("end", last(), () -> read(int0("x"), intType()));
            }));
    IrisException failure = readFails(spec, "end\n1\n");
    assertEquals(ErrorCode.LINE_MATCH, failure.code());
    assertEquals(4, specLine(failure));
    assertHere(failure, at[0]);
  }

  @Test
  void aLineNoArmMatchesNamesTheSections() {
    int[] at = new int[1];
    Spec spec =
        spec(() -> {
          at[0] = next();
          sections(() -> {
            arm("first");
            arm("second");
          });
        });
    IrisException failure = readFails(spec, "other\n");
    assertEquals(ErrorCode.SECTIONS_UNEXPECTED, failure.code());
    assertHere(failure, at[0]);
  }

  @Test
  void aFailureAfterSectionsEndHasNoCallSite() {
    Spec spec = spec(() -> sections(() -> arm("end", last())));
    IrisException failure = readFails(spec, "end\nmore");
    assertEquals(ErrorCode.TRAILING_INPUT, failure.code());
    assertTrue(failure.callSite().isEmpty());
  }
}
