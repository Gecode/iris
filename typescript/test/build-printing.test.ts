// The printed text: every grammar production, the parenthesis and lookahead
// tables, and specs far larger than a program would write by hand. The
// expected texts are those of python/tests/test_build_printing.py, which
// checks each with `iris fmt`.
import assert from "node:assert/strict";
import { test } from "node:test";
import {
  all,
  alphabet,
  any,
  atLeast,
  atMost,
  below,
  bind,
  blank,
  count,
  defined,
  discard,
  discardAs,
  distinct,
  div,
  eol,
  float,
  floatLit,
  gecode,
  int,
  lit,
  max,
  min,
  num,
  over,
  p,
  pattern,
  pow,
  range,
  ref,
  rem,
  rest,
  restOfLine,
  size,
  SpecBuilder,
  sum,
  validate,
  word,
  type Expr,
  type Field,
  type HandleDepth,
  type Presence,
  type Scope,
} from "gecode-iris/build";

type Int = Field<"int", 0, "required">;

function assertText(build: (b: SpecBuilder) => unknown, expected: string): SpecBuilder {
  const b = new SpecBuilder();
  build(b);
  assert.equal(b.source(), expected);
  return b;
}

function assertPrints(build: (b: SpecBuilder) => unknown, expected: string): void {
  assertText(build, expected).build();
}

/** `let v = expression`, with `x`, `y` and `z` defined first. */
function assertExpression(build: (x: Int, y: Int, z: Int) => Expr, expected: string): void {
  assertPrints((b) => {
    const x = b.read("x", int);
    const y = b.read("y", int);
    const z = b.read("z", int);
    b.let("v", build(x, y, z));
  }, `x: int\ny: int\nz: int\nlet v = ${expected}\n`);
}

test("settings", () => {
  const b = new SpecBuilder("alpha-beta.1");
  b.format("two words");
  b.comment("#");
  b.stop("a", "b");
  b.tokens(",;");
  b.separators(":");
  b.ignoreTrailing();
  assert.equal(
    b.source(),
    'format alpha-beta.1\nformat "two words"\ncomment "#"\nstop "a" "b"\n' +
      'tokens ",;"\nseparators ":"\nignore trailing\n',
  );
  b.build();
});

test("format names are bare only when they have that spelling", () => {
  const cases: [string, string][] = [
    ["a", "format a\n"],
    ["a-b", "format a-b\n"],
    ["a.b-2", "format a.b-2\n"],
    ["a-1.2", 'format "a-1.2"\n'],
    ["a-1e5", 'format "a-1e5"\n'],
    ["1a", 'format "1a"\n'],
    ["a-", 'format "a-"\n'],
    ["", 'format ""\n'],
    ["a b", 'format "a b"\n'],
  ];
  for (const [name, expected] of cases) {
    const b = new SpecBuilder(name);
    assert.equal(b.source(), expected);
    b.build();
  }
});

test("declarations", () => {
  assertPrints(
    (b) => {
      const n = b.read("n", int);
      b.read("a", float);
      b.read("b", num);
      b.read("c", word);
      b.read("d", rest);
      b.read("e", int.dim(2).dim(n));
      b.read("f", int.dim(2).dim(2).layout(1).offset(n));
      b.read("g", alphabet("ab", { blank: " ", strip: "\t", base: -1 }).dim(2).dim(2));
      b.read("h", alphabet("ab").dims(2, n));
      b.read(discard(), int);
    },
    "n: int\na: float\nb: num\nc: word\nd: rest\ne[2][n]: int\n" +
      "f[2][2]: int layout 1 offset n\n" +
      'g[2][2]: alphabet "ab" blank " " strip "\\t" base -1\n' +
      'h[2][n]: alphabet "ab"\n_: int\n',
  );
});

test("a layout is a string or any expression", () => {
  assertPrints((b) => {
    const name = b.let("name", "UPPER_ROW");
    b.read("a", int.dim(2).dim(2).layout("LOWER_COL"));
    b.read("b", float.dim(2).dim(2).layout(name));
  }, 'let name = "UPPER_ROW"\na[2][2]: int layout "LOWER_COL"\nb[2][2]: float layout name\n');
});

