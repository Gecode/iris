package dev.gecode.iris.build;

import static dev.gecode.iris.build.Iris.*;
import static java.util.Objects.requireNonNull;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.gecode.iris.Data;
import dev.gecode.iris.ErrorCode;
import dev.gecode.iris.IrisException;
import dev.gecode.iris.Spec;
import dev.gecode.iris.build.BuildException.Kind;
import java.util.List;
import java.util.concurrent.atomic.AtomicReference;
import org.jspecify.annotations.Nullable;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.function.Executable;

/**
 * The builder checks of "Testing builders" item 4 in {@code docs/builders.md}: scope discipline,
 * provenance, redefinition, unprintable text, capture binding, and the mapping of errors to the
 * caller's line. Each failure names the line of this file that made the statement.
 */
class ChecksTest {
  /** {@return the line after the caller's} */
  static int next() {
    return StackWalker.getInstance()
            .walk(frames -> frames.skip(1).findFirst().orElseThrow())
            .getLineNumber()
        + 1;
  }

  /** Runs {@code body}, which sets {@code at[0]}, and checks the failure names that line. */
  static BuildException fails(Kind kind, int[] at, Executable body) {
    BuildException failure = assertThrows(BuildException.class, body);
    assertEquals(kind, failure.kind(), failure.getMessage());
    StackTraceElement site = requireNonNull(failure.callSite(), failure.getMessage());
    assertEquals("ChecksTest.java", site.getFileName(), failure.getMessage());
    assertEquals(at[0], site.getLineNumber(), failure.getMessage());
    assertTrue(failure.getMessage().endsWith("call site: " + site), failure.getMessage());
    return failure;
  }

  // Scope

  @Test
  void aStatementOutsideABuilderThrows() {
    var x = int0("x");
    var failure = assertThrows(IllegalStateException.class, () -> read(x, intType()));
    assertTrue(String.valueOf(failure.getMessage()).contains("`read`"), failure.getMessage());
    assertThrows(IllegalStateException.class, () -> forward("y", Int.class));
  }

  @Test
  void aBodyKeptAndRunLaterThrows() throws InterruptedException {
    AtomicReference<@Nullable Runnable> kept = new AtomicReference<>();
    builder(() -> kept.set(() -> check(lit(1))));
    Runnable body = requireNonNull(kept.get());
    assertThrows(IllegalStateException.class, body::run);
    AtomicReference<@Nullable Throwable> thrown = new AtomicReference<>();
    builder(() -> {
      Thread other = new Thread(() -> check(lit(1)));
      other.setUncaughtExceptionHandler((thread, error) -> thrown.set(error));
      other.start();
      try {
        other.join();
      } catch (InterruptedException e) {
        throw new AssertionError(e);
      }
    });
    assertInstanceOf(IllegalStateException.class, thrown.get());
  }

  @Test
  void anArmOutsideSectionsIsRejected() {
    int[] at = new int[1];
    fails(
        Kind.SCOPE,
        at,
        () ->
            spec(() -> {
              at[0] = next();
              arm("x");
            }));
    fails(
        Kind.SCOPE,
        at,
        () ->
            spec(() ->
                sections(() ->
                    arm(
                        "x",
                        () -> {
                          at[0] = next();
                          arm("y");
                        }))));
  }

  @Test
  void aSectionsBodyTakesOnlyArms() {
    int[] at = new int[1];
    var failure =
        fails(
            Kind.SCOPE,
            at,
            () ->
                spec(() ->
                    sections(() -> {
                      arm("a");
                      at[0] = next();
                      check(lit(1));
                    })));
    assertTrue(failure.reason().contains("`check`"), failure.reason());
  }

  @Test
  void aSettingBelongsAtTheTopLevel() {
    int[] at = new int[1];
    fails(
        Kind.SCOPE,
        at,
        () ->
            spec(() ->
                repeat(
                    1,
                    () -> {
                      at[0] = next();
                      comment("#");
                    })));
    fails(
        Kind.SCOPE,
        at,
        () ->
            spec(() ->
                instances(() -> {
                  at[0] = next();
                  instances(() -> skipLine());
                })));
  }

