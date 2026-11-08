// Typed access through handles: `data.get`, `spec()` and its typed reads,
// empty arrays, absence, the errors a handle from another spec can meet, and
// a builder for the jobshop format judged against its cases. Ports
// python/tests/test_build_access.py and test_build_patterns.py.
import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { IrisError, type Data, type Params, type Value } from "gecode-iris";
import {
  atLeast,
  BuildError,
  type ArmOptions,
  type ForwardOptions,
  distinct,
  div,
  float,
  gecode,
  int,
  num,
  over,
  p,
  pattern,
  range,
  spec,
  SpecBuilder,
  sum,
  validate,
  word,
} from "gecode-iris/build";

function read(build: (b: SpecBuilder) => unknown, input: string): Data {
  const b = new SpecBuilder();
  build(b);
  return b.build().read(input);
}

test("an empty array has the typed empty view", () => {
  const b = new SpecBuilder();
  const handles = [
    b.read("ints", int.dim(0)),
    b.read("floats", float.dim(0)),
    b.read("strs", word.dim(0)),
    b.read("nums", num.dim(0)),
    b.read("rows", float.dims(0, 3)),
  ];
  const data = b.build().read("");
  for (const handle of handles) {
    assert.deepEqual(data.get(handle), []);
  }
});

test("a repeat of nothing collects an empty array", () => {
  const b = new SpecBuilder();
  const x = b.repeat(0, (r) => r.read("x", word));
  assert.deepEqual(b.build().read("").get(x), []);
});

test("rows nest one level per dimension", () => {
  const b = new SpecBuilder();
  const x = b.read("x", float.dims(2, 2));
  assert.deepEqual(b.build().read("1 2 3 4").get(x), [
    [1, 2],
    [3, 4],
  ]);
});

test("an absent optional entry is undefined", () => {
  const b = new SpecBuilder();
  const n = b.read("n", int);
  const x = b.if(n, (t) => t.read("x", int)).result;
  assert.equal(x.presence, "maybe");
  const data = b.build().read("0");
  assert.equal(data.get(x), undefined);
  assert.equal(data.has("x"), false);
});

test("numbers keep the type the data gave", () => {
  const b = new SpecBuilder();
  const row = b.read("row", num.dim(2));
  const built = b.build();
  assert.deepEqual(built.read("1 2").get(row, { bigint: true }), [1n, 2n]);
  // One float makes the whole entry float.
  assert.deepEqual(built.read("1 2.5").get(row, { bigint: true }), [1, 2.5]);
  assert.deepEqual(built.read("1 2").get(row), [1, 2]);
});

test("integers read as bigints on request", () => {
  const b = new SpecBuilder();
  const big = b.read("big", int);
  const data = b.build().read("9223372036854775807");
  assert.equal(data.get(big, { bigint: true }), 9223372036854775807n);
  assert.throws(() => data.get(big), RangeError);
});

test("a handle of another spec reports what went wrong", () => {
  const data = read((b) => b.read("x", int), "1");
  const other = new SpecBuilder();
  const missing = other.read("missing", int);
  const wrongType = other.read("x", word);
  const wrongDepth = other.repeat(1, (r) => r.read("x2", int));
  const absent = other.if(1, (t) => t.read("absent", int)).result;
  const loose = other.let(wrongType, 1);
  assert.throws(() => data.get(missing), {
    name: "RangeError",
    message: /unknown entry 'missing'/u,
  });
  assert.throws(() => data.get(wrongType), {
    name: "TypeError",
    message:
      "entry 'x' does not match its handle: the handle holds string values, but the entry holds integer values",
  });
  assert.equal(data.get(absent), undefined);
  assert.equal(data.get(loose), 1, "a dynamic handle reads any value");
  const deep = new SpecBuilder().read("x", int.dim(1));
  assert.throws(() => data.get(deep), {
    name: "TypeError",
    message:
      "entry 'x' does not match its handle: the handle has depth 1, but the entry has depth 0",
  });
  assert.equal(wrongDepth.name, "x2");
});