test("ranges", () => {
  assertPrints((b) => {
    const n = b.read("n", int);
    b.read("a", range(0).below(10));
    b.read("b", range(0, 10));
    b.read("c", atLeast(0));
    b.read("d", below(10));
    b.read("e", atMost(10));
    b.read("f", range(0).below(n));
    b.read("g", range(0).below(10).dim(n));
    b.read("h", atLeast(n.plus(1).dynamic()));
    b.read("i", range(gecode.intMin, gecode.intMax));
  }, "n: int\na: 0..<10\nb: 0..10\nc: 0..\nd: ..<10\ne: ..10\nf: 0..<n\ng[n]: 0..<10\nh: n + 1..\ni: gecode.int_min..gecode.int_max\n");
});

test("lists, types and params", () => {
  assertPrints((b) => {
    b.list("a", int.until(0));
    b.list("b", word.until("end"));
    b.list("c", int.until(eol).offset(1));
    const t = b.defineType("t", atLeast(0));
    b.read("x", t);
    const u = b.defineType("u", t);
    b.list("d", u.until(1));
    b.param("p");
    b.param("q", 3);
  }, 'list a: int until 0\nlist b: word until "end"\nlist c: int until eol offset 1\ntype t = 0..\nx: t\ntype u = t\nlist d: u until 1\nparam p\nparam q = 3\n');
});

test("line statements", () => {
  assertPrints((b) => {
    const n = b.read("n", int);
    b.line("plain");
    b.find('a "quoted" \\ line\n');
    b.skipLine();
    b.skipLines(n);
    b.index(n.plus(1));
    b.expect("x");
    b.expect("y", "z");
  }, 'n: int\nline "plain"\nfind "a \\"quoted\\" \\\\ line\\n"\nskip line\nskip n lines\nindex n + 1\nexpect "x"\nexpect "y" "z"\n');
});

test("patterns", () => {
  assertPrints(
    (b) => {
      const x = b.capture("x", int);
      const y = b.capture("y");
      const z = b.capture("z", atLeast(1));
      const w = b.forward("w", "string");
      b.line(p`a ${x} b ${y}`);
      b.line(pattern(z));
      b.line(pattern(ref(x)));
      b.line(pattern(validate(float)));
      b.line(p`${blank()} ${discard()}`);
      b.line(pattern(discardAs(word)));
      b.line(pattern(bind(w, word)));
      b.line(p`tail ${restOfLine}`);
      b.line(pattern("a", restOfLine, "  "));
      b.line("lone } brace");
    },
    'line "a {x:int} b {y}"\nline "{z:1..}"\nline "{$x}"\nline "{:float}"\n' +
      'line "{} {_}"\nline "{_:word}"\nline "{w:word}"\nline "tail ..."\n' +
      'line "a...  "\nline "lone } brace"\n',
  );
});

test("tagged templates and pattern parts print alike", () => {
  const b = new SpecBuilder();
  const x = b.capture("x", int);
  b.line(pattern("x = ", x, ", ", blank(), ""));
  b.line(p`x = ${ref(x)}, ${blank()}`);
  assert.equal(b.source(), 'line "x = {x:int}, {}"\nline "x = {$x}, {}"\n');
});

test("repeats", () => {
  assertPrints(
    (b) => {
      const n = b.read("n", int);
      b.repeat(n, () => {});
      b.repeat(n, (r) => {
        r.read("a", int);
      });
      b.repeatUntilEof((r) => {
        r.read("b", int);
      });
      b.repeatUntilEof();
      b.repeatUntil("end");
      const stored = b.forward("stored", "string");
      b.repeatUntil(["x", "y"], { store: stored }, (r) => {
        r.read("c", int);
      });
      b.repeatUntil("z", { store: b.forward("_", "string") });
    },
    "n: int\nrepeat n {}\nrepeat n {\n  a: int\n}\nrepeat until eof {\n  b: int\n}\n" +
      'repeat until eof {}\nrepeat until "end" {}\nrepeat until "x" "y" store stored {\n' +
      '  c: int\n}\nrepeat until "z" store _ {}\n',
  );
});

