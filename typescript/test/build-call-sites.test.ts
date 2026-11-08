// Errors point at the host code: every statement records the line that called
// it, and failures carry that line beside their location in the emitted text.
// A port of rust/iris-build/tests/call_sites.rs, by way of
// python/tests/test_build_call_sites.py.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { IrisError, Spec, type CallSite } from "gecode-iris";
import {
  alphabet,
  BuildError,
  float,
  floatLit,
  int,
  lit,
  p,
  pattern,
  range,
  rest,
  spec,
  Scope,
  SpecBuilder,
  trackCaller,
  word,
  type HandleDepth,
  type Presence,
} from "gecode-iris/build";
import { callSitesOf } from "../dist/build/scope.js";
import { parseFrame } from "../dist/build/site.js";

const FILE = import.meta.url;

/** The line of the caller's frame, `up` frames above the caller of this. */
function lineOf(up: number): number {
  const frame = new Error().stack?.split("\n")[2 + up] ?? "";
  const line = parseFrame(frame)?.line;
  assert.ok(line !== undefined, frame);
  return line;
}

/** The line of the caller. */
function here(): number {
  return lineOf(1);
}

function assertHere(site: CallSite | null | undefined, line: number): void {
  assert.ok(site, "the failure has a call site");
  assert.deepEqual([site.file, site.line], [FILE, line]);
}

/** What `b.build()` throws, which must be a `BuildError`. */
function buildError(b: SpecBuilder): BuildError {
  let caught: unknown = null;
  try {
    b.build();
  } catch (error) {
    caught = error;
  }
  assert.ok(
    caught instanceof BuildError,
    caught instanceof Error ? caught.message : "build() succeeds",
  );
  return caught;
}

function specError(error: BuildError): IrisError {
  assert.ok(error.error !== null, "the emitted text is rejected");
  return error.error;
}

/** What reading `data` throws, which must be an `IrisError`. */
function readError(built: Spec, data: string, origin = "input.txt"): IrisError {
  let caught: unknown = null;
  try {
    built.read(data, origin);
  } catch (error) {
    caught = error;
  }
  assert.ok(
    caught instanceof IrisError,
    caught instanceof Error ? caught.message : "the read succeeds",
  );
  return caught;
}

test("a parse error names the statement after instances", () => {
  const b = new SpecBuilder();
  b.instances();
  const line = here() + 1;
  b.read("after", int);
  const error = buildError(b);
  assertHere(error.callSite, line);
  assertHere(specError(error).callSite, line);
  assert.equal(specError(error).specLocation?.origin, "builder.iris");
  assert.equal(specError(error).specLocation?.line, 2);
  assert.equal(error.emittedLine, "after: int");
  assert.equal(error.text, "instances {}\nafter: int\n");
  assert.equal(error.cause, error.error);
});

test("a resolution error names the statement that uses the name", () => {
  const b = new SpecBuilder();
  const outside = b.read("outside", int);
  const record = b.repeat(2, (s) => {
    s.read("inside", int);
    return s.record;
  });
  const line = here() + 1;
  b.let("bad", record.get(outside));
  const error = buildError(b);
  assert.equal(specError(error).code, "E-SPEC-RECORD-SCOPE");
  assertHere(error.callSite, line);
  assert.equal(specError(error).specLocation?.line, 6);
  assert.equal(error.emittedLine, "let bad = record_1.outside");
});

test("the display shows the code, message, emitted line and call site", () => {
  const b = new SpecBuilder();
  b.instances();
  const line = here() + 1;
  b.read("after", int);
  const error = buildError(b);
  const { code, message } = specError(error);
  assert.equal(
    error.message,
    `${code}: builder.iris:2: ${message}\n     2 | after: int\ncall site: ${FILE}:${line}:5`,
  );
  assert.equal(error.callSite?.column, 5);
});

test("a failed check names the check in its block", () => {
  const b = new SpecBuilder("row");
  const n = b.read("n", int);
  let line = 0;
  b.repeat(n, (s) => {
    const x = s.read("x", int);
    line = here() + 1;
    s.check(x.gt(0), "not positive");
  });
  const error = readError(b.build(), "2 1 -1");
  assert.equal(error.code, "E-CHECK-FAILED");
  assertHere(error.callSite, line);
  assert.ok(error.diagnostic.includes("\nspec: builder.iris:5"));
  assert.ok(error.diagnostic.endsWith(`\ncall site: ${FILE}:${line}:7`));
});

