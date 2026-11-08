// The builder's own checks: each throws a BuildError at the call that failed
// it, or from build(), which throws the first one again. A port of
// python/tests/test_build_checks.py, with the misuse that callbacks allow.
import assert from "node:assert/strict";
import { test } from "node:test";
import type { Spec } from "gecode-iris";
import {
  all,
  alphabet,
  atLeast,
  bind,
  BuildError,
  defined,
  discard,
  float,
  floatLit,
  int,
  lit,
  over,
  pattern,
  ref,
  size,
  SpecBuilder,
  validate,
  word,
  type BuildErrorKind,
  type Field,
  type Scope,
} from "gecode-iris/build";

/**
 * Runs `build` on a new builder and expects an error of `kind`, from a
 * statement or from build(). A statement's error comes again from build().
 */
function failureOf(build: (b: SpecBuilder) => unknown, kind: BuildErrorKind): BuildError {
  const b = new SpecBuilder();
  let error: unknown;
  try {
    build(b);
  } catch (raised) {
    error = raised;
  }
  if (error === undefined) {
    try {
      b.build();
    } catch (raised) {
      error = raised;
    }
  } else {
    assert.throws(
      () => b.build(),
      (again) => again === error,
    );
  }
  assert.ok(error instanceof BuildError, String(error));
  assert.equal(error.kind, kind, error.message);
  return error;
}

function builds(build: (b: SpecBuilder) => unknown): Spec {
  const b = new SpecBuilder();
  build(b);
  return b.build();
}

test("a statement through an outer scope is rejected", () => {
  failureOf((b) => {
    b.instances((s) => {
      const n = s.capture("n", int);
      s.sections((arms) => {
        arms.arm(pattern(n), () => {
          s.read("x", int);
        });
      });
    });
  }, "scope");
});

test("an arm through an outer block is rejected", () => {
  failureOf((b) => {
    b.sections((arms) => {
      arms.arm("a", () => {
        arms.arm("b");
      });
    });
  }, "scope");
});

test("a scope kept after its block closed is rejected", () => {
  failureOf((b) => {
    let kept: Scope<1> | undefined;
    b.repeat(1, (r) => {
      kept = r;
    });
    kept?.read("x", int);
  }, "scope");
});

test("a guard after later statements is rejected", () => {
  const error = failureOf((b) => {
    const n = b.read("n", int);
    const guard = b.if(n, () => {});
    b.read("y", int);
    guard.else(() => {});
  }, "scope");
  assert.match(error.message, /later statements/u);
  failureOf((b) => {
    const guard = b.if(1, () => {});
    b.read("y", int);
    guard.elseIf(1, () => {});
  }, "scope");
});

test("a block's callback must be synchronous", () => {
  const error = failureOf((b) => {
    b.repeat(1, async (r) => {
      r.read("x", int);
    });
  }, "scope");
  assert.match(error.message, /promise/u);
});

test("a handle from another builder is rejected", () => {
  const other = new SpecBuilder();
  const foreign = other.read("x", int);
  const record = other.repeat(1, (r) => r.record);
  const named = other.defineType("t", int);
  failureOf((b) => b.let("y", foreign.plus(1)), "provenance");
  failureOf((b) => b.read(foreign, int), "provenance");
  failureOf((b) => b.line(pattern(ref(foreign))), "provenance");
  failureOf((b) => b.check(record.list.at(0).get(foreign)), "provenance");
  failureOf((b) => b.read("x", named), "provenance");
  failureOf((b) => b.capture("x", named), "provenance");
});

test("defining a name twice needs the handle", () => {
  const error = failureOf((b) => {
    b.read("x", int);
    b.let("x", 1);
  }, "redefinition");
  assert.match(error.message, /`x`/u);
  builds((b) => {
    const x = b.read("x", int);
    b.set(x, 2);
    b.let(x, floatLit(1.5));
  });
});