test("user names for records always print", () => {
  assertPrints((b) => {
    b.repeat(2, (r) => {
      r.eachName("job");
      r.asName("jobs");
    });
  }, "repeat 2 as jobs {\n  each job\n}\n");
});

test("a forward record serves use before the repeat", () => {
  assertPrints((b) => {
    const job = b.forwardRecord("job", "jobs");
    const x = b.forward("x", "int", { depth: 1 });
    b.let("n", size(job));
    b.let(
      "total",
      sum(job, (j) => j.get(x)),
    );
    b.repeat(2, (r) => {
      r.recordAs(job);
      r.read(x, int);
    });
  }, "let n = size(jobs)\nlet total = sum(element_1 in jobs: element_1.x)\nrepeat 2 as jobs {\n  each job\n  x: int\n}\n");
});

test("sections", () => {
  assertPrints(
    (b) => {
      const stored = b.forward("stored", "string", { optional: true });
      const a = b.optionalCapture("a", int);
      b.sections();
      b.sections(
        [{ until: "END", store: stored }, { skipOther: true }, { until: ["x", "y"] }],
        (s) => {
          s.arm("plain");
          s.arm(p`a ${a}`, { optional: true });
          s.arm("b", { last: true });
          s.arm("c", { optional: true, last: true });
          s.arm("d", () => {});
          s.arm("e", { optional: true, last: true }, (e) => {
            e.read("x", int);
          });
        },
      );
    },
    'sections {}\nsections until "END" store stored skip other until "x" "y" {\n' +
      '  "plain"\n  "a {a:int}" optional\n  "b" last\n  "c" optional last\n  "d" {}\n' +
      '  "e" optional last {\n    x: int\n  }\n}\n',
  );
});

test("sections take their clauses as options", () => {
  assertPrints((b) => {
    const stored = b.forward("stored", "string", { optional: true });
    b.sections({ until: ["a", "b"], skipOther: true, store: stored });
    b.sections({ until: "c" });
  }, 'sections until "a" "b" store stored skip other {}\nsections until "c" {}\n');
});

test("branches", () => {
  assertPrints(
    (b) => {
      const n = b.read("n", int);
      b.if(n, () => {});
      b.if(n, (t) => {
        t.read("a", int);
      });
      b.if(n, (t) => {
        t.read("b", int);
      }).else((e) => {
        e.read("c", int);
      });
      b.if(n.gt(1), () => {})
        .elseIf(n.gt(0), () => {})
        .else(() => {});
      const branch = b.forward("if");
      b.if(n, () => {}).elseReadIf(branch, int);
      b.if(n, (t) => {
        t.read("d", int);
      }).elseIf(n.lt(0), (t) => {
        t.read("e", int);
      });
    },
    "n: int\nif n {}\nif n {\n  a: int\n}\nif n {\n  b: int\n} else {\n  c: int\n}\n" +
      "if n > 1 {} else if n > 0 {} else {}\nif n {} else if: int\n" +
      "if n {\n  d: int\n} else if n < 0 {\n  e: int\n}\n",
  );
});

test("let and check", () => {
  assertPrints((b) => {
    const x = b.let("x", 1);
    b.set(x, 2);
    b.check(x);
    b.check(x.gt(0), 'a "quoted" message');
  }, 'let x = 1\nlet x = 2\ncheck x\ncheck x > 0 "a \\"quoted\\" message"\n');
});

test("instances and a later format", () => {
  const b = new SpecBuilder("one");
  b.instances((i) => {
    i.read("x", int);
  });
  b.format("two");
  assert.equal(b.source(), "format one\ninstances {\n  x: int\n}\nformat two\n");
  b.build();
});