  @Test
  void anElseMustDirectlyFollowAnIfInTheSameBlock() {
    int[] at = new int[1];
    var n = int0("n");
    fails(
        Kind.SCOPE,
        at,
        () ->
            spec(() -> {
              read(n, intType());
              ifThen(n.gt(0), () -> skipLine());
              skipLine();
              at[0] = next();
              elseIf(n.lt(0), () -> skipLine());
            }));
    fails(
        Kind.SCOPE,
        at,
        () ->
            spec(() -> {
              read(n, intType());
              at[0] = next();
              otherwise(() -> skipLine());
            }));
    fails(
        Kind.SCOPE,
        at,
        () ->
            spec(() -> {
              read(n, intType());
              ifThen(
                  n.gt(0),
                  () -> {
                    at[0] = next();
                    otherwise(() -> skipLine());
                  });
            }));
    fails(
        Kind.SCOPE,
        at,
        () ->
            spec(() -> {
              read(n, intType());
              repeat(1, () -> skipLine());
              at[0] = next();
              otherwise(() -> skipLine());
            }));
  }

  @Test
  void aNamedTypeIsVisibleOnlyInItsBlock() {
    int[] at = new int[1];
    AtomicReference<@Nullable IntType> small = new AtomicReference<>();
    var x = int0("x");
    fails(
        Kind.SCOPE,
        at,
        () ->
            spec(() -> {
              repeat(1, () -> small.set(defineType("small", range(0).to(9))));
              at[0] = next();
              read(x, requireNonNull(small.get()));
            }));
  }

  @Test
  void aNamedTypeHidesAnEarlierOneOfTheSameName() {
    int[] at = new int[1];
    var x = int0("x");
    var y = int0("y");
    fails(
        Kind.SCOPE,
        at,
        () ->
            spec(() -> {
              IntType first = defineType("t", atLeast(0));
              read(x, first);
              repeat(
                  1,
                  () -> {
                    defineType("t", atMost(0));
                    at[0] = next();
                    read(y, first);
                  });
            }));
  }

  // Provenance

  @Test
  void aForwardHandleOfAnotherBuilderIsRejected() {
    int[] at = new int[1];
    AtomicReference<@Nullable IntField<Long>> stolen = new AtomicReference<>();
    spec(() -> {
      IntField<Long> x = forward("x", Int.class);
      read(x, intType());
      stolen.set(x);
    });
    IntField<Long> x = requireNonNull(stolen.get());
    var failure =
        fails(
            Kind.PROVENANCE,
            at,
            () ->
                spec(() -> {
                  read(int0("y"), intType());
                  at[0] = next();
                  check(x.gt(0));
                }));
    assertTrue(failure.reason().contains("`x`"), failure.reason());
    fails(
        Kind.PROVENANCE,
        at,
        () ->
            spec(() -> {
              at[0] = next();
              read(x, intType());
            }));
  }

  @Test
  void aReturnedHandleBelongsToItsBuilder() {
    int[] at = new int[1];
    AtomicReference<@Nullable DynField> stolen = new AtomicReference<>();
    spec(() -> stolen.set(read(int0("x"), intType())));
    DynField x = requireNonNull(stolen.get());
    fails(
        Kind.PROVENANCE,
        at,
        () ->
            spec(() -> {
              at[0] = next();
              let(dynamic("y"), x);
            }));
  }

  @Test
  void aNamedTypeOfAnotherBuilderIsRejected() {
    int[] at = new int[1];
    AtomicReference<@Nullable IntType> stolen = new AtomicReference<>();
    spec(() -> stolen.set(defineType("t", atLeast(0))));
    IntType t = requireNonNull(stolen.get());
    fails(
        Kind.PROVENANCE,
        at,
        () ->
            spec(() -> {
              at[0] = next();
              read(int0("x"), t);
            }));
  }

