package dev.gecode.iris.build;

import static dev.gecode.iris.build.Iris.*;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.gecode.iris.Data;
import dev.gecode.iris.Spec;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.function.Supplier;
import java.util.stream.Stream;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.Arguments;
import org.junit.jupiter.params.provider.MethodSource;

/** The text that each statement and expression prints, and that the language accepts it. */
class PrintingTest {
  static final IntField<Long> x = int0("x");
  static final IntField<Long> y = int0("y");
  static final IntField<Long> z = int0("z");

  /** Checks the text of {@code body} and that it parses. */
  static Builder prints(String expected, Runnable body) {
    Builder builder = builder(body);
    assertEquals(expected, builder.source());
    builder.build();
    return builder;
  }

  /** Checks the text of {@code body}, which the language need not accept. */
  static void text(String expected, Runnable body) {
    assertEquals(expected, builder(body).source());
  }

  /** Checks the text of {@code value} in {@code let v = value}, after reading x, y and z. */
  static void expression(String expected, Supplier<Expr> value) {
    Builder builder =
        builder(() -> {
          read(x, intType());
          read(y, intType());
          read(z, intType());
          let(dynamic("v"), value.get());
        });
    assertEquals("x: int\ny: int\nz: int\nlet v = " + expected + "\n", builder.source());
    builder.build();
  }

  @Test
  void settings() {
    Builder builder =
        builder(
            "alpha-beta.1",
            () -> {
              format("two words");
              comment("#");
              stop("a", "b");
              tokens(",;");
              separators(":");
              ignoreTrailing();
            });
    assertEquals(
        """
        format alpha-beta.1
        format "two words"
        comment "#"
        stop "a" "b"
        tokens ",;"
        separators ":"
        ignore trailing
        """,
        builder.source());
    builder.build();
  }

  static Stream<Arguments> formatNames() {
    return Stream.of(
        Arguments.of("a", "format a\n"),
        Arguments.of("a-b", "format a-b\n"),
        Arguments.of("a.b-2", "format a.b-2\n"),
        Arguments.of("a-1.2", "format \"a-1.2\"\n"),
        Arguments.of("a-1e5", "format \"a-1e5\"\n"),
        Arguments.of("1a", "format \"1a\"\n"),
        Arguments.of("a-", "format \"a-\"\n"),
        Arguments.of("", "format \"\"\n"),
        Arguments.of("a b", "format \"a b\"\n"));
  }

  @ParameterizedTest
  @MethodSource("formatNames")
  void formatNamesAreBareOnlyWhenTheyHaveThatSpelling(String name, String expected) {
    Builder builder = builder(name, () -> {});
    assertEquals(expected, builder.source());
    builder.build();
  }

  @Test
  void declarations() {
    var n = int0("n");
    prints(
        """
        n: int
        a: float
        b: num
        c: word
        d: rest
        e[2][n]: int
        f[2][2]: int layout 1 offset n
        g[2][2]: alphabet "ab" blank " " strip "\\t" base -1
        h[2][n]: alphabet "ab"
        _: int
        """,
        () -> {
          read(n, intType());
          read(float0("a"), floatType());
          read(num0("b"), numType());
          read(string0("c"), word());
          read(string0("d"), rest());
          read(int2("e"), intType().dim(2).dim(n));
          read(int2("f"), intType().dim(2).dim(2).layout(1).offset(n));
          read(int2("g"), alphabet("ab").blank(" ").strip("\t").base(-1).dim(2).dim(2));
          read(int2("h"), alphabet("ab").dims(List.of(lit(2), n)));
          read(discard(), intType());
        });
  }

  @Test
  void aLayoutIsAStringOrAnyExpression() {
    var name = string0("name");
    prints(
        """
        let name = "UPPER_ROW"
        a[2][2]: int layout "LOWER_COL"
        b[2][2]: float layout name
        """,
        () -> {
          let(name, lit("UPPER_ROW"));
          read(int2("a"), intType().dims(2, 2).layout(lit("LOWER_COL")));
          read(float2("b"), floatType().dims(2, 2).layout(name));
        });
  }

  @Test
  void ranges() {
    var n = int0("n");
    prints(
        """
        n: int
        a: 0..<10
        b: 0..10
        c: 0..
        d: ..<10
        e: ..10
        f: 0..<n
        g[n]: 0..<10
        h: n + 1..
        i: gecode.int_min..gecode.int_max
        """,
        () -> {
          read(n, intType());
          read(int0("a"), range(0).below(10));
          read(int0("b"), range(0).to(10));
          read(int0("c"), atLeast(0));
          read(int0("d"), below(10));
          read(int0("e"), atMost(10));
          read(int0("f"), range(0).below(n));
          read(int1("g"), range(0).below(10).dim(n));
          read(int0("h"), atLeast(n.plus(1)));
          read(int0("i"), range(Gecode.intMin).to(Gecode.intMax));
        });
  }