test("a read failure names the declaration that read", () => {
  const b = new SpecBuilder();
  const line = here() + 1;
  b.read("n", int);
  assertHere(readError(b.build(), "word").callSite, line);
});

test("a catalog failure names the statement", () => {
  const b = new SpecBuilder();
  const line = here() + 1;
  b.read("n", int);
  b.instances();
  const built = b.build();
  assert.throws(
    () => built.catalog("word", "input.txt"),
    (error) => {
      assert.ok(error instanceof IrisError);
      assertHere(error.callSite, line);
      return true;
    },
  );
});

test("a deferred rejection keeps its call site", () => {
  const b = new SpecBuilder();
  let line = 0;
  b.instances((s) => {
    const n = s.read("n", int);
    line = here() + 1;
    s.check(n.gt(0));
  });
  const error = readError(b.build(), "0");
  assert.equal(error.code, "E-CHECK-FAILED");
  assertHere(error.callSite, line);
});

test("a failure without a statement has no call site", () => {
  const b = new SpecBuilder();
  b.read("n", int);
  const error = readError(b.build(), "1 2");
  assert.equal(error.code, "E-TRAILING-INPUT");
  assert.equal(error.callSite, null);
  assert.ok(!error.diagnostic.includes("call site"));
});

test("a spec parsed from text has no call sites", () => {
  // The same text, parsed and built, differs only in the call site.
  const b = new SpecBuilder();
  const n = b.read("n", int);
  b.check(n.gt(0), "not positive");
  const built = b.build();
  const parsed = Spec.parse(b.source(), "builder.iris");
  const fromText = readError(parsed, "0");
  const fromBuilder = readError(built, "0");
  assert.equal(fromText.callSite, null);
  assert.deepEqual(
    [fromText.code, fromText.message, fromText.inputLocation, fromText.specLocation],
    [fromBuilder.code, fromBuilder.message, fromBuilder.inputLocation, fromBuilder.specLocation],
  );
  assert.equal(fromText.toJSON(), fromBuilder.toJSON());
  assert.ok(fromBuilder.callSite !== null);
  assert.equal(
    `${fromText.diagnostic}\ncall site: ${FILE}:${fromBuilder.callSite.line}:${fromBuilder.callSite.column}`,
    fromBuilder.diagnostic,
  );
  assert.throws(
    () => Spec.parse("instances {}\nafter: int\n", "builder.iris"),
    (error) => {
      assert.ok(error instanceof IrisError);
      assert.equal(error.callSite, null);
      assert.ok(!error.diagnostic.includes("call site"));
      return true;
    },
  );
});

test("a builder check names the statement that failed it", () => {
  const b = new SpecBuilder();
  b.read("x", int);
  const line = here() + 2;
  assert.throws(
    () => b.read("x", int),
    (error) => {
      assert.ok(error instanceof BuildError);
      assert.equal(error.kind, "redefinition");
      assertHere(error.callSite, line);
      assert.ok(error.message.endsWith(`\ncall site: ${FILE}:${line}:${error.callSite?.column}`));
      return true;
    },
  );
  function namesTheStatement(): void {
    b.read("x", int);
  }
  let site: CallSite | null = null;
  try {
    namesTheStatement();
  } catch (error) {
    assert.ok(error instanceof BuildError);
    site = error.callSite;
  }
  assert.equal(site?.function, "namesTheStatement");
});

test("a handle from another builder names the statement that used it", () => {
  const other = new SpecBuilder();
  const foreign = other.read("x", int);
  const b = new SpecBuilder();
  const line = here() + 2;
  assert.throws(
    () => b.let("y", foreign),
    (error) => {
      assert.ok(error instanceof BuildError);
      assert.equal(error.kind, "provenance");
      assertHere(error.callSite, line);
      return true;
    },
  );
});

test("a handle never bound names the call that made it", () => {
  let b = new SpecBuilder();
  let line = here() + 1;
  b.forward("later", "int");
  const error = buildError(b);
  assert.equal(error.kind, "binding");
  assertHere(error.callSite, line);

  b = new SpecBuilder();
  line = here() + 1;
  b.capture("n", int);
  assertHere(buildError(b).callSite, line);

  b = new SpecBuilder();
  line = here() + 1;
  b.forwardRecord("each", "list");
  assertHere(buildError(b).callSite, line);
});