test("values with no spelling are rejected", () => {
  const unprintable: ((b: SpecBuilder) => unknown)[] = [
    (b) => b.line("a { b"),
    (b) => b.line("a ... b"),
    (b) => b.read("format", int),
    (b) => b.read("_", int),
    (b) => b.list("_", int.until(0)),
    (b) => b.let("_", 1),
    (b) => b.param("_"),
    (b) => b.defineType("_", int),
    (b) => b.capture("_", int),
    (b) => b.optionalCapture("_", int),
    (b) => b.repeatUntil("x", { store: "_" }),
    // @ts-expect-error a store needs a name
    (b) => b.repeatUntil("x", { store: discard() }),
    // @ts-expect-error a store needs a name
    (b) => b.sections({ until: "x", store: discard() }),
    (b) => b.read("gecode.x", int),
    (b) => b.read("not a name", int),
    (b) => b.read("", int),
    (b) => {
      const a = b.read("a", int.dim(1));
      b.let(
        "v",
        all(over(a, "not a name"), (x) => x),
      );
    },
    (b) => b.let("x", floatLit(Number.NaN)),
    (b) => b.let("x", floatLit(Number.POSITIVE_INFINITY)),
    (b) => {
      b.read("x", atLeast(0));
      b.format("second");
    },
    (b) => {
      b.defineType("t", atLeast(0));
      b.format("second");
    },
    (b) => b.read("x", alphabet("ab", { base: -(2n ** 63n) })),
    (b) => b.list("x", int.layout(1).until(0)),
    (b) => b.line(pattern(validate(atLeast(lit("}").dynamic())))),
    (b) => b.sections({ skipOther: true, store: "end" }),
  ];
  for (const build of unprintable) {
    failureOf(build, "unprintable");
  }
});

test("a bound after an open range is fine before other statements", () => {
  builds((b) => {
    b.read("x", atLeast(0));
    b.read("y", atLeast(0));
    b.instances();
    b.format("second");
  });
});

test("a capture binds exactly once in its scope", () => {
  const twice = failureOf((b) => {
    const n = b.capture("n", int);
    b.line(pattern(n));
    b.line(pattern(n));
  }, "binding");
  assert.match(twice.message, /twice/u);
  const never = failureOf((b) => b.capture("n", int), "binding");
  assert.match(never.message, /never/u);
  const elsewhere = failureOf((b) => {
    const n = b.capture("n", int);
    b.repeat(1, (r) => {
      r.line(pattern(n));
    });
  }, "binding");
  assert.match(elsewhere.message, /different scope/u);
});

test("the presence of a capture follows its arm", () => {
  const required = failureOf((b) => {
    const n = b.capture("n", int);
    b.sections((arms) => {
      arms.arm(pattern(n), { optional: true });
    });
  }, "binding");
  assert.match(required.message, /optionalCapture/u);
  const optional = failureOf((b) => {
    const n = b.optionalCapture("n", int);
    b.line(pattern(n));
  }, "binding");
  assert.match(optional.message, /`capture`/u);
  builds((b) => {
    const n = b.optionalCapture("n", int);
    b.sections((arms) => {
      arms.arm(pattern(n), { optional: true });
    });
  });
});

test("a handle that is not a capture is not guessed", () => {
  const error = failureOf((b) => {
    const x = b.read("x", int);
    b.line(pattern(x));
  }, "binding");
  assert.match(error.message, /`ref`/u);
  builds((b) => {
    const x = b.read("x", int);
    b.line(pattern(ref(x)));
  });
});

test("a forward handle must be bound", () => {
  const error = failureOf((b) => b.forward("x", "int"), "binding");
  assert.match(error.message, /never/u);
  failureOf((b) => b.forwardRecord("each_one", "all_of_them"), "binding");
});

test("a forward handle must be bound as declared", () => {
  failureOf((b) => {
    const x = b.forward("x", "int");
    b.read(x, float);
  }, "binding");
  failureOf((b) => {
    const x = b.forward("x", "int");
    b.repeat(2, (r) => {
      r.read(x, int);
    });
  }, "binding");
  const absent = failureOf((b) => {
    const x = b.forward("x", "int");
    b.if(1, (t) => {
      t.read(x, int);
    });
  }, "binding");
  assert.match(absent.message, /\{ optional: true \}/u);
  builds((b) => {
    const x = b.forward("x", "int", { depth: 1 });
    b.repeat(2, (r) => {
      r.read(x, int);
    });
  });
});