test("literals", () => {
  const cases: [Expr, string][] = [
    [lit(1), "1"],
    [lit(-1), "-1"],
    [floatLit(1), "1.0"],
    [floatLit(-1.5), "-1.5"],
    [floatLit(1e300), "1e300"],
    [floatLit(9007199254740992), "9007199254740992.0"],
    [floatLit(1e16), "1e16"],
    [floatLit(1e-5), "1e-5"],
    [floatLit(0.0001), "0.0001"],
    [floatLit(-0), "-0.0"],
    [lit("a\tb"), '"a\\tb"'],
    [lit("s"), '"s"'],
    [lit(4294967295), "4294967295"],
    [lit(-(2n ** 63n)), "(-9223372036854775807 - 1)"],
    [lit(2n ** 63n - 1n), "9223372036854775807"],
  ];
  for (const [value, expected] of cases) {
    assertExpression(() => value, expected);
  }
});

test("a number literal is a safe integer, and a bigint fits 64 bits", () => {
  assert.throws(() => lit(1.5), { name: "TypeError", message: /floatLit/u });
  assert.throws(() => lit(2 ** 53), { name: "RangeError", message: /bigint/u });
  assert.throws(() => lit(2n ** 63n), RangeError);
  assert.throws(() => lit(-(2n ** 63n) - 1n), RangeError);
  const b = new SpecBuilder();
  assert.throws(() => b.let("x", 0.5), { name: "TypeError", message: /floatLit/u });
});

test("host constants", () => {
  assertExpression(() => gecode.intMax.plus(0), "gecode.int_max + 0");
  assertExpression(() => gecode.intMin.neg(), "-gecode.int_min");
});

test("parentheses follow the precedence levels", () => {
  const cases: [(x: Int, y: Int, z: Int) => Expr, string][] = [
    [(x, y, z) => x.plus(y).plus(z), "x + y + z"],
    [(x, y, z) => x.plus(y.plus(z)), "x + (y + z)"],
    [(x, y, z) => x.minus(y.minus(z)), "x - (y - z)"],
    [(x, y, z) => x.times(y).plus(z), "x * y + z"],
    [(x, y, z) => x.times(y.plus(z)), "x * (y + z)"],
    [(x, y, z) => div(x.plus(y), z), "(x + y) / z"],
    [(x, y, z) => rem(x, y.times(z)), "x % (y * z)"],
    [(x, y, z) => x.lt(y.lt(z)), "x < (y < z)"],
    [(x, y, z) => x.lt(y).lt(z), "x < y < z"],
    [(x, y, z) => x.eq(y).ne(z), "x == y != z"],
    [(x, y, z) => x.eq(y.ne(z)), "x == (y != z)"],
    [(x, y, z) => x.le(y).ge(z).gt(1), "x <= y >= z > 1"],
    [(x, y, z) => x.lt(y).eq(z), "x < y == z"],
    [(x, y, z) => x.and(y).or(z), "x && y || z"],
    [(x, y, z) => x.and(y.or(z)), "x && (y || z)"],
    [(x, y, z) => x.and(y).and(z), "x && y && z"],
    [(x, y, z) => x.and(y.and(z)), "x && (y && z)"],
    [(x, y) => x.and(y).not(), "!(x && y)"],
    [(x, y) => x.not().and(y), "!x && y"],
    [(x, y) => x.plus(y).neg(), "-(x + y)"],
    [(x) => x.neg().neg(), "--x"],
    [(x) => x.not().not(), "!!x"],
    [(x) => x.not().neg(), "-!x"],
    [(x, y) => x.minus(y.neg()), "x - -y"],
    [(x, _y, z) => z.gt(x), "z > x"],
    [(x, y) => size(x).eq(y), "size(x) == y"],
  ];
  for (const [build, expected] of cases) {
    assertExpression(build, expected);
  }
});