test("a binding failure names the definition", () => {
  const b = new SpecBuilder();
  const name = b.undefined("name");
  const line = here() + 1;
  b.read(name, int);
  const error = buildError(b);
  assert.equal(error.kind, "binding");
  assertHere(error.callSite, line);
});

test("an unprintable value names the statement that holds it", () => {
  const b = new SpecBuilder();
  b.read("x", int);
  const line = here() + 1;
  b.let("y", floatLit(Number.NaN));
  const error = buildError(b);
  assert.equal(error.kind, "unprintable");
  assertHere(error.callSite, line);
});

test("a guard used late names the guard call", () => {
  const b = new SpecBuilder();
  const guard = b.if(1, () => {});
  b.read("m", int);
  const line = here() + 2;
  assert.throws(
    () => guard.elseIf(1, () => {}),
    (error) => {
      assert.ok(error instanceof BuildError);
      assert.equal(error.kind, "scope");
      assertHere(error.callSite, line);
      return true;
    },
  );
});

function withoutTracking(s: Scope): number {
  const line = here() + 1;
  s.read("x", int);
  return line;
}

const withTracking = trackCaller((s: Scope) => {
  s.read("x", int);
});

const outerTracking = trackCaller((s: Scope) => {
  withTracking(s);
});

test("a helper records its own line unless it tracks its caller", () => {
  let b = new SpecBuilder();
  let line = withoutTracking(b);
  assertHere(readError(b.build(), "word", "i").callSite, line);

  b = new SpecBuilder();
  line = here() + 1;
  withTracking(b);
  assertHere(readError(b.build(), "word", "i").callSite, line);

  b = new SpecBuilder();
  line = here() + 1;
  outerTracking(b);
  assertHere(readError(b.build(), "word", "i").callSite, line);

  // After a tracked helper returns, statements record their own lines again.
  b = new SpecBuilder();
  withTracking(new SpecBuilder());
  line = here() + 1;
  b.read("x", int);
  assertHere(readError(b.build(), "word", "i").callSite, line);
});

test("trackCaller takes only functions", () => {
  // @ts-expect-error not a function
  assert.throws(() => trackCaller(1), TypeError);
});

test("a method used as a function value records its caller", () => {
  // The call site comes from the stack, so a method value records the line
  // that called it.
  const b = new SpecBuilder();
  const skip = b.skipLine.bind(b);
  const line = here() + 1;
  skip();
  b.read("n", int);
  assertHere(readError(b.build(), "").callSite, line);
});

/** What each statement's text starts with and the line of its call. */
class Expected {
  readonly starts: [string, number][] = [];

  /** Expects a statement starting with `start` from the caller's line, or `below` lines under it. */
  at(start: string, below = 0): void {
    this.starts.push([start, lineOf(1) + below]);
  }
}

/** Checks each statement's call site, in text order. */
function assertSites(b: SpecBuilder, expected: Expected): void {
  const text = b.source();
  const sites = callSitesOf(b);
  const data = Buffer.from(text);
  let cursor = 0;
  for (const [start, line] of expected.starts) {
    const offset = data.indexOf(start, cursor);
    assert.ok(offset >= 0, `no \`${start}\` after byte ${cursor} of:\n${text}`);
    const site = sites.at(offset);
    assert.deepEqual([start, site?.file, site?.line], [start, FILE, line], text);
    cursor = offset + Buffer.byteLength(start);
  }
}

test("every setting and plain statement records its call", () => {
  const e = new Expected();
  // prettier-ignore
  {
    e.at("format first"); const b = new SpecBuilder("first");
    e.at("format second"); b.format("second");
    e.at("comment"); b.comment("#");
    e.at("stop"); b.stop("END");
    e.at("tokens"); b.tokens(":");
    e.at("separators"); b.separators(",");
    e.at("ignore trailing"); b.ignoreTrailing();
    e.at("param p"); b.param("p");
    e.at("param q"); b.param("q", 1);
    e.at("type t"); b.defineType("t", int);
    e.at("n: int"); const n = b.read("n", int);
    e.at("list l"); b.list("l", int.until(0));
    e.at("let v"); const v = b.let("v", 1);
    e.at("let v"); b.set(v, 2);
    e.at("check"); b.check(n.ge(0));
    e.at("expect"); b.expect("word");
    e.at("skip line"); b.skipLine();
    e.at("skip 1 lines"); b.skipLines(1);
    const m = b.capture("m", word);
    e.at("line"); b.line(p`m ${m}`);
    e.at("find"); b.find("x");
    e.at("index"); b.index(0);
    assertSites(b, e);
  }
});