  @Test
  void listsTypesAndParams() {
    var n = int0("n");
    var k = dynamic("k");
    prints(
        """
        n: int
        list a: int until eol
        list b: word until "end"
        list c: float until n
        list _: int until eol
        type small = 0..<n
        d: small
        param k
        param m = 3
        param s = "x"
        """,
        () -> {
          read(n, intType());
          list(int1("a"), intType().untilEol());
          list(string1("b"), word().until("end"));
          list(float1("c"), floatType().until(n));
          list(discard(), intType().untilEol());
          IntType small = defineType("small", range(0).below(n));
          read(int0("d"), small);
          param(k);
          param(int0("m"), lit(3));
          param(string0("s"), lit("x"));
        });
  }

  @Test
  void lineStatements() {
    prints(
        "expect \"a\" \"b\"\nline \"x {} y\"\nfind \"z\"\nskip line\nskip 2 lines\nindex 1\n",
        () -> {
          expect("a", "b");
          line("x ", blank(), " y");
          find("z");
          skipLine();
          skipLines(2);
          index(1);
        });
  }

  @Test
  void patterns() {
    var a = int0("a");
    var b = string0("b");
    prints(
        "line \"{a:0..} {b} {$a} {:int} {_} {_:word} ...\"\n",
        () ->
            line(
                capture(a, atLeast(0)),
                " ",
                b,
                " ",
                ref(a),
                " ",
                validate(intType()),
                " ",
                discard(),
                " ",
                discard(word()),
                " ",
                restOfLine()));
    prints("line \"[{a:int}]\"\n", () -> line(pattern("[", pattern(capture(a, intType()), "]"))));
  }

  @Test
  void repeats() {
    var n = int0("n");
    prints(
        """
        n: int
        repeat n {}
        repeat n {
          a: int
        }
        repeat until eof {
          b: int
        }
        repeat until eof {}
        repeat until "end" {}
        repeat until "x" "y" store stored {
          c: int
        }
        repeat until "z" store _ {}
        """,
        () -> {
          read(n, intType());
          repeat(n, () -> {});
          repeat(n, () -> read(int1("a"), intType()));
          repeatUntilEof(() -> read(int1("b"), intType()));
          repeatUntilEof(() -> {});
          repeatUntil(until("end"), () -> {});
          StringField<String> stored = forward("stored", Str.class);
          repeatUntil(until("x", "y").store(stored), () -> read(int1("c"), intType()));
          repeatUntil(until("z").store(forward("_", Str.class)), () -> {});
        });
  }

  @Test
  void userNamesForRecordsAlwaysPrint() {
    prints(
        "repeat 2 as jobs {\n  each job\n}\n",
        () -> repeat(2, recordName("job", "jobs"), () -> {}));
  }

  @Test
  void aRecordNameServesUseBeforeTheRepeat() {
    var job = recordName("job", "jobs");
    var x = int1("x");
    prints(
        """
        let n = size(jobs)
        let total = sum(element_1 in jobs: element_1.x)
        repeat 2 as jobs {
          each job
          x: int
        }
        """,
        () -> {
          let(int0("n"), size(job));
          let(int0("total"), sum(job, j -> j.get(x)));
          repeat(2, job, () -> read(x, intType()));
        });
  }

  @Test
  void sections() {
    var a = int0("a").optional();
    prints(
        """
        sections {}
        sections until "END" store stored skip other until "x" "y" {
          "plain"
          "a {a:int}" optional
          "b" last
          "c" optional last
          "d" {}
          "e" optional last {
            x: int
          }
        }
        """,
        () -> {
          var stored = string0("stored").optional();
          Iris.sections(() -> {});
          Iris.sections(
              until("END").store(stored),
              skipOther(),
              until("x", "y"),
              () -> {
                arm("plain");
                arm(pattern("a ", capture(a, intType())), optional());
                arm("b", last());
                arm("c", optional(), last());
                arm("d", () -> {});
                arm("e", optional(), last(), () -> read(int0("x").optional(), intType()));
              });
        });
  }