test("powers bind tighter than unary on the left only", () => {
  const cases: [(x: Int, y: Int, z: Int) => Expr, string][] = [
    [() => pow(2, 2).neg(), "-2 ^ 2"],
    [() => pow(lit(2).neg(), 2), "(-2) ^ 2"],
    [() => pow(-2, 2), "(-2) ^ 2"],
    [() => pow(2, pow(3, 4)), "2 ^ 3 ^ 4"],
    [() => pow(pow(2, 3), 4), "(2 ^ 3) ^ 4"],
    [(x) => pow(2, x.neg()), "2 ^ -x"],
    [(x, y) => pow(x.plus(y), 2), "(x + y) ^ 2"],
    [(x, y) => pow(2, x.plus(y)), "2 ^ (x + y)"],
    [(x, y, z) => x.times(pow(y, z)), "x * y ^ z"],
  ];
  for (const [build, expected] of cases) {
    assertExpression(build, expected);
  }
});

test("operators take literals on either side", () => {
  assertExpression((x) => lit(1).plus(x), "1 + x");
  assertExpression((x) => lit(2).times(x.minus(1)), "2 * (x - 1)");
  assertExpression((x) => div(7, x), "7 / x");
  assertExpression((x) => rem(7, x), "7 % x");
  assertExpression((x) => lit(1).plus(gecode.intMax).minus(x), "1 + gecode.int_max - x");
  assertExpression((x) => x.gt(1), "x > 1");
  assertExpression((x) => lit(1).lt(x), "1 < x");
});

test("calls and comprehensions", () => {
  assertPrints(
    (b) => {
      const a = b.read("a", int.dim(2));
      b.let("v1", size(a));
      b.let("v2", count(a));
      b.let("v3", min(a));
      b.let("v4", max(a, 1));
      b.let("v5", sum(1, 2, 3));
      b.let("v6", distinct(1, 2));
      b.let("v7", defined(a));
      b.let(
        "v8",
        all(a, (x) => x.gt(0)),
      );
      b.let(
        "v9",
        any(a, (x) => x.gt(0)),
      );
      b.let(
        "v10",
        sum(a, (x) => x),
      );
      b.let(
        "v11",
        count(over(a, "item"), (x) => x.gt(1)),
      );
      b.let(
        "v12",
        sum(a, (x) => sum(a, (y) => x.dynamic().times(y))),
      );
      b.let("v13", a.at(0).plus(a.at(1)));
    },
    "a[2]: int\nlet v1 = size(a)\nlet v2 = count(a)\nlet v3 = min(a)\nlet v4 = max(a, 1)\n" +
      "let v5 = sum(1, 2, 3)\nlet v6 = distinct(1, 2)\nlet v7 = defined(a)\n" +
      "let v8 = all(element_1 in a: element_1 > 0)\n" +
      "let v9 = any(element_2 in a: element_2 > 0)\n" +
      "let v10 = sum(element_3 in a: element_3)\nlet v11 = count(item in a: item > 1)\n" +
      "let v12 = sum(element_4 in a: sum(element_5 in a: element_4 * element_5))\n" +
      "let v13 = a[0] + a[1]\n",
  );
});

test("internal names skip every name in the spec", () => {
  assertPrints(
    (b) => {
      b.read("record_1", int);
      b.read("records_1", int);
      const a = b.read("element_1", int.dim(1));
      const [job, x] = b.repeat(1, (r) => [r.record, r.read("x", int)] as const);
      b.let(
        "v",
        sum(job, (j) => j.get(x)),
      );
      b.check(job.get(x));
      b.let(
        "u",
        all(a, (e) => e),
      );
    },
    "record_1: int\nrecords_1: int\nelement_1[1]: int\nrepeat 1 as records_2 {\n" +
      "  each record_2\n  x: int\n}\nlet v = sum(element_2 in records_2: element_2.x)\n" +
      "check record_2.x\nlet u = all(element_3 in element_1: element_3)\n",
  );
});