test("a later definition of another type is a redefinition", () => {
  const b = new SpecBuilder();
  const x = b.forward("x", "int");
  b.read(x, int);
  const again = b.read(x, float);
  assert.equal(again.valueType, "dyn");
  assert.equal(again.depth, null);
  const data = b.build().read("1 2.5");
  assert.equal(data.get(again), 2.5);
  assert.throws(() => data.get(x), { name: "TypeError", message: /does not match its handle/u });
  failureOf((inner) => {
    const y = inner.forward("x", "int");
    inner.if(1, (t) => {
      t.read(y, int);
    });
    inner.if(1, (t) => {
      t.read(y, int);
      t.read(y, float);
    });
  }, "binding");
});

test("a redefinition needs a binding that always precedes it", () => {
  const inElse = failureOf((b) => {
    const x = b.forward("x", "int", { optional: true });
    b.if(1, (t) => {
      t.read(x, int);
    }).else((e) => {
      e.read(x, float);
    });
  }, "binding");
  assert.match(inElse.message, /float at depth 0/u);
  failureOf((b) => {
    const x = b.forward("x", "int", { optional: true });
    b.if(1, (t) => {
      t.read(x, int);
    });
    b.read(x, float);
  }, "binding");
  failureOf((b) => {
    const x = b.forward("x", "int", { optional: true });
    b.sections((arms) => {
      arms.arm(pattern(bind(x, int)), { optional: true });
      arms.arm(pattern(bind(x, float)), { optional: true });
    });
  }, "binding");
  builds((b) => {
    const x = b.forward("x", "int", { optional: true });
    b.if(1, (t) => {
      t.read(x, int);
      t.if(1, (nested) => {
        nested.read(x, float);
      });
    });
  });
});

/** A build that reads an int array `a` and passes it to `let_`. */
function arrayInto(let_: (b: SpecBuilder, a: Field<"int", 1>) => unknown) {
  return (b: SpecBuilder) => let_(b, b.read("a", int.dim(2)));
}

test("an unknown shape does not bind a static forward handle", () => {
  const whole = failureOf(
    arrayInto((b, a) => b.let(b.forward("x", "int"), a)),
    "binding",
  );
  assert.match(whole.message, /int at depth 1/u);
  failureOf(
    arrayInto((b, a) => b.let(b.forward("x", "int"), a.at(0))),
    "binding",
  );
  failureOf(
    arrayInto((b, a) => b.let(b.forward("x", "int"), a.dynamic())),
    "binding",
  );
  failureOf((b) => b.let(b.forward("x", "int"), lit("text").dynamic()), "binding");
  failureOf((b) => b.param(b.forward("x", "int")), "binding");
  const bare = failureOf((b) => {
    const y = b.forward("y", "int", { depth: 1 });
    b.repeat(1, (r) => {
      const v = r.read("v", int);
      r.let(y, v);
    });
  }, "binding");
  assert.match(bare.message, /unknown depth/u);
});

test("a known shape binds a static forward handle", () => {
  let b = new SpecBuilder();
  const a = b.read("a", int.dim(2));
  const n = b.read("n", int);
  const whole = b.forward("whole", "int", { depth: 1 });
  b.let(whole, a);
  const scalar = b.forward("scalar", "int");
  b.let(scalar, n);
  const computed = b.forward("computed", "int");
  b.let(computed, n.plus(1));
  const rows = b.forward("rows", "int", { depth: 1 });
  b.repeat(2, (r) => {
    r.let(rows, n.plus(2));
  });
  let data = b.build().read("1 2 3");
  assert.deepEqual(data.get(whole), [1, 2]);
  assert.equal(data.get(scalar), 3);
  assert.equal(data.get(computed), 4);
  assert.deepEqual(data.get(rows), [5, 5]);

  b = new SpecBuilder();
  const pair = b.read("a", int.dim(2));
  const x = b.forward("x");
  b.let(x, pair.at(0));
  const y = b.forward("y");
  b.repeat(1, (r) => {
    r.let(y, pair);
  });
  data = b.build().read("7 8");
  assert.equal(data.get(x), 7);
  assert.deepEqual(data.get(y), [[7, 8]]);
});

