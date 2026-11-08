//! The Kotlin text of each production, each identifier rule and each layout
//! rule that the backend takes from ktfmt's kotlinlang style.
use gecode_iris_gen::{Language, builder_code, view_code};
use gecode_iris_sema::analyze;
use gecode_iris_syntax::{LanguageVersion, parse};

fn code(source: &str) -> String {
    let parsed = parse(source.as_bytes(), "test.iris", LanguageVersion::V0_1).unwrap();
    let analysis = analyze(&parsed);
    builder_code(&parsed, &analysis, Language::Kotlin).expect("the spec is accepted")
}

/// The builder of `source`, which must be finished within seconds: an
/// identifier search that never ends would otherwise hang the test.
fn code_in_time(source: &str) -> String {
    let source = source.to_owned();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || sender.send(code(&source)));
    receiver
        .recv_timeout(std::time::Duration::from_secs(10))
        .unwrap_or_else(|error| panic!("the builder is not finished: {error}"))
}

/// The prefixes of the numbered names that some backend makes, such as
/// `e1`, `s1`, `c1`, `block1` and `block_1`.
const NUMBERED_PREFIXES: &[&str] = &["e", "s", "a", "t", "c", "block", "body"];

/// Specs where a bare prefix, such as `e`, names two things once a leading
/// underscore is gone: values, a type and a value, and a comprehension
/// variable and a value.
fn prefix_collisions(prefix: &str) -> [String; 3] {
    [
        format!("_{prefix}: int\n{prefix}: int\n"),
        format!("type _{prefix} = int\n{prefix}: _{prefix}\n"),
        format!("x[2]: int\ncheck all({prefix} in x: {prefix} > 0)\nlet _{prefix} = 1\n"),
    ]
}

/// Whether the builder of each spec holds its expected lines, in order, with
/// their indentation relative to the first of them.
#[track_caller]
fn assert_statements(cases: &[(&str, &[&str])]) {
    for (source, statements) in cases {
        let code = code(source);
        let lines: Vec<&str> = code.lines().collect();
        let found = lines.windows(statements.len()).any(|window| {
            let indent = window[0].len() - window[0].trim_start().len();
            window.iter().zip(*statements).all(|(line, expected)| {
                if line.is_empty() {
                    expected.is_empty()
                } else {
                    line.get(indent..) == Some(*expected)
                }
            })
        });
        assert!(found, "{source:?} lacks {statements:?} in:\n{code}");
    }
}

/// The text of a spec with `source` repeated `count` times.
fn repeated(source: &str, count: usize) -> String {
    source.repeat(count)
}

#[test]
fn the_program_defines_a_builder_and_prints_its_source() {
    assert_eq!(
        code("format dimacs\nskip line\n"),
        "import dev.gecode.iris.kotlin.build.IrisBuilder\n\
         import dev.gecode.iris.kotlin.build.irisBuilder\n\n\
         fun specBuilder(): IrisBuilder<Unit> =\n    \
         irisBuilder(\"dimacs\") {\n        \
         skipLine()\n    \
         }\n\n\
         fun main() {\n    \
         print(specBuilder().source())\n\
         }\n"
    );
    assert_eq!(
        code("skip line\n"),
        "import dev.gecode.iris.kotlin.build.IrisBuilder\n\
         import dev.gecode.iris.kotlin.build.irisBuilder\n\n\
         fun specBuilder(): IrisBuilder<Unit> = irisBuilder {\n    \
         skipLine()\n\
         }\n\n\
         fun main() {\n    \
         print(specBuilder().source())\n\
         }\n"
    );
    assert_statements(&[
        (
            "",
            &["fun specBuilder(): IrisBuilder<Unit> = irisBuilder {}"],
        ),
        (
            "format dimacs\n",
            &["fun specBuilder(): IrisBuilder<Unit> = irisBuilder(\"dimacs\") {}"],
        ),
        (
            "format \"two words\"\nskip line\n",
            &["irisBuilder(\"two words\") {"],
        ),
        (
            "skip line\nformat late\n",
            &["skipLine()", "format(\"late\")"],
        ),
    ]);
    // A format too wide for its line goes into a constant.
    let long = "f".repeat(90);
    assert_statements(&[
        (
            &format!("format {long}\nskip line\n"),
            &[
                "private const val T1 =",
                &format!("    \"{long}\""),
                "",
                "fun specBuilder(): IrisBuilder<Unit> =",
                "    irisBuilder(T1) {",
            ],
        ),
        (
            &format!("format {long}\n"),
            &["fun specBuilder(): IrisBuilder<Unit> = irisBuilder(T1) {}"],
        ),
    ]);
}