  @Test
  void aDeclaredHandleBelongsToNoBuilder() {
    var x = int0("x");
    Spec first = spec(() -> read(x, intType()));
    Spec second = spec(() -> read(x, atLeast(0)));
    assertEquals(1L, first.read("1").get(x));
    assertEquals(2L, second.read("2").get(x));
  }

  // Redefinition

  @Test
  void twoHandlesCannotNameOneEntry() {
    int[] at = new int[1];
    var failure =
        fails(
            Kind.REDEFINITION,
            at,
            () ->
                spec(() -> {
                  read(int0("x"), intType());
                  at[0] = next();
                  read(float0("x"), floatType());
                }));
    assertTrue(failure.reason().contains("float at depth 0"), failure.reason());
    fails(
        Kind.REDEFINITION,
        at,
        () ->
            spec(() -> {
              read(string0("x"), word());
              at[0] = next();
              read(string0("x").optional(), word());
            }));
    fails(
        Kind.REDEFINITION,
        at,
        () ->
            spec(() -> {
              read(int0("x"), intType());
              at[0] = next();
              forward("x", Int.class);
            }));
    fails(
        Kind.REDEFINITION,
        at,
        () ->
            spec(() -> {
              forward("x", Int.class);
              at[0] = next();
              undefined("x");
            }));
  }

  @Test
  void equalDeclarationsAreOneHandle() {
    Spec spec =
        spec(() -> {
          read(int0("x"), intType());
          check(int0("x").ge(0));
        });
    assertEquals(3L, spec.read("3").get(int0("x")));
  }

  @Test
  void aRedefinitionNeedsABindingThatAlwaysPrecedesIt() {
    int[] at = new int[1];
    var n = int0("n");
    var x = int0("x");
    var y = int0("y");
    fails(
        Kind.BINDING,
        at,
        () ->
            spec(() -> {
              read(n, intType());
              ifThen(n.gt(0), () -> read(x, intType()));
              at[0] = next();
              read(x, floatType());
            }));
    Spec spec =
        spec(() -> {
          read(y, intType());
          read(y, floatType());
        });
    assertEquals(2.5, spec.read("1 2.5").get("y").asDouble());
  }

  @Test
  void anUndefinedNameCannotBeDefined() {
    int[] at = new int[1];
    var failure =
        fails(
            Kind.BINDING,
            at,
            () ->
                spec(() -> {
                  DynField zz = undefined("zz");
                  check(defined(zz).not());
                  at[0] = next();
                  read(zz, intType());
                }));
    assertTrue(failure.reason().contains("`undefined`"), failure.reason());
  }

  // Unprintable text

  @Test
  void aFactoryRejectsANameWithNoSpelling() {
    int[] at = new int[1];
    var failure =
        fails(
            Kind.UNPRINTABLE,
            at,
            () -> {
              at[0] = next();
              int0("_");
            });
    assertTrue(failure.reason().contains("discard()"), failure.reason());
    fails(
        Kind.UNPRINTABLE,
        at,
        () -> {
          at[0] = next();
          string1("gecode.x");
        });
    fails(
        Kind.UNPRINTABLE,
        at,
        () -> {
          at[0] = next();
          dynamic("1x");
        });
    fails(
        Kind.UNPRINTABLE,
        at,
        () -> {
          at[0] = next();
          recordName("job", "two words");
        });
    assertThrows(IllegalArgumentException.class, () -> intN("x", -1));
  }