test("an entry with no values matches any value type", () => {
  const data = read((b) => b.read("x", int.dim(0)), "");
  const words = new SpecBuilder().read("x", word.dim(1));
  assert.deepEqual(data.get(words), []);
});

test("data.get takes any value with the handle shape", () => {
  const data = read((b) => b.read("x", int), "5");
  const handle = { name: "x", valueType: "int", depth: 0, presence: "required" } as const;
  const value: number = data.get(handle);
  assert.equal(value, 5);
});

test("handles name their entries", () => {
  const b = new SpecBuilder();
  const x = b.read("x", int);
  assert.deepEqual([x.name, x.valueType, x.depth, x.presence], ["x", "int", 0, "required"]);
  const rows = b.repeat(2, (r) => r.read("row", float.dim(3)));
  assert.deepEqual([rows.valueType, rows.depth], ["float", 2]);
  const typed = b.forward("later", "string", { depth: 2, optional: true });
  assert.deepEqual([typed.valueType, typed.depth, typed.presence], ["string", 2, "maybe"]);
  b.if(1, (t) => {
    t.repeat(1, (r) => {
      r.read(typed, word.dim(1));
    });
  });
  b.build();
});

test("spec() reads the handles it returns", () => {
  const built = spec("pair", (b) => {
    const n = b.read("n", int);
    const values = b.read("values", float.dim(n));
    const label = b.if(n.gt(1), (t) => t.read("label", word)).result;
    return { n, values, label, note: "not a handle" };
  });
  assert.equal(
    built.source,
    "format pair\nn: int\nvalues[n]: float\nif n > 1 {\n  label: word\n}\n",
  );
  const data = built.read("2 1.5 2.5 ok");
  assert.deepEqual(data, { n: 2, values: [1.5, 2.5], label: "ok" });
  assert.deepEqual(built.read("1 3"), { n: 1, values: [3], label: undefined });
  assert.equal(built.handles.note, "not a handle");
  const single = spec((b) => b.read("x", int));
  assert.equal(single.read("4"), 4);
});

test("spec() reads the structural handles it returns", () => {
  const handle = { name: "n", valueType: "int", depth: 0, presence: "required" } as const;
  const single = spec((b) => {
    b.read("n", int);
    return handle;
  });
  const value: number = single.read("7");
  assert.equal(value, 7);
  const object = spec((b) => {
    b.read("n", int);
    return { n: handle, note: { name: "n" } };
  });
  const data: { n: number } = object.read("7");
  assert.deepEqual(data, { n: 7 });
});

test("spec() keeps a handle under the key __proto__ as an ordinary property", () => {
  const scalar = spec((b) => ({ ["__proto__"]: b.read("n", int) })).read("7");
  assert.deepEqual(Object.keys(scalar), ["__proto__"]);
  assert.equal(scalar.__proto__, 7);
  const array = spec((b) => ({ ["__proto__"]: b.read("n", int.dim(2)) })).read("7 8");
  assert.equal(Object.getPrototypeOf(array), Object.prototype);
  assert.deepEqual(array.__proto__, [7, 8]);
});

test("a forward handle with options of a wide type promises little", () => {
  const options: ForwardOptions = { optional: true };
  const maybe = spec((b) => {
    const handle = b.forward("maybe", "int", options);
    b.if(0, (t) => t.read(handle, int));
    return handle;
  });
  const absent: Value | undefined = maybe.read("");
  assert.equal(absent, undefined);
  const depthOptions: ForwardOptions = { depth: 2 };
  const grid = spec((b) => {
    const handle = b.forward("grid", "int", depthOptions);
    b.read(handle, int.dims(2, 2));
    return handle;
  });
  assert.deepEqual(grid.read("1 2 3 4"), [
    [1, 2],
    [3, 4],
  ]);
  const armOptions: ArmOptions = { optional: true };
  const arm = spec((b) => b.sections((s) => s.arm("X", armOptions, (a) => a.read("x", int))));
  const missing: number | undefined = arm.read("");
  assert.equal(missing, undefined);
});