test("set keeps a shape the types cannot compare until the walk", () => {
  failureOf((b) => {
    const a = b.read("a", int.dim(2));
    const y = b.forward("y", "int", { depth: 1 });
    b.repeat(1, (r) => {
      r.set(y, a);
    });
  }, "binding");
});

test("set rejects another value type or a known other depth", () => {
  const b = new SpecBuilder();
  const x = b.read("x", int);
  assert.throws(
    // @ts-expect-error a float is not an int
    () => b.set(x, floatLit(1.5)),
    { name: "TypeError", message: /value type int/u },
  );
  const a = b.read("a", int.dim(2));
  // @ts-expect-error the handle is a scalar, the value an array
  assert.throws(() => b.set(x, a), {
    name: "TypeError",
    message: /redefine the name with `let/u,
  });
  // @ts-expect-error the handle is an array, the value a scalar
  assert.throws(() => b.set(a, 1), {
    name: "TypeError",
    message: /keeps the handle's depth 1, but this value has depth 0/u,
  });
  const loose = b.let(x, floatLit(1.5));
  b.set(loose, "anything");
  b.build();
});

/** Defines a type `t` inside a conditional block and returns its handle. */
function oldType(b: SpecBuilder) {
  return b.if(1, (s) => s.defineType("t", int)).result;
}

test("a named type handle belongs to its declaration", () => {
  const read = failureOf((b) => {
    const old = oldType(b);
    b.if(1, (s) => {
      s.defineType("t", float);
      s.read("x", old);
    });
  }, "scope");
  assert.match(read.message, /`t`/u);
  failureOf((b) => {
    const old = oldType(b);
    b.if(1, (s) => {
      s.defineType("t", float);
      const x = s.optionalCapture("x", old);
      s.line(pattern(x));
    });
  }, "scope");
  failureOf((b) => {
    const old = oldType(b);
    b.if(1, (s) => {
      s.defineType("t", float);
      s.line(pattern(validate(old)));
    });
  }, "scope");
  failureOf((b) => b.read("x", oldType(b)), "scope");

  const b = new SpecBuilder();
  const first = b.if(1, (s) => {
    const t = s.defineType("t", int);
    const handle = s.read("first", t);
    s.if(1, (nested) => {
      nested.read("nested", t);
    });
    return handle;
  }).result;
  const second = b.if(1, (s) => s.read("second", s.defineType("t", float))).result;
  const data = b.build().read("1 2 2.5");
  assert.equal(data.get(first), 1);
  assert.equal(data.get(second), 2.5);
});

test("only a store defines the entry named _", () => {
  const binding = (define: (b: SpecBuilder, stored: Field<"dyn", null, "maybe">) => unknown) => {
    const error = failureOf((b) => {
      const stored = b.forward("_");
      b.repeatUntil("END", { store: stored });
      define(b, stored);
    }, "binding");
    assert.match(error.message, /`store`/u);
  };
  binding((b, stored) => b.read(stored, int));
  binding((b, stored) => b.list(stored, int.until(0)));
  binding((b, stored) => b.let(stored, 1));
  binding((b, stored) => b.param(stored));
  binding((b, stored) => b.line(pattern(bind(stored, int))));
  failureOf((b) => b.check(defined(b.forward("_"))), "binding");
});

test("a store of _ reads as an entry at the top level", () => {
  let b = new SpecBuilder();
  const stored = b.forward("_", "string");
  b.repeatUntil("END", { store: stored });
  const copy = b.let("s", stored);
  b.line(pattern(ref(stored)));
  const spec = b.build();
  assert.equal(b.source(), 'repeat until "END" store _ {}\nlet s = _\nline "{$_}"\n');
  const data = spec.read("END\nEND");
  assert.equal(data.get(stored), "END");
  assert.equal(data.get(copy), "END");
  failureOf((inner) => inner.repeatUntil("END", { store: inner.forward("_", "int") }), "binding");

  b = new SpecBuilder();
  const maybe = b.forward("_", "string", { optional: true });
  b.sections({ until: "END", store: maybe });
  assert.equal(b.build().read("END").get(maybe), "END");
});

test("a store of _ in a repeat leaves no entry", () => {
  failureOf((b) => {
    const stored = b.forward("_", "string", { depth: 1 });
    b.repeat(1, (r) => {
      r.repeatUntil("END", { store: stored });
    });
  }, "binding");
  const required = failureOf((b) => {
    const stored = b.forward("_", "string");
    b.repeat(1, (r) => {
      r.repeatUntil("END", { store: stored });
    });
  }, "binding");
  assert.match(required.message, /\{ optional: true \}/u);

  let b = new SpecBuilder();
  const stored = b.forward("_");
  const copy = b.repeat(2, (r) => {
    r.repeatUntil("END", { store: stored });
    return r.let("copy", stored);
  });
  const data = b.build().read("END END");
  assert.equal(data.has("_"), false);
  assert.deepEqual(data.get(copy), ["END", "END"]);

  b = new SpecBuilder();
  const maybe = b.forward("_", "string", { optional: true });
  b.repeat(1, (r) => {
    r.repeatUntil("END", { store: maybe });
  });
  assert.equal(b.build().read("END").get(maybe), undefined);
});

test("_ names records", () => {
  let b = new SpecBuilder();
  b.repeat(1, (r) => {
    r.asName("_");
    r.eachName("row");
    r.read("x", int);
  });
  assert.ok(b.source().includes("repeat 1 as _ {"));
  b.build();

  b = new SpecBuilder();
  const [record, x] = b.repeat(1, (r) => {
    r.eachName("_");
    return [r.record, r.read("x", int)] as const;
  });
  b.check(record.get(x).eq(1));
  assert.ok(b.source().includes("check _.x == 1"));
  b.build().read("1");

  b = new SpecBuilder();
  const rows = b.forwardRecord("_", "rows");
  b.repeat(1, (r) => {
    r.recordAs(rows);
    r.read("x", int);
  });
  const n = b.let("n", size(rows));
  assert.equal(b.build().read("1").get(n), 1);
});

test("a conditional and a later definition are not optional", () => {
  builds((b) => {
    const x = b.forward("x", "int");
    b.if(1, (t) => {
      t.read(x, int);
    });
    b.let(x, 1);
  });
});

test("a pattern binds a forward handle", () => {
  builds((b) => {
    const x = b.forward("x", "string");
    b.line(pattern("x = ", bind(x, word)));
  });
  failureOf((b) => {
    const x = b.forward("x", "int");
    b.line(pattern("x = ", bind(x, word)));
  }, "binding");
});

test("an undefined name cannot be defined", () => {
  const b = new SpecBuilder();
  const zz = b.undefined("zz");
  b.check(defined(zz).not());
  b.source();
  b.let(zz, 1);
  assert.throws(
    () => b.build(),
    (error) =>
      error instanceof BuildError && error.kind === "binding" && /undefined/u.test(error.message),
  );
  const stored = failureOf(
    (inner) => inner.repeatUntil("END", { store: inner.undefined("_") }),
    "binding",
  );
  assert.match(stored.message, /undefined/u);
});

test("a store defines a forward handle", () => {
  builds((b) => {
    const end = b.forward("end", "string");
    b.repeatUntil("END", { store: end }, (r) => {
      r.read("x", int);
    });
  });
  failureOf((b) => b.sections({ until: "END", store: b.forward("end", "string") }), "binding");
  builds((b) =>
    b.sections({ until: "END", store: b.forward("end", "string", { optional: true }) }),
  );
});

test("a store does not precede the body of its repeat", () => {
  failureOf((b) => {
    const x = b.forward("x", "string");
    b.repeatUntil("END", { store: x }, (r) => {
      r.let(x, 1);
      r.read(discard(), int);
    });
  }, "binding");
  builds((b) => {
    const x = b.forward("x", "string");
    b.repeatUntil("END", { store: x }, (r) => {
      r.read(discard(), int);
    });
    b.read(x, int);
  });
});

test("a num forward handle takes an int or a float", () => {
  const b = new SpecBuilder();
  const n = b.forward("n", "num");
  b.read(n, int);
  assert.equal(b.build().read("7").get(n), 7);
  builds((inner) => inner.let(inner.forward("n", "num"), floatLit(1.5)));
  failureOf((inner) => inner.read(inner.forward("n", "num"), word), "binding");
  failureOf((inner) => inner.read(inner.forward("n", "num"), int.dim(2)), "binding");
});

test("a forward record binds once", () => {
  failureOf((b) => {
    const job = b.forwardRecord("job", "jobs");
    b.repeat(1, (r) => {
      r.recordAs(job);
    });
    b.repeat(1, (r) => {
      r.recordAs(job);
    });
  }, "binding");
  failureOf((b) => {
    const record = b.repeat(1, (r) => r.record);
    b.repeat(1, (r) => {
      r.recordAs(record);
    });
  }, "binding");
});

test("the language decides the rest", () => {
  let b = new SpecBuilder();
  const x = b.undefined("x");
  b.let("y", x);
  b.read(discard(), int);
  b.build();
  assert.ok(b.source().includes("let y = x"));

  b = new SpecBuilder();
  b.instances();
  b.read("after", int);
  assert.throws(
    () => b.build(),
    (error) => {
      assert.ok(error instanceof BuildError);
      assert.equal(error.kind, "spec");
      assert.ok(error.error !== null);
      assert.equal(error.cause, error.error);
      assert.ok(error.text?.includes("after: int"));
      return true;
    },
  );
});

test("forward rejects a value type or depth it does not know", () => {
  const b = new SpecBuilder();
  // @ts-expect-error not a value type
  assert.throws(() => b.forward("x", "bool"), TypeError);
  assert.throws(() => b.forward("x", "int", { depth: -1 }), TypeError);
  assert.throws(() => b.forward("x", "int", { depth: 1.5 }), TypeError);
});

test("a boolean is not a condition", () => {
  const b = new SpecBuilder();
  b.read("x", int);
  // @ts-expect-error a boolean is not an Iris expression
  assert.throws(() => b.check(true), { name: "TypeError", message: /not a boolean/u });
  // @ts-expect-error a boolean is not an Iris expression
  assert.throws(() => b.if(false, () => {}), { name: "TypeError", message: /not a boolean/u });
});

test("an expression is not text", () => {
  const b = new SpecBuilder();
  const x = b.read("x", int);
  assert.throws(() => String(x), { name: "TypeError", message: /\bp`/u });
  assert.throws(() => x.toString(), TypeError);
});