  @Test
  void aValueWithNoSpellingNamesItsStatement() {
    int[] at = new int[1];
    fails(
        Kind.UNPRINTABLE,
        at,
        () ->
            spec(() -> {
              skipLine();
              at[0] = next();
              check(lit(Double.NaN).gt(0));
            }));
    fails(
        Kind.UNPRINTABLE,
        at,
        () ->
            spec(() -> {
              at[0] = next();
              line("a{b");
            }));
    fails(
        Kind.UNPRINTABLE,
        at,
        () ->
            spec(() -> {
              at[0] = next();
              read(int0("x"), alphabet("ab").base(Long.MIN_VALUE));
            }));
    var n = int0("n");
    fails(
        Kind.UNPRINTABLE,
        at,
        () ->
            spec(() -> {
              read(n, intType());
              ifThen(n.gt(0), () -> skipLine());
              at[0] = next();
              elseReadIf(int0("other"), intType());
            }));
  }

  @Test
  void anOpenRangeCannotComeBeforeFormat() {
    int[] at = new int[1];
    fails(
        Kind.UNPRINTABLE,
        at,
        () ->
            spec(() -> {
              read(int0("x"), atLeast(0));
              at[0] = next();
              format("later");
            }));
  }

  // Capture binding

  @Test
  void aCaptureHasItsHandlesType() {
    int[] at = new int[1];
    var x = int0("x");
    var failure =
        fails(
            Kind.BINDING,
            at,
            () ->
                spec(() -> {
                  at[0] = next();
                  line(capture(x, word()));
                }));
    assertTrue(failure.reason().contains("is int at depth 0"), failure.reason());
    assertTrue(failure.reason().contains("is string at depth 0"), failure.reason());
    fails(
        Kind.BINDING,
        at,
        () ->
            spec(() -> {
              at[0] = next();
              line("x=", x);
            }));
  }

  @Test
  void aCaptureHasItsHandlesDepth() {
    int[] at = new int[1];
    var x = int1("x");
    fails(
        Kind.BINDING,
        at,
        () ->
            spec(() -> {
              at[0] = next();
              line(capture(x, intType()));
            }));
    Spec spec = spec(() -> repeat(2, () -> line(capture(x, intType()))));
    assertEquals(List.of(4L, 5L), toList(spec.read("4\n5\n").get(x)));
  }

  private static List<Long> toList(long[] values) {
    return java.util.Arrays.stream(values).boxed().toList();
  }

  @Test
  void aRequiredHandleNeedsADefinitionThatAlwaysRuns() {
    int[] at = new int[1];
    var name = string0("name");
    var failure =
        fails(
            Kind.BINDING,
            at,
            () ->
                spec(() ->
                    sections(() -> {
                      at[0] = next();
                      arm(pattern("name ", name), optional());
                      arm("end", last());
                    })));
    assertTrue(failure.reason().contains(".optional()"), failure.reason());
    var maybe = string0("name").optional();
    Spec spec =
        spec(() ->
            sections(() -> {
              arm(pattern("name ", maybe), optional());
              arm("end", last());
            }));
    assertNull(spec.read("end\n").get(maybe));
    assertEquals("a", spec.read("name a\nend\n").get(maybe));
  }

  @Test
  void aForwardHandleMustBeDefined() {
    int[] at = new int[1];
    var failure =
        fails(
            Kind.BINDING,
            at,
            () ->
                spec(() -> {
                  at[0] = next();
                  IntField<Long> x = forward("x", Int.class);
                  check(defined(x));
                }));
    assertTrue(failure.reason().contains("never defined"), failure.reason());
    fails(
        Kind.BINDING,
        at,
        () ->
            spec(() -> {
              StringField<String> x = forward("x", Str.class);
              at[0] = next();
              read(x, intType());
            }));
  }

  @Test
  void aDeclaredHandleUsedButNeverDefinedNamesItsFirstUse() {
    int[] at = new int[1];
    var x = int0("x");
    fails(
        Kind.BINDING,
        at,
        () ->
            spec(() -> {
              skipLine();
              at[0] = next();
              check(x.gt(0));
            }));
  }