test("capture types name the records and elements they use", () => {
  let handles: [Field, Field, Field] | undefined;
  const builder = assertText(
    (b) => {
      const a = b.read("a", int.dim(2));
      const x = b.capture(
        "x",
        range(
          0,
          sum(a, (v) => v),
        ),
      );
      b.line(pattern(x));
      const [record, y, z] = b.repeat(2, (r) => {
        const row = r.read("y", int);
        const bound = r.capture("z", range(0, r.record.get(row)));
        r.line(pattern(bound));
        return [r.record, row, bound] as const;
      });
      const w = b.capture(
        "w",
        range(
          0,
          sum(record, (e) => e.get(y)),
        ),
      );
      b.line(pattern(w));
      handles = [x, z, w];
    },
    'a[2]: int\nline "{x:0..sum(element_1 in a: element_1)}"\n' +
      'repeat 2 as records_1 {\n  each record_1\n  y: int\n  line "{z:0..record_1.y}"\n}\n' +
      'line "{w:0..sum(element_2 in records_1: element_2.y)}"\n',
  );
  assert.ok(handles !== undefined);
  const data = builder.build().read("1 2\n3\n1\n1\n2\n2\n3\n");
  assert.deepEqual(
    handles.map((handle) => data.get(handle)),
    [3, [1, 2], 3],
  );
});

test("records project and index", () => {
  assertPrints(
    (b) => {
      const [outer, inner, x] = b.repeat(2, (r) => {
        const [record, field] = r.repeat(2, (s) => [s.record, s.read("x", int)] as const);
        return [r.record, record, field] as const;
      });
      b.let("a", outer.list.at(0).get(x).at(1));
      b.check(size(outer.records(inner)));
      b.let("c", outer.list.at(1).records(inner).at(0).get(x));
      b.let("d", size(outer.list));
      b.check(defined(outer));
      b.let(
        "f",
        sum(outer, (o) => sum(o.records(inner), (i) => i.get(x))),
      );
    },
    "repeat 2 as records_1 {\n  each record_1\n  repeat 2 as records_2 {\n    x: int\n  }\n}\n" +
      "let a = records_1[0].x[1]\ncheck size(record_1.records_2)\n" +
      "let c = records_1[1].records_2[0].x\nlet d = size(records_1)\n" +
      "check defined(records_1)\n" +
      "let f = sum(element_1 in records_1: sum(element_2 in element_1.records_2: element_2.x))\n",
  );
});

test("a member never glues to a literal or a name gecode", () => {
  assertText((b) => {
    const x = b.read("x", int);
    const g = b.undefined("gecode");
    b.let("a", lit(1).dynamic().get(x));
    b.let("b", g.at(0).get(x));
    b.let("c", g.dynamic().get(x));
    b.let("d", g.dynamic().get(g).get(x));
    b.let("e", lit(1).dynamic().plus(2).dynamic().get(x));
  }, "x: int\nlet a = (1).x\nlet b = gecode[0].x\nlet c = gecode . x\nlet d = gecode . gecode . x\nlet e = (1 + 2).x\n");
});

test("keywords at the start of an expression take parentheses", () => {
  assertPrints(
    (b) => {
      const [until, line, end, integer, layout, alpha, t] = [
        "until",
        "line",
        "eol",
        "int",
        "layout",
        "alphabet",
        "t",
      ].map((name) => b.read(name, int));
      assert.ok(t !== undefined && until && line && end && integer && layout && alpha);
      b.repeat(until, () => {});
      b.repeat(until.plus(1), () => {});
      b.skipLines(line);
      b.skipLines(line.times(2));
      b.list("a", int.until(end));
      b.list("b", int.until(end.plus(1)));
      b.read("c", range(0, layout));
      b.read("d", range(0).below(layout));
      b.read("e", atMost(layout.plus(1)));
      b.read("f", range(integer, t));
      b.read("g", atLeast(integer.plus(1)));
      b.read("h", atLeast(alpha));
      b.read("j", atLeast(t));
      b.line(pattern(validate(range(0, layout))));
    },
    "until: int\nline: int\neol: int\nint: int\nlayout: int\nalphabet: int\nt: int\n" +
      "repeat (until) {}\nrepeat (until + 1) {}\nskip (line) lines\n" +
      "skip (line * 2) lines\nlist a: int until (eol)\nlist b: int until (eol + 1)\n" +
      "c: 0..(layout)\nd: 0..<(layout)\ne: ..(layout + 1)\nf: (int)..t\ng: (int + 1)..\n" +
      'h: (alphabet)..\nj: t..\nline "{:0..(layout)}"\n',
  );
});