#[test]
fn handles_come_before_the_statements() {
    assert_statements(&[
        (
            "let y = x\nx: int\n",
            &[
                "import dev.gecode.iris.kotlin.build.IrisBuilder",
                "import dev.gecode.iris.kotlin.build.dynamic",
                "import dev.gecode.iris.kotlin.build.int",
                "import dev.gecode.iris.kotlin.build.irisBuilder",
                "",
                "private val x = dynamic(\"x\")",
                "private val y = dynamic(\"y\")",
                "",
                "fun specBuilder(): IrisBuilder<Unit> = irisBuilder {",
                "    let(y, x)",
                "    read(x, int())",
                "}",
            ],
        ),
        (
            "repeat 2 as jobs {\n  each job\n  m: int\n}\nlet t = size(jobs)\n",
            &[
                "private val m = dynamic(\"m\")",
                "private val t = dynamic(\"t\")",
                "private val job = recordName(\"job\", \"jobs\")",
            ],
        ),
        (
            "repeat 2 as jobs {\n  each job\n  m: int\n}\nlet t = size(jobs)\n",
            &[
                "repeat(2, job) {",
                "    read(m, int())",
                "}",
                "let(t, size(job.list()))",
            ],
        ),
        (
            "let y = defined(zz)\n",
            &[
                "fun specBuilder(): IrisBuilder<Unit> = irisBuilder {",
                "    val zz = undefined(\"zz\")",
                "    let(y, defined(zz))",
            ],
        ),
        (
            "repeat until \"end\" store _ {\n  x: int\n}\n",
            &[
                "val name = forwardDynamic(\"_\")",
                "repeatUntil(until(\"end\").store(name)) {",
                "    read(x, int())",
                "}",
            ],
        ),
    ]);
}

#[test]
fn settings() {
    assert_statements(&[
        ("comment \"c\" \"#\"\n", &["comment(\"c\", \"#\")"]),
        ("stop \"EOF\"\n", &["stop(\"EOF\")"]),
        ("tokens \"ab\"\n", &["tokens(\"ab\")"]),
        ("separators \",\"\n", &["separators(\",\")"]),
        ("ignore trailing\n", &["ignoreTrailing()"]),
    ]);
}

#[test]
fn declarations_and_types() {
    assert_statements(&[
        (
            "a: int\nb: float\nc: num\nd: word\ne: rest\n",
            &[
                "read(a, int())",
                "read(b, float())",
                "read(c, num())",
                "read(d, word())",
                "read(e, rest())",
            ],
        ),
        ("x: 1..\n", &["read(x, atLeast(1))"]),
        ("x: ..9\n", &["read(x, atMost(9))"]),
        ("x: ..<9\n", &["read(x, below(9))"]),
        // Two literals would make a Kotlin range, so the first is an
        // expression.
        ("x: 1..9\n", &["read(x, lit(1)..9)"]),
        // ktfmt removes the import of `rangeUntil`, so `..<` is a method.
        ("n: int\nx: 0..<n\n", &["read(x, range(0).below(n))"]),
        ("n: int\nx: n..9\n", &["read(x, n..9)"]),
        ("n: int\nx: -1..n - 1\n", &["read(x, -lit(1)..n - 1)"]),
        ("n: int\nx: 0..1.5\n", &["read(x, 0..lit(1.5).dynamic())"]),
        (
            "n: int\nxs[n][2]: int\n",
            &["read(xs, int().dim(n).dim(2))"],
        ),
        (
            "n: int\nxs[n][n]: int layout n offset 1\n",
            &["read(xs, int().dim(n).dim(n).layout(n).offset(1))"],
        ),
        (
            "n: int\nxs[n]: 0..<n\n",
            &["read(xs, range(0).below(n).dim(n))"],
        ),
        (
            "list ys: float until eol\n",
            &["list(ys, float().untilEol())"],
        ),
        ("list ys: int until 0\n", &["list(ys, int().until(0))"]),
        (
            "list ys: word until \";\"\n",
            &["list(ys, word().until(\";\"))"],
        ),
        (
            "list ys: int until 0 offset 1\n",
            &["list(ys, int().until(0).offset(1))"],
        ),
        (
            "q[1][2]: alphabet \"ab\" blank \".\" base 1\n",
            &["read(q, alphabet(\"ab\").blank(\".\").base(1).dim(1).dim(2))"],
        ),
        (
            "type small = 1..9\nxs[2]: small\n",
            &[
                "val small = defineType(\"small\", lit(1)..9)",
                "read(xs, small.dim(2))",
            ],
        ),
        ("type unused = int\n", &["defineType(\"unused\", int())"]),
        ("_: int\n", &["read(discard(), int())"]),
    ]);
}