test("every block statement records its call", () => {
  const e = new Expected();
  const b = new SpecBuilder();
  // prettier-ignore
  {
    e.at("n: int"); const n = b.read("n", int);
    e.at("if n", 1);
    b.if(n.gt(0), (s) => {
      e.at("a: int"); s.read("a", int);
      e.at("else if n", 2);
    })
      .elseIf(n.lt(0), (s) => {
        e.at("b: int"); s.read("b", int);
        e.at("else", 2);
      })
      .else((s) => {
        e.at("c: int"); s.read("c", int);
      });
    e.at("repeat 2", 1);
    b.repeat(2, (s) => {
      e.at("d: int"); s.read("d", int);
    });
    e.at('repeat until "END"', 1);
    b.repeatUntil("END", (s) => {
      e.at("f: int"); s.read("f", int);
    });
    e.at("sections", 1);
    b.sections((arms) => {
      e.at('"one"', 1);
      arms.arm("one", (s) => {
        e.at("h: int"); s.read("h", int);
      });
    });
    e.at("repeat until eof", 1);
    b.repeatUntilEof((s) => {
      e.at("i: float"); s.read("i", float);
    });
    e.at("instances", 1);
    b.instances((s) => {
      e.at("j: int"); s.read("j", int);
    });
  }
  assertSites(b, e);
});

test("the guards of if record their calls", () => {
  const e = new Expected();
  const b = new SpecBuilder();
  const n = b.read("n", int);
  const guard = b.if(n.gt(0), () => {});
  // prettier-ignore
  {
    e.at("else if n"); const next = guard.elseIf(n.lt(0), () => {});
    e.at("else if:"); next.elseReadIf("if", int);
  }
  assertSites(b, e);
});

test("a closing brace belongs to the call that wrote the block", () => {
  const b = new SpecBuilder();
  const line = here() + 1;
  b.repeat(2, (s) => {
    s.read("x", int);
  });
  b.read("y", int);
  assertHere(callSitesOf(b).at(b.source().indexOf("}")), line);
});

test("an empty block gives its brace to the call that wrote it", () => {
  let b = new SpecBuilder();
  let line = here() + 1;
  b.repeatUntil("END", () => {});
  const text = b.source();
  assert.equal(text, 'repeat until "END" {}\n');
  assertHere(callSitesOf(b).at(text.indexOf("}")), line);

  b = new SpecBuilder();
  line = here() + 1;
  b.repeatUntil("END");
  assertHere(callSitesOf(b).at(b.source().indexOf("}")), line);
});

test("call sites count bytes of the text", () => {
  const b = new SpecBuilder();
  b.line("ünïcödé ✓");
  const line = here() + 1;
  b.read("n", int);
  assertHere(readError(b.build(), "ünïcödé ✓\nword").callSite, line);
});

/**
 * A declaration's form is checked at the token after it, which may start
 * another statement. The location stays there; the call site does not.
 */
function assertOwner(
  error: BuildError,
  line: number,
  emittedLine: number,
  emitted: string | null,
): void {
  const rejected = specError(error);
  assert.equal(rejected.code, "E-SPEC-TYPE");
  assert.equal(rejected.message, "'rest' values cannot be arrays");
  assertHere(error.callSite, line);
  assertHere(rejected.callSite, line);
  assert.equal(rejected.specLocation?.line, emittedLine);
  assert.equal(error.emittedLine, emitted);
  assert.ok(error.text !== null);
  const text = error.text;
  assert.throws(
    () => Spec.parse(text, "builder.iris"),
    (parsed) => {
      assert.ok(parsed instanceof IrisError);
      assert.equal(parsed.callSite, null);
      assert.deepEqual(
        [parsed.code, parsed.message, parsed.specLocation?.line],
        [rejected.code, rejected.message, rejected.specLocation?.line],
      );
      return true;
    },
  );
}