test("spec() reads instances and catalogs with parameters", () => {
  const built = spec((b) =>
    b.instances((s) => {
      s.line(p`instance ${s.capture("name")}`);
      return s.read("x", int);
    }),
  );
  const input = "instance a\n1\ninstance b\n2\n";
  assert.equal(built.read(input, { instance: "b" }), 2);
  const catalog = built.catalog(input);
  assert.equal(catalog.size("name"), 2);
});

test("spec() throws what the builder throws", () => {
  assert.throws(
    () =>
      spec((b) => {
        b.read("x", int);
        b.read("x", int);
      }),
    BuildError,
  );
  // @ts-expect-error spec needs a function
  assert.throws(() => spec("name"), TypeError);
});

test("a pattern joins text and parts", () => {
  const b = new SpecBuilder();
  const x = b.capture("x", int);
  const y = b.capture("y");
  b.line(pattern("x = ", x, ", ", "y = ", y, ""));
  assert.equal(b.source(), 'line "x = {x:int}, y = {y}"\n');
  const data = b.build().read("x = 1, y = two\n");
  assert.deepEqual([data.get(x), data.get(y)], [1, "two"]);
  const empty = new SpecBuilder();
  empty.line(pattern());
  assert.equal(empty.source(), 'line ""\n');
});

test("braces and dots in text are rejected", () => {
  for (const text of ["a { b", "a ... b", "{", "..."]) {
    const b = new SpecBuilder();
    assert.throws(
      () => {
        b.line(pattern("", text));
        b.source();
      },
      (error) => error instanceof BuildError && error.kind === "unprintable",
    );
  }
});

test("a pattern part has a pattern type", () => {
  const b = new SpecBuilder();
  // @ts-expect-error a number is not a pattern part
  assert.throws(() => b.line(pattern("x", 1)), { name: "TypeError", message: /pattern part/u });
  // @ts-expect-error an array is not a pattern
  assert.throws(() => b.line(["x"]), { name: "TypeError", message: /a pattern is/u });
  // @ts-expect-error a type with dimensions is not a scalar
  assert.throws(() => validate(int.dim(2)), { name: "TypeError", message: /no dimensions/u });
});

function program(): string {
  const b = new SpecBuilder("determinism");
  const n = b.capture("n", atLeast(1));
  b.line(pattern(n));
  const [record, x] = b.repeat(n, (r) => [r.record, r.read("x", int)] as const);
  b.check(sum(record, (a) => sum(record, (c) => a.get(x).dynamic().times(c.get(x)))).ge(0));
  return b.source();
}

test("the same program prints the same text", () => {
  assert.equal(program(), program());
});

/** The jobshop format, as formats/jobshop/jobshop.iris writes it. */
function jobshop() {
  return spec("jobshop", (b) => {
    b.comment("#", "+++");
    return b.instances((s) => {
      const name = s.optionalCapture("name", word);
      const nJobs = s.capture("n_jobs", atLeast(1));
      const nMachines = s.capture("n_machines", atLeast(1));
      const { job, machine, duration } = s.sections({ skipOther: true }, (arms) => {
        arms.arm(p`instance ${name}`, { optional: true });
        return arms.arm(p`${nJobs} ${nMachines}`, { last: true }, (t) => {
          t.check(
            nJobs.le(div(div(gecode.intMax.minus(2), 2), nMachines)),
            "there are too many jobs and machines",
          );
          return t.repeat(nJobs, (jobs) => {
            jobs.asName("jobs");
            jobs.eachName("job");
            const [machines, durations] = jobs.repeat(nMachines, (steps) => {
              steps.asName("steps");
              steps.eachName("step");
              return [
                steps.read("machine", range(0).below(nMachines)),
                steps.read("duration", atLeast(0)),
              ] as const;
            });
            return { job: jobs.record, machine: machines, duration: durations };
          });
        });
      });
      s.check(
        distinct(job.get(machine)),
        "a job uses a machine twice; each job must use every machine once",
      );
      s.check(
        sum(over(job, "j"), (j) => sum(j.get(duration))).le(gecode.intMax),
        "the total duration is too large",
      );
      return { name, nJobs, nMachines, machine, duration };
    });
  });
}