#[test]
fn values_and_checks() {
    assert_statements(&[
        ("param p = 3\n", &["param(p, lit(3))"]),
        ("param p\n", &["param(p)"]),
        ("let p = \"s\"\n", &["let(p, lit(\"s\"))"]),
        ("x: int\ncheck x > 0\n", &["check(x gt 0)"]),
        (
            "x: int\ncheck x > 0 \"positive\"\n",
            &["check(x gt 0, \"positive\")"],
        ),
        ("check 1\n", &["check(lit(1))"]),
        ("expect \"begin\"\n", &["expect(\"begin\")"]),
        ("expect \"a\" \"b\"\n", &["expect(\"a\", \"b\")"]),
        ("skip 2 lines\n", &["skipLines(2)"]),
        ("x: int\nskip x lines\n", &["skipLines(x)"]),
        ("index 1.5\n", &["index(lit(1.5))"]),
        ("skip line\n", &["skipLine()"]),
    ]);
}

#[test]
fn lines_and_patterns() {
    assert_statements(&[
        ("line \"begin\"\n", &["line(\"begin\")"]),
        ("find \"x\"\n", &["find(\"x\")"]),
        (
            "line \"size {n:int} of {name:word}\"\n",
            &["line(\"size \", capture(n, int()), \" of \", capture(name, word()))"],
        ),
        ("line \"{x}\"\n", &["line(x)"]),
        ("n: int\nline \"{$n}\"\n", &["line(ref(n))"]),
        (
            "line \"{x:int}{_}{_:int}{}{:int}...\"\n",
            &[
                "line(capture(x, int()), discard(), discard(int()), blank(), validate(int()), restOfLine())",
            ],
        ),
    ]);
}

#[test]
fn blocks_are_trailing_lambdas() {
    assert_statements(&[
        (
            "repeat 2 {\n  x: int\n}\n",
            &["repeat(2) {", "    read(x, int())", "}"],
        ),
        (
            "repeat until eof {\n  x: int\n}\n",
            &["repeatUntilEof {", "    read(x, int())", "}"],
        ),
        (
            "repeat until \"end\" \"stop\" store last {\n  w: word\n}\n",
            &["repeatUntil(until(\"end\", \"stop\").store(last)) {"],
        ),
        (
            "repeat 2 as rows {\n  each row\n  x: int\n}\n",
            &["repeat(2, recordName(\"row\", \"rows\")) {"],
        ),
        ("repeat 2 {\n}\n", &["repeat(2) {}"]),
        ("repeat until eof {\n}\n", &["repeatUntilEof {}"]),
        (
            "instances {\n  x: int\n}\n",
            &["instances {", "    read(x, int())", "}"],
        ),
    ]);
}