  @Test
  void branches() {
    var n = int0("n");
    var opt = int0("a").optional();
    prints(
        """
        n: int
        if n {}
        if n {
          a: int
        }
        if n {
          b: int
        } else {
          c: int
        }
        if n > 1 {} else if n > 0 {} else {}
        if n {} else if: int
        if n {
          d: int
        } else if n < 0 {
          e: int
        }
        """,
        () -> {
          read(n, intType());
          ifThen(n, () -> {});
          ifThen(n, () -> read(opt, intType()));
          ifThen(n, () -> read(int0("b").optional(), intType()));
          otherwise(() -> read(int0("c").optional(), intType()));
          ifThen(n.gt(1), () -> {});
          elseIf(n.gt(0), () -> {});
          otherwise(() -> {});
          DynField branch = forward("if", Dyn.class);
          ifThen(n, () -> {});
          elseReadIf(branch, intType());
          ifThen(n, () -> read(int0("d").optional(), intType()));
          elseIf(n.lt(0), () -> read(int0("e").optional(), intType()));
        });
  }

  @Test
  void letAndCheck() {
    var v = int0("v");
    prints(
        "let v = 1\nlet v = 2\ncheck v\ncheck v > 0 \"a \\\"quoted\\\" message\"\n",
        () -> {
          let(v, lit(1));
          set(v, 2);
          check(v);
          check(v.gt(0), "a \"quoted\" message");
        });
  }

  @Test
  void instancesAndALaterFormat() {
    Builder builder =
        builder(
            "one",
            () -> {
              instances(() -> read(int0("x"), intType()));
              format("two");
            });
    assertEquals("format one\ninstances {\n  x: int\n}\nformat two\n", builder.source());
    builder.build();
  }

  static Stream<Arguments> literals() {
    return Stream.of(
        Arguments.of(lit(1), "1"),
        Arguments.of(lit(-1), "-1"),
        Arguments.of(lit(1.0), "1.0"),
        Arguments.of(lit(-1.5), "-1.5"),
        Arguments.of(lit(1e300), "1e300"),
        Arguments.of(lit(9007199254740992.0), "9007199254740992.0"),
        Arguments.of(lit(1e16), "1e16"),
        Arguments.of(lit(1e-5), "1e-5"),
        Arguments.of(lit(0.0001), "0.0001"),
        Arguments.of(lit(-0.0), "-0.0"),
        Arguments.of(lit(-Double.MAX_VALUE), "-1.7976931348623157e308"),
        Arguments.of(lit(1e23), "1e23"),
        Arguments.of(lit(2e22), "2e22"),
        Arguments.of(lit(1.0e15), "1000000000000000.0"),
        Arguments.of(lit(0.1), "0.1"),
        Arguments.of(lit(123456.789e3), "123456789.0"),
        Arguments.of(lit("a\tb"), "\"a\\tb\""),
        Arguments.of(lit("s"), "\"s\""),
        Arguments.of(lit(4294967295L), "4294967295"),
        Arguments.of(lit(Long.MIN_VALUE), "(-9223372036854775807 - 1)"),
        Arguments.of(lit(Long.MAX_VALUE), "9223372036854775807"));
  }

  @ParameterizedTest
  @MethodSource("literals")
  void literals(Expr value, String expected) {
    expression(expected, () -> value);
  }

  /** Java prints at least two digits, Rust the fewest; the language rejects subnormals. */
  @Test
  void aSubnormalPrintsItsShortestDigits() {
    text("let v = 5e-324\n", () -> let(float0("v"), lit(Double.MIN_VALUE)));
    text("let v = 1e-323\n", () -> let(float0("v"), lit(2 * Double.MIN_VALUE)));
    var failure =
        assertThrows(
            BuildException.class, () -> spec(() -> let(float0("v"), lit(Double.MIN_VALUE))));
    assertEquals(BuildException.Kind.SPEC, failure.kind());
  }

  @Test
  void hostConstants() {
    expression("gecode.int_max + 0", () -> Gecode.intMax.plus(0));
    expression("-gecode.int_min", () -> Gecode.intMin.neg());
  }