test("an error on the next statement names the statement it belongs to", () => {
  const b = new SpecBuilder();
  const line = here() + 1;
  b.read("bad", rest.dim(2));
  b.read("next", int);
  assertOwner(buildError(b), line, 2, "next: int");
});

test("an error at the end of a nested block names the statement in it", () => {
  const b = new SpecBuilder();
  let line = 0;
  b.repeat(1, (s) => {
    s.repeat(1, (t) => {
      line = here() + 1;
      t.read("bad", rest.dim(2));
    });
  });
  assertOwner(buildError(b), line, 4, "  }");
});

test("an error at the end of an else block names the statement in it", () => {
  const b = new SpecBuilder();
  let line = 0;
  b.if(lit(1).eq(1), () => {}).else((s) => {
    line = here() + 1;
    s.read("bad", rest.dim(2));
  });
  assertOwner(buildError(b), line, 3, "}");
});

test("an error at the end of the spec names the last statement", () => {
  const b = new SpecBuilder();
  b.read("n", int);
  const line = here() + 1;
  b.read("bad", rest.dim(2));
  assertOwner(buildError(b), line, 3, null);
});

test("an error after a brace names the block that opened it", () => {
  const lines = { read: 0, block: 0 };
  function nest(s: Scope<HandleDepth, Presence>, depth: number): void {
    if (depth === 0) {
      lines.read = here() + 1;
      s.read("x", int);
    } else {
      lines.block = here() + 1;
      s.if(lit(1).eq(1), (inner: Scope<HandleDepth, Presence>) => nest(inner, depth - 1));
    }
  }
  const b = new SpecBuilder();
  b.if(lit(1).eq(1), (s) => nest(s, 256));
  const error = buildError(b);
  assert.equal(specError(error).code, "E-SPEC-NESTING");
  assert.equal(error.emittedLine?.trim(), "x: int");
  assertHere(error.callSite, lines.block);
});

test("an error in an arm names the arm", () => {
  const b = new SpecBuilder();
  const cap = b.capture("bad", alphabet("ab"));
  let line = 0;
  b.sections((arms) => {
    line = here() + 1;
    arms.arm(pattern(cap));
  });
  const error = buildError(b);
  assert.equal(specError(error).code, "E-SPEC-PATTERN");
  assertHere(error.callSite, line);
});

test("a resolution error in an arm names the arm", () => {
  const b = new SpecBuilder();
  const outside = b.read("outside", int);
  const record = b.repeat(2, (s) => {
    s.eachName("row");
    s.read("inside", int);
    return s.record;
  });
  const cap = b.capture("cap", range(0).below(record.get(outside)));
  let line = 0;
  b.sections((arms) => {
    line = here() + 1;
    arms.arm(pattern(cap));
  });
  const error = buildError(b);
  assert.equal(specError(error).code, "E-SPEC-RECORD-SCOPE");
  assert.equal(error.emittedLine, "sections {");
  assertHere(error.callSite, line);
});

test("a store failure names the statement that stores", () => {
  const b = new SpecBuilder();
  const stored = b.forward("stored", "int");
  const line = here() + 1;
  b.repeatUntil("END", { store: stored }, (s) => {
    s.read("x", int);
  });
  const error = buildError(b);
  assert.equal(error.kind, "binding");
  assertHere(error.callSite, line);
});

test("a repeat that reads nothing names the repeat", () => {
  const b = new SpecBuilder();
  const line = here() + 1;
  b.repeatUntilEof((s) => {
    s.let("x", 1);
  });
  const error = readError(b.build(), "1");
  assert.equal(error.code, "E-NO-PROGRESS");
  assert.ok(error.diagnostic.includes("\nspec: builder.iris:2"));
  assertHere(error.callSite, line);
});

test("a column failure names the repeat", () => {
  const b = new SpecBuilder();
  const line = here() + 1;
  b.repeat(2, (s) => {
    const x = s.read("x", int);
    s.if(x.gt(0), (t) => {
      t.read("y", int);
    });
    s.read("z", int);
  });
  const error = readError(b.build(), "1 2 3 0 4");
  assert.equal(error.code, "E-REPEAT-MISSING");
  assert.ok(error.diagnostic.includes("\nspec: builder.iris:6"));
  assertHere(error.callSite, line);
});