#[test]
fn sections_take_their_clauses_and_arms_their_options() {
    assert_statements(&[
        (
            "sections {\n  \"a\" {\n    y: int\n  }\n}\n",
            &[
                "sections {",
                "    arm(\"a\") {",
                "        read(y, int())",
                "    }",
                "}",
            ],
        ),
        (
            "x: int\nsections until \"x\" store x skip other {\n  \"b\"\n}\n",
            &[
                "sections(until(\"x\").store(x), skipOther()) {",
                "    arm(\"b\")",
                "}",
            ],
        ),
        (
            "sections skip other {\n  \"b\" optional\n  \"c\" last\n  \"{x:int}\" optional last\n}\n",
            &[
                "arm(\"b\", optional = true)",
                "arm(\"c\", last = true)",
                "arm(capture(x, int()), optional = true, last = true)",
            ],
        ),
        (
            "sections {\n  \"a\" optional last {\n    y: int\n  }\n  \"\" {\n    z: int\n  }\n}\n",
            &[
                "optionalArm(\"a\", last = true) {",
                "    read(y, int())",
                "}",
                "arm(\"\") {",
            ],
        ),
        (
            "sections until \"X\" skip other until \"Y\" until \"Z\" {\n  \"A\" optional\n}\n",
            &["sections(until(\"X\"), skipOther(), until(\"Y\"), until(\"Z\")) {"],
        ),
        ("sections {\n}\n", &["sections {}"]),
    ]);
}

#[test]
fn if_chains_are_statements_one_after_another() {
    assert_statements(&[
        (
            "x: int\nif x > 1 {\n  a: int\n} else if x < 0 {\n  b: word\n} else {\n  c: num\n}\n",
            &[
                "ifThen(x gt 1) {",
                "    read(a, int())",
                "}",
                "elseIf(x lt 0) {",
                "    read(b, word())",
                "}",
                "otherwise {",
                "    read(c, num())",
                "}",
            ],
        ),
        (
            "x: int\nif x > 1 {\n  a: int\n} else if: word\n",
            &[
                "ifThen(x gt 1) {",
                "    read(a, int())",
                "}",
                "elseReadIf(if2, word())",
            ],
        ),
        (
            "x: int\nif x > 0 {\n} else {\n  y: int\n}\n",
            &["ifThen(x gt 0) {}", "otherwise {"],
        ),
    ]);
}

#[test]
fn deep_blocks_become_extension_functions_with_their_locals() {
    let source = format!(
        "type t = 1..3\nx: int\n{}y: t\nlet z = defined(zz)\n{}",
        repeated("repeat 1 {\n", 9),
        repeated("}\n", 9)
    );
    assert_statements(&[
        (
            source.as_str(),
            &[
                "private fun DeepScope.block1(t: IntType, zz: DynField) {",
                "    read(y, t)",
                "    let(z, defined(zz))",
                "}",
            ],
        ),
        (
            source.as_str(),
            &[
                "val zz = undefined(\"zz\")",
                "val t = defineType(\"t\", lit(1)..3)",
                "read(x, int())",
            ],
        ),
        (source.as_str(), &["repeat(1) {", "    block1(t, zz)", "}"]),
    ]);
    // An optional block has the optional class at the same depth, and an arm
    // the class of the block around its list.
    let optional = format!(
        "x: int\n{}if x > 0 {{\n{}y: int\n{}}}\n{}",
        repeated("repeat 1 {\n", 3),
        repeated("repeat 1 {\n", 5),
        repeated("}\n", 5),
        repeated("}\n", 3)
    );
    assert_statements(&[(
        optional.as_str(),
        &["private fun OptionalDeepScope.block1() {"],
    )]);
    let arms = format!(
        "{}sections {{\n\"a\" {{\n{}y: int\n{}}}\n}}\n{}",
        repeated("repeat 1 {\n", 2),
        repeated("if 1 {\n", 5),
        repeated("}\n", 5),
        repeated("}\n", 2)
    );
    assert_statements(&[(arms.as_str(), &["private fun OptionalScope2.block1() {"])]);
}