test("operands have the types their operators take", () => {
  const b = new SpecBuilder();
  const x = b.read("x", int);
  const s = b.read("s", word);
  const f = b.read("f", float);
  // @ts-expect-error a word is not an int
  assert.throws(() => x.plus(s), TypeError);
  // @ts-expect-error arithmetic takes ints
  assert.throws(() => f.plus(1), TypeError);
  // @ts-expect-error comparisons of order take numbers
  assert.throws(() => s.lt("a"), TypeError);
  // @ts-expect-error a boolean is not a literal
  assert.throws(() => lit(true), TypeError);
  assert.throws(() => lit(2n ** 63n), RangeError);
  assert.equal(f.lt(floatLit(1.5)).valueType, "int");
  assert.equal(s.eq("a").valueType, "int");
  assert.equal(s.dynamic().plus(1).valueType, "int");
  // @ts-expect-error an int is not compared with a word
  assert.throws(() => x.eq(s), TypeError);
  // @ts-expect-error a word is not compared with an int
  assert.throws(() => s.ne(x), TypeError);
  assert.equal(x.eq(s.dynamic()).valueType, "int");
  assert.equal(x.ne(f).valueType, "int");
});

test("a check throws where it fails and build throws it again", () => {
  const b = new SpecBuilder();
  b.read("x", int);
  let first: unknown;
  try {
    b.read("x", int);
  } catch (error) {
    first = error;
  }
  assert.ok(first instanceof BuildError);
  assert.throws(() => b.forward("x", "int"), { name: "BuildError", message: /already declared/u });
  assert.throws(
    () => b.source(),
    (again) => again === first,
  );
});