test("a named type takes parentheses as a lower bound", () => {
  assertText((b) => {
    const named = b.undefined("named");
    b.defineType("named", int);
    b.read("x", atLeast(named.dynamic()));
  }, "type named = int\nx: (named)..\n");
});

test("every stop word is guarded as an upper bound", () => {
  const words = [
    "comment",
    "stop",
    "expect",
    "tokens",
    "separators",
    "ignore",
    "param",
    "list",
    "line",
    "find",
    "skip",
    "index",
    "repeat",
    "each",
    "instances",
    "sections",
    "if",
    "let",
    "check",
    "type",
    "layout",
    "offset",
    "until",
  ];
  for (const name of words) {
    const b = new SpecBuilder();
    const bound = b.let(name, 5);
    const x = b.read("x", range(0, bound));
    const text = b.source();
    assert.ok(text.endsWith(`x: 0..(${name})\n`), text);
    assert.equal(b.build().read("3").get(x), 3);
  }
});

test("numbers round trip", () => {
  const b = new SpecBuilder();
  const values = [0.1, 1e-7, 1.7976931348623157e308, 123456789012345680, -0];
  const fields = values.map((value, index) => b.let(`v${index}`, floatLit(value)));
  const minimum = b.let("minimum", -(2n ** 63n));
  const data = b.build().read("");
  fields.forEach((handle, index) => {
    assert.ok(Object.is(data.get(handle), values[index]));
  });
  assert.equal(data.get(minimum, { bigint: true }), -(2n ** 63n));
});

test("a long chain prints", () => {
  // Composing keeps a tree, so a long chain takes linear time and no recursion.
  const b = new SpecBuilder();
  const x = b.read("x", int);
  let chain = x.plus(1);
  for (let index = 0; index < 200_000; index++) {
    chain = chain.plus(x);
  }
  b.let("v", chain);
  assert.equal(b.source().split("+").length - 1, 200_001);
});

test("a deep right chain prints without recursion", () => {
  const b = new SpecBuilder();
  const x = b.read("x", int);
  let chain = x.neg();
  for (let index = 0; index < 20_000; index++) {
    chain = chain.neg();
  }
  b.let("v", chain);
  assert.equal(b.source().split("-").length - 1, 20_001);
});

test("three hundred dimensions read", () => {
  const b = new SpecBuilder();
  const deep = b.read("deep", int.dims(...Array.from({ length: 300 }, () => 1)));
  assert.equal(deep.depth, null);
  let value: unknown = b.build().read("7").get(deep);
  for (let index = 0; index < 299; index++) {
    assert.ok(Array.isArray(value));
    value = value[0];
  }
  assert.deepEqual(value, [7]);
});

/** Reads `leaf` inside `depth` nested repeats. */
function nest(scope: Scope<HandleDepth, Presence>, depth: number): Field {
  if (depth === 0) {
    return scope.read("leaf", int);
  }
  return scope.repeat(1, (inner: Scope<HandleDepth, Presence>) => nest(inner, depth - 1));
}

test("ten nested repeats stay open past the static depths", () => {
  const b = new SpecBuilder();
  const leaf = nest(b, 10);
  assert.equal(leaf.depth, null);
  let value: unknown = b.build().read("9").get(leaf);
  for (let index = 0; index < 9; index++) {
    assert.ok(Array.isArray(value));
    value = value[0];
  }
  assert.deepEqual(value, [9]);
});

test("num entries read as numbers", () => {
  const b = new SpecBuilder();
  const x = b.read("x", num);
  assert.equal(b.build().read("2.5").get(x), 2.5);
});