#[test]
fn operators_are_kotlins_with_the_parentheses_they_need() {
    assert_statements(&[
        (
            "x: int\nlet y = -x + 2 * x - (x - 1)\n",
            &["let(y, -x + lit(2) * x - (x - 1))"],
        ),
        (
            "x: int\nlet y = (x + 1) * x % 3\n",
            &["let(y, (x + 1) * x % 3)"],
        ),
        ("x: int\nlet y = x / (x * 2)\n", &["let(y, x / (x * 2))"]),
        ("x: int\nlet y = 1.5 < x\n", &["let(y, lit(1.5) lt x)"]),
        ("x: int\nlet y = x < 1.5\n", &["let(y, x lt 1.5)"]),
        ("x: int\nlet y = x < -1\n", &["let(y, x lt -lit(1))"]),
        ("x: int\nlet y = 2 ^ x\n", &["let(y, lit(2).pow(x))"]),
        ("x: int\nlet y = (x + 1) ^ 2\n", &["let(y, (x + 1).pow(2))"]),
        (
            "x: int\nlet y = gecode.int_max - x\n",
            &["let(y, Gecode.intMax - x)"],
        ),
        (
            "x: int\nlet y = !(x > 1) || x == 2 && x != 3\n",
            &["let(y, !(x gt 1) or ((x eq 2) and (x ne 3)))"],
        ),
        ("x: int\nlet y = -(-x)\n", &["let(y, -(-x))"]),
        ("x: int\nlet y = !!x\n", &["let(y, !(!x))"]),
        ("let y = -1\n", &["let(y, -lit(1))"]),
        ("let y = \"a\" == \"b\"\n", &["let(y, lit(\"a\") eq \"b\")"]),
        (
            "let y = 1 == \"b\"\n",
            &["let(y, lit(1).dynamic() eq \"b\")"],
        ),
        (
            "let x = 1.5\nlet y = x - 1 == \"a\"\n",
            &["let(y, (x - 1).dynamic() eq \"a\")"],
        ),
        ("let y = 1.5 + 1\n", &["let(y, lit(1.5).dynamic() + 1)"]),
        (
            "let y = 1 + 1.5\n",
            &["let(y, lit(1) + lit(1.5).dynamic())"],
        ),
        (
            "let y = \"a\" < 1\n",
            &["let(y, lit(\"a\").dynamic() lt 1)"],
        ),
        ("let y = 3000000000 + 1\n", &["let(y, lit(3000000000) + 1)"]),
        (
            "let y = -9223372036854775807 - 1 + 0\n",
            &["let(y, -lit(9223372036854775807) - 1 + 0)"],
        ),
    ]);
}

#[test]
fn host_constants_are_int_expressions() {
    assert_statements(&[
        (
            "let y = size(gecode.int_max)\n",
            &["let(y, size(Gecode.intMax))"],
        ),
        (
            "let y = gecode.int_max[0]\n",
            &["let(y, Gecode.intMax.at(0))"],
        ),
        (
            "let y = gecode.int_max == \"s\"\n",
            &["let(y, Gecode.intMax.dynamic() eq \"s\")"],
        ),
        (
            "let y = sum(v in gecode.int_max: v)\n",
            &["let(y, sum(over(Gecode.intMax).named(\"v\")) { v -> v })"],
        ),
    ]);
    assert!(code("let y = gecode.int_min\n").starts_with("import dev.gecode.iris.build.Gecode\n"));
}

#[test]
fn functions_records_and_comprehensions() {
    assert_statements(&[
        (
            "xs[2]: int\nlet a = min(xs[0], xs[1])\n",
            &["let(a, min(xs.at(0), xs.at(1)))"],
        ),
        (
            "xs[2]: int\nlet a = sum(1, xs[0])\n",
            &["let(a, sum(lit(1), xs.at(0)))"],
        ),
        ("xs[2]: int\nlet a = size(xs)\n", &["let(a, size(xs))"]),
        (
            "xs[2]: int\nlet a = defined(xs)\n",
            &["let(a, defined(xs))"],
        ),
        (
            "xs[2]: int\nlet c = all(v in xs: v > 0)\n",
            &["let(c, all(over(xs).named(\"v\")) { v -> v gt 0 })"],
        ),
        (
            "xs[2]: int\nlet c = count(v in xs: 1)\n",
            &["let(c, count(over(xs).named(\"v\")) { _ -> lit(1) })"],
        ),
        (
            "repeat 2 as jobs {\n  each job\n  repeat 2 as steps {\n    each step\n    \
             m: int\n  }\n}\nlet y = jobs[0].steps[1].m\n",
            &["let(y, job.list().at(0).records(step).at(1)[m])"],
        ),
        (
            "repeat 2 as jobs {\n  each job\n  repeat 2 as steps {\n    each step\n    \
             m: int\n  }\n}\nlet x = sum(j in jobs: sum(s in j.steps: s.m))\n",
            &[
                "val e1 = over(job.list()).named(\"j\")",
                "let(x, sum(e1) { j -> sum(over(j.records(step)).named(\"s\")) { s -> s[m] } })",
            ],
        ),
    ]);
}