/** The jobshop example of the TypeScript section of docs/builders.md. */
function guideJobshop() {
  return spec("jobshop", (b) => {
    b.comment("#", "+++");
    return b.instances((i) => {
      const name = i.optionalCapture("name", word);
      const nJobs = i.capture("n_jobs", atLeast(1));
      const nMachines = i.capture("n_machines", atLeast(1));
      const { job, machine, duration } = i.sections({ skipOther: true }, (s) => {
        s.arm(p`instance ${name}`, { optional: true });
        return s.arm(p`${nJobs} ${nMachines}`, { last: true }, (a) => {
          a.check(
            nJobs.le(div(div(gecode.intMax.minus(2), 2), nMachines)),
            "there are too many jobs and machines",
          );
          return a.repeat(nJobs, (r) => {
            r.asName("jobs");
            r.eachName("job");
            return {
              job: r.record,
              ...r.repeat(nMachines, (step) => ({
                machine: step.read("machine", range(0).below(nMachines)),
                duration: step.read("duration", atLeast(0)),
              })),
            };
          });
        });
      });
      i.check(distinct(job.get(machine)), MSG_DISTINCT);
      i.check(sum(job, (j) => sum(j.get(duration))).le(gecode.intMax), MSG_TOTAL);
      return { name, nJobs, nMachines, machine, duration };
    });
  });
}

const MSG_DISTINCT = "a job uses a machine twice; each job must use every machine once";
const MSG_TOTAL = "the total duration is too large";

for (const [label, built] of [
  ["the jobshop builder", jobshop()],
  ["the guide's jobshop example", guideJobshop()],
] as const) {
  test(`${label} reads every jobshop case as the format does`, () => {
    readsJobshopCases(built);
  });
}

function readsJobshopCases(built: ReturnType<typeof jobshop>): void {
  const root = new URL("../../formats/jobshop/cases/", import.meta.url);
  const cases = readdirSync(root);
  assert.ok(cases.length >= 3);
  for (const name of cases) {
    const at = (file: string) => new URL(`${name}/${file}`, root);
    const input = readFileSync(at("input.txt"));
    const params: Params = existsSync(at("params.json"))
      ? JSON.parse(readFileSync(at("params.json"), "utf8"))
      : {};
    if (existsSync(at("expected.json"))) {
      const expected: unknown = JSON.parse(readFileSync(at("expected.json"), "utf8"));
      const data = built.spec.read(input, undefined, { params });
      assert.deepEqual(JSON.parse(data.toJSON()), expected, name);
      const typed = built.read(input, params);
      assert.deepEqual(typed.machine, data.get(built.handles.machine), name);
    } else {
      const expected: unknown = JSON.parse(readFileSync(at("error.json"), "utf8"));
      assert.throws(
        () => built.read(input, params),
        (error) => {
          assert.ok(error instanceof IrisError);
          assert.ok(error.callSite !== null, "the failure names the check");
          assert.deepEqual(
            { code: error.code, message: error.message },
            {
              code: Reflect.get(Object(expected), "code"),
              message: Reflect.get(Object(expected), "message"),
            },
            name,
          );
          return true;
        },
      );
    }
  }
}

test("the jobshop builder's handles have the types of its entries", () => {
  const { handles } = jobshop();
  assert.deepEqual(
    [handles.name.presence, handles.nJobs.presence, handles.machine.depth],
    ["maybe", "required", 2],
  );
});