  @Test
  void onlyAStoreDefinesTheEntryNamedUnderscore() {
    int[] at = new int[1];
    fails(
        Kind.BINDING,
        at,
        () ->
            spec(() -> {
              StringField<String> end = forward("_", Str.class);
              at[0] = next();
              read(end, word());
            }));
    AtomicReference<@Nullable StringField<String>> kept = new AtomicReference<>();
    Spec spec =
        spec(() -> {
          StringField<String> end = forward("_", Str.class);
          kept.set(end);
          repeatUntil(until("end", "stop").store(end), () -> read(int1("x"), intType()));
        });
    Data data = spec.read("1 2 stop");
    assertEquals("stop", data.get(requireNonNull(kept.get())));
  }

  @Test
  void aRecordNameBindsOneRepeat() {
    int[] at = new int[1];
    var job = recordName();
    var failure =
        fails(
            Kind.BINDING,
            at,
            () ->
                spec(() -> {
                  repeat(1, job, () -> skipLine());
                  at[0] = next();
                  repeat(1, job, () -> skipLine());
                }));
    assertTrue(failure.reason().contains("already bound"), failure.reason());
    var x = int1("x");
    fails(
        Kind.BINDING,
        at,
        () ->
            spec(() -> {
              read(x, intType().dim(1));
              at[0] = next();
              check(count(job).eq(1));
            }));
  }

  @Test
  void setKeepsItsHandlesDepth() {
    int[] at = new int[1];
    var x = int0("x");
    var row = int1("row");
    fails(
        Kind.BINDING,
        at,
        () ->
            spec(() -> {
              read(row, intType().dim(2));
              read(x, intType());
              at[0] = next();
              set(x, row);
            }));
    Spec spec =
        spec(() -> {
          read(x, intType());
          set(x, x.plus(1));
        });
    assertEquals(3L, spec.read("2").get(x));
  }

  // Errors of the language

  @Test
  void aLanguageErrorNamesTheStatementThatEmittedTheLine() {
    int[] at = new int[1];
    var outside = int0("outside");
    var job = recordName();
    var failure =
        fails(
            Kind.SPEC,
            at,
            () ->
                spec(() -> {
                  read(outside, intType());
                  repeat(2, job, () -> read(int1("inside"), intType()));
                  at[0] = next();
                  let(dynamic("bad"), job.get(outside));
                }));
    IrisException language = requireNonNull(failure.failure());
    assertEquals(ErrorCode.SPEC_RECORD_SCOPE, language.code());
    assertEquals(6, language.line());
    assertEquals("let bad = record_1.outside", failure.emittedLine());
    assertTrue(requireNonNull(failure.text()).endsWith("let bad = record_1.outside\n"));
    assertTrue(failure.getMessage().startsWith("E-SPEC-RECORD-SCOPE: "), failure.getMessage());
    assertTrue(
        failure.getMessage().contains("\n     6 | let bad = record_1.outside\n"),
        failure.getMessage());
  }

  @Test
  void theLanguageDecidesTheRest() {
    int[] at = new int[1];
    Spec spec =
        spec(() -> {
          DynField x = undefined("x");
          let(dynamic("y"), x);
          read(discard(), intType());
        });
    assertThrows(IrisException.class, () -> spec.read("1"));
    var failure =
        fails(
            Kind.SPEC,
            at,
            () ->
                spec(() -> {
                  instances(() -> skipLine());
                  at[0] = next();
                  read(int0("after"), intType());
                }));
    assertTrue(requireNonNull(failure.text()).contains("after: int"));
  }

  /** A helper's statements name the helper's own lines, as for any other method. */
  @Test
  void aHelpersStatementNamesTheHelpersLine() {
    int[] at = new int[1];
    var failure = fails(Kind.SCOPE, at, () -> spec(() -> helper(at)));
    assertEquals("helper", requireNonNull(failure.callSite()).getMethodName());
  }

  private static void helper(int[] at) {
    at[0] = next();
    arm("x");
  }
}