#[test]
fn wide_comprehension_bodies_become_functions() {
    let long = "n".repeat(20);
    let source =
        format!("xs[2]: int\n{long}: int\nlet y = sum(v in xs: v + {long} + {long} + {long})\n");
    assert_statements(&[
        (
            source.as_str(),
            &["let(y, sum(over(xs).named(\"v\")) { v -> body1(v) })"],
        ),
        (
            source.as_str(),
            &[
                "private fun body1(v: DynExpr): Expr {",
                &format!("    return v + {long} + {long} + {long}"),
                "}",
            ],
        ),
    ]);
    assert!(
        code(&source).starts_with(
            "import dev.gecode.iris.build.DynExpr\nimport dev.gecode.iris.build.Expr\n"
        )
    );
    // A body that uses an outer variable takes it, and a wide return value
    // goes into a local value.
    let captured = format!(
        "xs[2]: int\n{long}: int\nlet y = sum(v in xs: sum(w in xs: v + w + {long} + {long} + {long} + {long}))\n"
    );
    assert_statements(&[
        (
            captured.as_str(),
            &[
                "let(y, sum(over(xs).named(\"v\")) { v -> sum(over(xs).named(\"w\")) { w -> body1(w, v) } })",
            ],
        ),
        (
            captured.as_str(),
            &[
                "private fun body1(w: DynExpr, v: DynExpr): Expr {",
                &format!("    val e1 = v + w + {long} + {long} + {long}"),
                &format!("    return e1 + {long}"),
                "}",
            ],
        ),
    ]);
    // A domain too wide for the lambda goes into a local value first.
    let domain = format!("xs[2]: int\nlet y = sum({long}{long}{long} in xs: 1)\n");
    assert_statements(&[(
        domain.as_str(),
        &[
            &format!("val e1 = over(xs).named(\"{long}{long}{long}\")"),
            "let(y, sum(e1) { _ -> lit(1) })",
        ],
    )]);
}

#[test]
fn layouts_are_those_of_ktfmt() {
    let long = "n".repeat(24);
    let m70 = "m".repeat(70);
    let m120 = "m".repeat(120);
    let fill = "\"abcdefg\" ".repeat(12);
    let pattern = format!(
        "sections {{\n  \"{{{long}:int}} {{{long}:int}} {{{long}:int}}\" optional {{\n    x: int\n  }}\n}}\n"
    );
    assert_statements(&[
        // One argument on each line, with a trailing comma.
        (
            &format!("{long}: int\nx: int\ncheck x > {long} \"{m70}\"\n"),
            &[
                "check(",
                &format!("    x gt {long},"),
                &format!("    \"{m70}\","),
                ")",
            ],
        ),
        // A string too wide for any line goes into a constant, which keeps it
        // whole.
        (
            &format!("x: int\ncheck x > 0 \"{m120}\"\n"),
            &["private const val T1 =", &format!("    \"{m120}\"")],
        ),
        (
            &format!("x: int\ncheck x > 0 \"{m120}\"\n"),
            &["check(x gt 0, T1)"],
        ),
        // Short arguments too, never filled.
        (
            &format!("expect {fill}\n"),
            &[
                "expect(",
                "    \"abcdefg\",",
                "    \"abcdefg\",",
                "    \"abcdefg\",",
                "    \"abcdefg\",",
                "    \"abcdefg\",",
                "    \"abcdefg\",",
                "    \"abcdefg\",",
                "    \"abcdefg\",",
                "    \"abcdefg\",",
                "    \"abcdefg\",",
                "    \"abcdefg\",",
                "    \"abcdefg\",",
                ")",
            ],
        ),
        // A block's opener closes its arguments before the brace.
        (
            &pattern,
            &[
                "optionalArm(",
                &format!("    capture({long}, int()),"),
                "    \" \",",
                &format!("    capture({long}, int()),"),
                "    \" \",",
                &format!("    capture({long}, int()),"),
                ") {",
                "    read(x, int())",
                "}",
            ],
        ),
    ]);
    // A local value breaks after `=`, then puts one argument on each line.
    let [a, b, c] = ["a", "b", "c"].map(|letter| letter.repeat(20));
    assert_statements(&[(
        &format!(
            "x: int\n{a}: int\n{b}: int\n{c}: int\nlet z = min({a}, {b}, {c}, {a}, {b}, x) + 1\n"
        ),
        &[
            "val e1 =",
            "    min(",
            &format!("        {a},"),
            &format!("        {b},"),
            &format!("        {c},"),
            &format!("        {a},"),
            &format!("        {b},"),
            "        x,",
            "    )",
            "let(z, e1 + 1)",
        ],
    )]);
}