  static Stream<Arguments> precedence() {
    return Stream.of(
        Arguments.of(x.plus(y).plus(z), "x + y + z"),
        Arguments.of(x.plus(y.plus(z)), "x + (y + z)"),
        Arguments.of(x.minus(y.minus(z)), "x - (y - z)"),
        Arguments.of(x.times(y).plus(z), "x * y + z"),
        Arguments.of(x.times(y.plus(z)), "x * (y + z)"),
        Arguments.of(x.plus(y).div(z), "(x + y) / z"),
        Arguments.of(x.mod(y.times(z)), "x % (y * z)"),
        Arguments.of(x.lt(y.lt(z)), "x < (y < z)"),
        Arguments.of(x.lt(y).lt(z), "x < y < z"),
        Arguments.of(x.eq(y).ne(z), "x == y != z"),
        Arguments.of(x.eq(y.ne(z)), "x == (y != z)"),
        Arguments.of(x.le(y).ge(z).gt(1), "x <= y >= z > 1"),
        Arguments.of(x.lt(y).eq(z), "x < y == z"),
        Arguments.of(x.and(y).or(z), "x && y || z"),
        Arguments.of(x.and(y.or(z)), "x && (y || z)"),
        Arguments.of(x.and(y).and(z), "x && y && z"),
        Arguments.of(x.and(y.and(z)), "x && (y && z)"),
        Arguments.of(x.and(y).not(), "!(x && y)"),
        Arguments.of(x.not().and(y), "!x && y"),
        Arguments.of(x.plus(y).neg(), "-(x + y)"),
        Arguments.of(x.neg().neg(), "--x"),
        Arguments.of(x.not().not(), "!!x"),
        Arguments.of(x.not().neg(), "-!x"),
        Arguments.of(x.minus(y.neg()), "x - -y"),
        Arguments.of(size(x).eq(y), "size(x) == y"),
        Arguments.of(lit(2).pow(2).neg(), "-2 ^ 2"),
        Arguments.of(lit(2).neg().pow(2), "(-2) ^ 2"),
        Arguments.of(lit(-2).pow(2), "(-2) ^ 2"),
        Arguments.of(lit(2).pow(lit(3).pow(4)), "2 ^ 3 ^ 4"),
        Arguments.of(lit(2).pow(3).pow(4), "(2 ^ 3) ^ 4"),
        Arguments.of(lit(2).pow(x.neg()), "2 ^ -x"),
        Arguments.of(x.plus(y).pow(2), "(x + y) ^ 2"),
        Arguments.of(lit(2).pow(x.plus(y)), "2 ^ (x + y)"),
        Arguments.of(x.times(y.pow(z)), "x * y ^ z"),
        Arguments.of(lit(1).plus(x), "1 + x"),
        Arguments.of(lit(7).div(x), "7 / x"),
        Arguments.of(lit(1).plus(Gecode.intMax).minus(x), "1 + gecode.int_max - x"),
        Arguments.of(lit(1.5).lt(x), "1.5 < x"),
        Arguments.of(lit("a").eq("b"), "\"a\" == \"b\""));
  }

  @ParameterizedTest
  @MethodSource("precedence")
  void parenthesesFollowThePrecedenceLevels(Expr value, String expected) {
    expression(expected, () -> value);
  }

  @Test
  void callsAndComprehensions() {
    var a = int1("a");
    prints(
        """
        a[2]: int
        let v1 = size(a)
        let v2 = count(a)
        let v3 = min(a)
        let v4 = max(a, 1)
        let v5 = sum(1, 2, 3)
        let v6 = distinct(1, 2)
        let v7 = defined(a)
        let v8 = all(element_1 in a: element_1 > 0)
        let v9 = any(element_2 in a: element_2 > 0)
        let v10 = sum(element_3 in a: element_3)
        let v11 = count(item in a: item > 1)
        let v12 = sum(element_4 in a: sum(element_5 in a: element_4 * element_5))
        let v13 = a[0] + a[1]
        """,
        () -> {
          read(a, intType().dim(2));
          let(int0("v1"), size(a));
          let(int0("v2"), count(a));
          let(dynamic("v3"), min(a));
          let(dynamic("v4"), max(a, lit(1)));
          let(int0("v5"), sum(lit(1), lit(2), lit(3)));
          let(int0("v6"), distinct(lit(1), lit(2)));
          let(int0("v7"), defined(a));
          let(int0("v8"), all(a, e -> e.gt(0)));
          let(int0("v9"), any(a, e -> e.gt(0)));
          let(int0("v10"), sum(a, e -> e));
          let(int0("v11"), count(over(a).named("item"), e -> e.gt(1)));
          let(int0("v12"), sum(a, e -> sum(a, f -> e.times(f))));
          let(int0("v13"), a.at(0).plus(a.at(1)));
        });
  }

  @Test
  void internalNamesSkipEveryNameInTheSpec() {
    var a = int1("element_1");
    var x = int1("x");
    var job = recordName();
    prints(
        """
        record_1: int
        records_1: int
        element_1[1]: int
        repeat 1 as records_2 {
          each record_2
          x: int
        }
        let v = sum(element_2 in records_2: element_2.x)
        check record_2.x
        let u = all(element_3 in element_1: element_3)
        """,
        () -> {
          read(int0("record_1"), intType());
          read(int0("records_1"), intType());
          read(a, intType().dim(1));
          repeat(1, job, () -> read(x, intType()));
          let(int0("v"), sum(job, j -> j.get(x)));
          check(job.get(x));
          let(int0("u"), all(a, e -> e));
        });
  }