test("a read failure in an arm names the arm", () => {
  const b = new SpecBuilder();
  const n = b.capture("n", range(1, 2));
  let line = 0;
  b.sections((arms) => {
    line = here() + 1;
    arms.arm(p`n = ${n}`);
  });
  const error = readError(b.build(), "n = 3\n");
  assert.equal(error.code, "E-EXPECTED-VALUE");
  assert.ok(error.diagnostic.includes("\nspec: builder.iris:1"));
  assertHere(error.callSite, line);
});

test("a line a last arm left unread names its arm", () => {
  const b = new SpecBuilder();
  let line = 0;
  b.sections((arms) => {
    line = here() + 1;
    arms.arm("needed");
    arms.arm("end", { last: true }, (s) => {
      s.read("x", int);
    });
  });
  const error = readError(b.build(), "end\n1\n");
  assert.equal(error.code, "E-LINE-MATCH");
  assert.ok(error.diagnostic.includes("\nspec: builder.iris:4"));
  assertHere(error.callSite, line);
});

test("a line no arm matches names the sections", () => {
  const b = new SpecBuilder();
  const line = here() + 1;
  b.sections((arms) => {
    arms.arm("known");
  });
  const error = readError(b.build(), "other\n");
  assert.equal(error.code, "E-SECTIONS-UNEXPECTED");
  assertHere(error.callSite, line);
});

test("a failure after sections end has no call site", () => {
  const b = new SpecBuilder();
  b.sections((arms) => {
    arms.arm("end", { last: true });
  });
  const error = readError(b.build(), "end\nmore");
  assert.equal(error.code, "E-TRAILING-INPUT");
  assert.equal(error.callSite, null);
});

test("a built spec's reads name the caller through spec()", () => {
  // `spec` records the line of each statement in the callback, and the
  // callback's own statements record theirs.
  let line = 0;
  const built = spec((b) => {
    line = here() + 1;
    return b.read("n", int);
  });
  assertHere(readError(built.spec, "word").callSite, line);
});

test("every public scope method records its caller first", () => {
  // Each method that writes or checks a statement calls `caller()` before
  // anything else, so the frame above it is the user's.
  const file = readFileSync(new URL("../src/build/scope.ts", import.meta.url), "utf8");
  const source = file.slice(0, file.indexOf("export class BuiltSpec"));
  const lines = source.split("\n");
  const checked = new Set<string>();
  lines.forEach((line, index) => {
    const name = /^ {2}([a-z][A-Za-z]*)[<(]/u.exec(line)?.[1];
    if (name === undefined || ["constructor", "source", "build"].includes(name)) {
      return;
    }
    // The header ends at `;` for an overload or at ` {` for the body.
    let end = index;
    while (!/(?:;| \{)$/u.test(lines[end] ?? ";")) {
      end += 1;
    }
    if (lines[end]?.endsWith(" {") === true) {
      assert.match(lines[end + 1] ?? "", /^ {4}const site = caller\(\);$/u, name);
      checked.add(name);
    }
  });
  for (const name of ["read", "list", "let", "set", "param", "check", "repeat", "if", "arm"]) {
    assert.ok(checked.has(name), `${name} is checked`);
  }
});

test("frames of V8, SpiderMonkey and JavaScriptCore parse", () => {
  assert.deepEqual(parseFrame("    at build (file:///app/spec.ts:12:7)"), {
    file: "file:///app/spec.ts",
    line: 12,
    column: 7,
    function: "build",
  });
  assert.deepEqual(parseFrame("    at file:///app/spec.ts:3:1"), {
    file: "file:///app/spec.ts",
    line: 3,
    column: 1,
    function: null,
  });
  assert.deepEqual(parseFrame("    at async run (http://host/a.js:1:2)"), {
    file: "http://host/a.js",
    line: 1,
    column: 2,
    function: "run",
  });
  assert.deepEqual(parseFrame("build@http://host/spec.js:4:9"), {
    file: "http://host/spec.js",
    line: 4,
    column: 9,
    function: "build",
  });
  assert.deepEqual(parseFrame("@http://host/spec.js:5:1"), {
    file: "http://host/spec.js",
    line: 5,
    column: 1,
    function: null,
  });
  assert.equal(parseFrame("Error"), null);
  assert.equal(parseFrame("    at Array.map (<anonymous>)"), null);
});