#[test]
fn identifiers_are_lower_camel_case_and_never_reserved() {
    assert_statements(&[
        (
            "if: int\nval: int\ne1: int\ncount: int\nn_jobs: int\nHTTPServer: int\nrecord: int\n\
             N_MAX: int\ndata: int\n",
            &[
                "read(if2, int())",
                "read(val2, int())",
                "read(e1v, int())",
                "read(count, int())",
                "read(nJobs, int())",
                "read(httpServer, int())",
                "read(record2, int())",
                "read(nMax, int())",
                "read(data, int())",
            ],
        ),
        (
            "nJobs: int\nn_jobs: int\n",
            &["read(nJobs, int())", "read(nJobs2, int())"],
        ),
        (
            "type session = 1..2\ny: session\n",
            &[
                "val session2 = defineType(\"session\", lit(1)..2)",
                "read(y, session2)",
            ],
        ),
    ]);
}

#[test]
fn the_temporary_prefix_takes_a_letter_before_a_number() {
    for prefix in NUMBERED_PREFIXES {
        for source in prefix_collisions(prefix) {
            code_in_time(&source);
        }
    }
    assert_statements(&[("_e: int\ne: int\n", &["read(e, int())", "read(ev, int())"])]);
}

#[test]
fn strings_are_ascii_with_their_templates_escaped() {
    let code = code(
        "x: word\ncheck x == \"a\\tb\\\"c\\\\\u{e9}$y${z}\"\nlet y = \"a very long string that \
         does not fit on one line of a hundred columns at all, not even close, no, not at all\"\n",
    );
    for expected in [
        "private const val T1 =\n    \
         \"a very long string that does not fit on one line of a hundred columns at all, not even \
         close, no, not at all\"\n",
        "    check(x eq \"a\\tb\\\"c\\\\\\u00e9\\$y\\${z}\")\n",
        "    let(y, lit(T1))\n",
    ] {
        assert!(code.contains(expected), "{expected:?} is not in:\n{code}");
    }
    assert!(code.is_ascii(), "{code}");
}

#[test]
fn kotlin_has_builders_and_a_view_printer() {
    assert!(Language::Kotlin.has_view());
    assert_eq!(Language::Kotlin.name(), "kotlin");
    assert_eq!("kotlin".parse::<Language>(), Ok(Language::Kotlin));
    let parsed = parse(b"x: int\n", "test.iris", LanguageVersion::V0_1).unwrap();
    let analysis = analyze(&parsed);
    assert!(view_code(&parsed, &analysis, Language::Kotlin).is_ok());
}

/// detekt refuses a wildcard import and ktfmt removes an unused one, so the
/// program imports each builder declaration it names, operators included.
#[test]
fn the_program_imports_exactly_what_it_names() {
    let code = code(
        "n: int\nx: 0..<n\nz: 1..n\nlet y = -n % 2\ncheck n <= 1\nl[2]: int\n\
         let s = sum(it in l: it)\n",
    );
    let imports: Vec<&str> = code.lines().take_while(|line| !line.is_empty()).collect();
    assert_eq!(
        imports,
        [
            "IrisBuilder",
            "dynamic",
            "int",
            "irisBuilder",
            "le",
            "over",
            "range",
            "rangeTo",
            "rem",
            "sum",
            "unaryMinus",
        ]
        .map(|name| format!("import dev.gecode.iris.kotlin.build.{name}"))
    );
    // A variable named `it` is the lambda's implicit parameter.
    assert!(
        code.contains("let(s, sum(over(l).named(\"it\")) { it })"),
        "{code}"
    );
}