  @Test
  void recordsProjectAndIndex() {
    var outer = recordName();
    var inner = recordName();
    var x = int2("x");
    prints(
        """
        repeat 2 as records_1 {
          each record_1
          repeat 2 as records_2 {
            x: int
          }
        }
        let a = records_1[0].x[1]
        check size(record_1.records_2)
        let c = records_1[1].records_2[0].x
        let d = size(records_1)
        check defined(records_1)
        let f = sum(element_1 in records_1: sum(element_2 in element_1.records_2: element_2.x))
        """,
        () -> {
          repeat(2, outer, () -> repeat(2, inner, () -> read(x, intType())));
          let(dynamic("a"), outer.at(0).get(x).at(1));
          check(size(outer.records(inner)));
          let(dynamic("c"), outer.at(1).records(inner).at(0).get(x));
          let(int0("d"), size(outer.list()));
          check(defined(outer));
          let(dynamic("f"), sum(outer, o -> sum(o.records(inner), i -> i.get(x))));
        });
  }

  @Test
  void aMemberNeverGluesToALiteralOrTheNameGecode() {
    text(
        "x: int\nlet a = (1).x\nlet b = gecode[0].x\nlet c = gecode . x\nlet e = (1 + 2).x\n",
        () -> {
          read(x, intType());
          DynField g = undefined("gecode");
          let(dynamic("a"), lit(1).dynamic().get(x));
          let(dynamic("b"), g.at(0).get(x));
          let(dynamic("c"), g.get(x));
          let(dynamic("e"), lit(1).plus(2).dynamic().get(x));
        });
  }

  @Test
  void keywordsAtTheStartOfAnExpressionTakeParentheses() {
    var until = int0("until");
    var line = int0("line");
    var eol = int0("eol");
    var layout = int0("layout");
    prints(
        """
        until: int
        line: int
        eol: int
        layout: int
        repeat (until) {}
        skip (line * 2) lines
        list a: int until (eol + 1)
        c: 0..(layout)
        line "{:0..(layout)}"
        """,
        () -> {
          read(until, intType());
          read(line, intType());
          read(eol, intType());
          read(layout, intType());
          repeat(until, () -> {});
          skipLines(line.times(2));
          list(int1("a"), intType().until(eol.plus(1)));
          read(int0("c"), range(0).to(layout));
          Iris.line(validate(range(0).to(layout)));
        });
  }

  @Test
  void numEntriesReadAsNumbers() {
    var b = num1("b");
    Spec spec = prints("b[3]: num\n", () -> read(b, numType().dim(3))).build();
    assertEquals(List.of(1L, 2L, -3L), spec.read("1 2 -3").get(b));
    assertEquals(List.of(1.0, 2.5, -3.0), spec.read("1 2.5 -3").get(b));
  }

  @Test
  void aDeepRightChainPrintsWithoutRecursion() {
    IntExpr chain = lit(0);
    for (int k = 0; k < 20_000; k++) {
      chain = lit(1).minus(chain);
    }
    IntExpr value = chain;
    String text = builder(() -> let(int0("v"), value)).source();
    assertTrue(text.startsWith("let v = 1 - (1 - (1 - "), text.substring(0, 40));
  }

  @Test
  void theSameProgramPrintsTheSameText() {
    assertEquals(JobshopTest.builder.source(), JobshopTest.builder.source());
    assertEquals(builder(PrintingTest::program).source(), builder(PrintingTest::program).source());
  }

  @Test
  void concurrentBuildersPrintTheSameText() throws InterruptedException, ExecutionException {
    String expected = builder(PrintingTest::program).source();
    List<Future<String>> results = new ArrayList<>();
    try (ExecutorService pool = Executors.newFixedThreadPool(8)) {
      for (int k = 0; k < 32; k++) {
        results.add(pool.submit(() -> builder(PrintingTest::program).source()));
      }
      for (Future<String> result : results) {
        assertEquals(expected, result.get());
      }
    }
  }

  private static void program() {
    var job = recordName();
    var a = int1("a");
    repeat(2, job, () -> read(a, intType()));
    check(all(job, j -> j.get(a).ge(0)));
    check(sum(a, e -> e).le(10));
  }

  @Test
  void aBuiltSpecReads() {
    Spec spec = JobshopTest.builder.build();
    Data data = spec.read("2 1\n0 5\n0 6\n");
    assertEquals(2L, data.get(JobshopTest.nJobs));
  }
}
