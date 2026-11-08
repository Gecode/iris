//! The Java text of each production, each identifier rule and each layout rule
//! that the backend takes from google-java-format.
use gecode_iris_gen::{Language, ViewError, builder_code, view_code};
use gecode_iris_sema::analyze;
use gecode_iris_syntax::{LanguageVersion, parse};

fn code(source: &str) -> String {
    let parsed = parse(source.as_bytes(), "test.iris", LanguageVersion::V0_1).unwrap();
    let analysis = analyze(&parsed);
    builder_code(&parsed, &analysis, Language::Java).expect("the spec is accepted")
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
        "import static dev.gecode.iris.build.Iris.*;\n\n\
         import dev.gecode.iris.build.Builder;\n\n\
         final class SpecBuilder {\n  \
         private SpecBuilder() {}\n\n  \
         static Builder specBuilder() {\n    \
         return builder(\"dimacs\", SpecBuilder::body);\n  \
         }\n\n  \
         private static void body() {\n    \
         skipLine();\n  \
         }\n\n  \
         public static void main(String[] args) {\n    \
         System.out.print(specBuilder().source());\n  \
         }\n\
         }\n"
    );
    assert_statements(&[
        (
            "",
            &[
                "static Builder specBuilder() {",
                "  return builder(SpecBuilder::body);",
                "}",
                "",
                "private static void body() {}",
            ],
        ),
        (
            "format \"two words\"\n",
            &["return builder(\"two words\", SpecBuilder::body);"],
        ),
        (
            "skip line\nformat late\n",
            &["skipLine();", "format(\"late\");"],
        ),
    ]);
}

#[test]
fn handles_come_before_the_statements() {
    assert_statements(&[
        (
            "let y = x\nx: int\n",
            &[
                "final class SpecBuilder {",
                "  private static final DynField x = dynamic(\"x\");",
                "  private static final DynField y = dynamic(\"y\");",
                "",
                "  private SpecBuilder() {}",
            ],
        ),
        (
            "let y = x\nx: int\n",
            &[
                "private static void body() {",
                "  let(y, x);",
                "  read(x, intType());",
            ],
        ),
        (
            "repeat 2 as jobs {\n  each job\n  m: int\n}\nlet t = size(jobs)\n",
            &[
                "private static final DynField m = dynamic(\"m\");",
                "private static final DynField t = dynamic(\"t\");",
                "private static final RecordName job = recordName(\"job\", \"jobs\");",
            ],
        ),
        (
            "repeat 2 as jobs {\n  each job\n  m: int\n}\nlet t = size(jobs)\n",
            &[
                "repeat(",
                "    2,",
                "    job,",
                "    () -> {",
                "      read(m, intType());",
                "    });",
                "let(t, size(job.list()));",
            ],
        ),
        (
            "let y = defined(zz)\n",
            &[
                "private static void body() {",
                "  var zz = undefined(\"zz\");",
                "  let(y, defined(zz));",
            ],
        ),
        (
            "repeat until \"end\" store _ {\n  x: int\n}\n",
            &[
                "var name = forward(\"_\", Dyn.class);",
                "repeatUntil(",
                "    until(\"end\").store(name),",
            ],
        ),
    ]);
}

#[test]
fn settings() {
    assert_statements(&[
        ("comment \"c\" \"#\"\n", &["comment(\"c\", \"#\");"]),
        ("stop \"EOF\"\n", &["stop(\"EOF\");"]),
        ("tokens \"ab\"\n", &["tokens(\"ab\");"]),
        ("separators \",\"\n", &["separators(\",\");"]),
        ("ignore trailing\n", &["ignoreTrailing();"]),
    ]);
}

#[test]
fn declarations_and_types() {
    assert_statements(&[
        (
            "a: int\nb: float\nc: num\nd: word\ne: rest\n",
            &[
                "read(a, intType());",
                "read(b, floatType());",
                "read(c, numType());",
                "read(d, word());",
                "read(e, rest());",
            ],
        ),
        ("x: 1..\n", &["read(x, atLeast(1));"]),
        ("x: ..9\n", &["read(x, atMost(9));"]),
        ("x: ..<9\n", &["read(x, below(9));"]),
        ("x: 1..9\n", &["read(x, range(1).to(9));"]),
        ("n: int\nx: 0..<n\n", &["read(x, range(0).below(n));"]),
        (
            "x: 0..1.5\n",
            &["read(x, range(0).to(lit(1.5).dynamic()));"],
        ),
        (
            "n: int\nxs[n][2]: int\n",
            &["read(xs, intType().dim(n).dim(2));"],
        ),
        (
            "n: int\nxs[n][n]: int layout n offset 1\n",
            &["read(xs, intType().dim(n).dim(n).layout(n).offset(1));"],
        ),
        (
            "list ys: float until eol\n",
            &["list(ys, floatType().untilEol());"],
        ),
        ("list ys: int until 0\n", &["list(ys, intType().until(0));"]),
        (
            "list ys: word until \";\"\n",
            &["list(ys, word().until(\";\"));"],
        ),
        (
            "list ys: int until 0 offset 1\n",
            &["list(ys, intType().until(0).offset(1));"],
        ),
        (
            "q[1][2]: alphabet \"ab\" blank \".\" base 1\n",
            &["read(q, alphabet(\"ab\").blank(\".\").base(1).dim(1).dim(2));"],
        ),
        (
            "type small = 1..9\nxs[2]: small\n",
            &[
                "var small = defineType(\"small\", range(1).to(9));",
                "read(xs, small.dim(2));",
            ],
        ),
        (
            "type unused = int\n",
            &["defineType(\"unused\", intType());"],
        ),
        ("_: int\n", &["read(discard(), intType());"]),
    ]);
}

#[test]
fn values_and_checks() {
    assert_statements(&[
        ("param p = 3\n", &["param(p, lit(3));"]),
        ("param p\n", &["param(p);"]),
        ("let p = \"s\"\n", &["let(p, lit(\"s\"));"]),
        ("x: int\ncheck x > 0\n", &["check(x.gt(0));"]),
        (
            "x: int\ncheck x > 0 \"positive\"\n",
            &["check(x.gt(0), \"positive\");"],
        ),
        ("check 1\n", &["check(lit(1));"]),
        ("expect \"begin\"\n", &["expect(\"begin\");"]),
        ("expect \"a\" \"b\"\n", &["expect(\"a\", \"b\");"]),
        ("skip 2 lines\n", &["skipLines(2);"]),
        ("x: int\nskip x lines\n", &["skipLines(x);"]),
        ("index 1.5\n", &["index(lit(1.5));"]),
        ("skip line\n", &["skipLine();"]),
    ]);
}

#[test]
fn lines_and_patterns() {
    assert_statements(&[
        ("line \"begin\"\n", &["line(\"begin\");"]),
        ("find \"x\"\n", &["find(\"x\");"]),
        (
            "line \"size {n:int} of {name:word}\"\n",
            &["line(\"size \", capture(n, intType()), \" of \", capture(name, word()));"],
        ),
        ("line \"{x}\"\n", &["line(x);"]),
        ("n: int\nline \"{$n}\"\n", &["line(ref(n));"]),
        (
            "line \"{x:int}{_}{_:int}{}{:int}...\"\n",
            &[
                "line(",
                "    capture(x, intType()),",
                "    discard(),",
                "    discard(intType()),",
                "    blank(),",
                "    validate(intType()),",
                "    restOfLine());",
            ],
        ),
    ]);
}

#[test]
fn repeats() {
    assert_statements(&[
        (
            "repeat 2 {\n  x: int\n}\n",
            &[
                "repeat(",
                "    2,",
                "    () -> {",
                "      read(x, intType());",
                "    });",
            ],
        ),
        (
            "repeat until eof {\n  x: int\n}\n",
            &["repeatUntilEof(() -> {", "  read(x, intType());", "});"],
        ),
        (
            "repeat until \"end\" \"stop\" store last {\n  w: word\n}\n",
            &[
                "repeatUntil(",
                "    until(\"end\", \"stop\").store(last),",
                "    () -> {",
            ],
        ),
        (
            "repeat 2 as rows {\n  each row\n  x: int\n}\n",
            &[
                "repeat(",
                "    2,",
                "    recordName(\"row\", \"rows\"),",
                "    () -> {",
            ],
        ),
        ("repeat 2 {\n}\n", &["repeat(2, () -> {});"]),
    ]);
}

#[test]
fn sections_take_their_clauses_one_by_one_up_to_three() {
    assert_statements(&[
        (
            "sections {\n  \"a\" {\n    y: int\n  }\n}\n",
            &[
                "sections(() -> {",
                "  arm(",
                "      \"a\",",
                "      () -> {",
                "        read(y, intType());",
                "      });",
                "});",
            ],
        ),
        (
            "x: int\nsections until \"x\" store x skip other {\n  \"b\"\n}\n",
            &[
                "sections(",
                "    until(\"x\").store(x),",
                "    skipOther(),",
                "    () -> {",
                "      arm(\"b\");",
                "    });",
            ],
        ),
        (
            "sections skip other {\n  \"b\" optional\n  \"c\" last\n  \"{x:int}\" optional last\n}\n",
            &[
                "arm(\"b\", optional());",
                "arm(\"c\", last());",
                "arm(pattern(capture(x, intType())), optional(), last());",
            ],
        ),
        (
            "sections until \"X\" skip other until \"Y\" until \"Z\" {\n  \"A\" optional\n}\n",
            &[
                "sections(",
                "    List.of(until(\"X\"), skipOther(), until(\"Y\"), until(\"Z\")),",
                "    () -> {",
            ],
        ),
        ("sections {\n}\n", &["sections(() -> {});"]),
    ]);
    assert!(
        code("sections until \"X\" skip other until \"Y\" until \"Z\" {\n  \"A\"\n}\n")
            .contains("\nimport java.util.List;\n")
    );
}

#[test]
fn instances() {
    assert_statements(&[(
        "instances {\n  x: int\n}\n",
        &["instances(() -> {", "  read(x, intType());", "});"],
    )]);
}

#[test]
fn if_chains_are_statements_one_after_another() {
    assert_statements(&[
        (
            "x: int\nif x > 1 {\n  a: int\n} else if x < 0 {\n  b: word\n} else {\n  c: num\n}\n",
            &[
                "ifThen(",
                "    x.gt(1),",
                "    () -> {",
                "      read(a, intType());",
                "    });",
                "elseIf(",
                "    x.lt(0),",
                "    () -> {",
                "      read(b, word());",
                "    });",
                "otherwise(() -> {",
                "  read(c, numType());",
                "});",
            ],
        ),
        (
            "x: int\nif x > 1 {\n  a: int\n} else if: word\n",
            &[
                "ifThen(",
                "    x.gt(1),",
                "    () -> {",
                "      read(a, intType());",
                "    });",
                "elseReadIf(if2, word());",
            ],
        ),
        (
            "x: int\nif x > 0 {\n} else {\n  y: int\n}\n",
            &["ifThen(x.gt(0), () -> {});", "otherwise(() -> {"],
        ),
    ]);
}

#[test]
fn deep_blocks_become_methods_with_their_locals() {
    let source = format!(
        "type t = 1..3\nx: int\n{}y: t\nlet z = defined(zz)\n{}",
        repeated("repeat 1 {\n", 7),
        repeated("}\n", 7)
    );
    assert_statements(&[(
        source.as_str(),
        &[
            "private static void block1(IntType t, DynField zz) {",
            "  repeat(",
            "      1,",
            "      () -> {",
            "        read(y, t);",
            "        let(z, defined(zz));",
            "      });",
            "}",
        ],
    )]);
    assert_statements(&[(
        source.as_str(),
        &[
            "var zz = undefined(\"zz\");",
            "var t = defineType(\"t\", range(1).to(3));",
            "read(x, intType());",
        ],
    )]);
    assert_statements(&[(source.as_str(), &["repeat(1, () -> block1(t, zz));"])]);
    let plain = format!(
        "{}y: int\n{}",
        repeated("repeat 1 {\n", 7),
        repeated("}\n", 7)
    );
    assert_statements(&[(plain.as_str(), &["repeat(1, SpecBuilder::block1);"])]);
}

#[test]
fn operators_are_methods_of_the_left_operand() {
    assert_statements(&[
        (
            "x: int\nlet y = -x + 2 * x - (x - 1)\n",
            &["let(y, x.neg().plus(lit(2).times(x)).minus(x.minus(1)));"],
        ),
        ("x: int\nlet y = 1.5 < x\n", &["let(y, lit(1.5).lt(x));"]),
        ("x: int\nlet y = x < 1.5\n", &["let(y, x.lt(1.5));"]),
        ("x: int\nlet y = 2 ^ x\n", &["let(y, lit(2).pow(x));"]),
        (
            "x: int\nlet y = gecode.int_max - x\n",
            &["let(y, Gecode.intMax.minus(x));"],
        ),
        (
            "x: int\nlet y = !(x > 1) || x == 2 && x != 3\n",
            &["let(y, x.gt(1).not().or(x.eq(2).and(x.ne(3))));"],
        ),
        ("x: int\nlet y = -(-x)\n", &["let(y, x.neg().neg());"]),
        (
            "let y = \"a\" == \"b\"\n",
            &["let(y, lit(\"a\").eq(\"b\"));"],
        ),
        (
            "let y = 1 == \"b\"\n",
            &["let(y, lit(1).dynamic().eq(\"b\"));"],
        ),
        (
            "let x = 1.5\nlet y = x - 1 == \"a\"\n",
            &["let(y, x.minus(1).dynamic().eq(\"a\"));"],
        ),
        (
            "let y = 1.5 + 1\n",
            &["let(y, lit(1.5).dynamic().plus(1));"],
        ),
        (
            "let y = 1 + 1.5\n",
            &["let(y, lit(1).plus(lit(1.5).dynamic()));"],
        ),
        (
            "let y = \"a\" < 1\n",
            &["let(y, lit(\"a\").dynamic().lt(1));"],
        ),
        (
            "let y = 3000000000 + 1\n",
            &["let(y, lit(3000000000L).plus(1));"],
        ),
    ]);
}

#[test]
fn host_constants_are_int_expressions() {
    assert_statements(&[
        (
            "let y = size(gecode.int_max)\n",
            &["let(y, size(Gecode.intMax));"],
        ),
        (
            "let y = gecode.int_max[0]\n",
            &["let(y, Gecode.intMax.at(0));"],
        ),
        (
            "let y = gecode.int_max == \"s\"\n",
            &["let(y, Gecode.intMax.dynamic().eq(\"s\"));"],
        ),
        (
            "let y = sum(v in gecode.int_max: v)\n",
            &["let(y, sum(over(Gecode.intMax).named(\"v\"), v -> v));"],
        ),
    ]);
    assert!(code("let y = gecode.int_min\n").contains("\nimport dev.gecode.iris.build.Gecode;\n"));
}

#[test]
fn functions_records_and_comprehensions() {
    assert_statements(&[
        (
            "xs[2]: int\nlet a = min(xs[0], xs[1])\n",
            &["let(a, min(xs.at(0), xs.at(1)));"],
        ),
        (
            "xs[2]: int\nlet a = sum(1, xs[0])\n",
            &["let(a, sum(lit(1), xs.at(0)));"],
        ),
        ("xs[2]: int\nlet a = size(xs)\n", &["let(a, size(xs));"]),
        (
            "xs[2]: int\nlet a = defined(xs)\n",
            &["let(a, defined(xs));"],
        ),
        (
            "xs[2]: int\nlet c = all(v in xs: v > 0)\n",
            &["let(c, all(over(xs).named(\"v\"), v -> v.gt(0)));"],
        ),
        (
            "xs[2]: int\nlet c = count(v in xs: 1)\n",
            &["let(c, count(over(xs).named(\"v\"), _ -> lit(1)));"],
        ),
        (
            "repeat 2 as jobs {\n  each job\n  repeat 2 as steps {\n    each step\n    \
             m: int\n  }\n}\nlet y = jobs[0].steps[1].m\n",
            &["let(y, job.list().at(0).records(step).at(1).get(m));"],
        ),
        (
            "repeat 2 as jobs {\n  each job\n  repeat 2 as steps {\n    each step\n    \
             m: int\n  }\n}\nlet x = sum(j in jobs: sum(s in j.steps: s.m))\n",
            &[
                "var e1 =",
                "    sum(over(job.list()).named(\"j\"), j -> sum(over(j.records(step)).named(\"s\"), \
                 s -> s.get(m)));",
                "let(x, e1);",
            ],
        ),
    ]);
}

#[test]
fn wide_comprehension_bodies_become_methods() {
    let long = "n".repeat(20);
    let source =
        format!("xs[2]: int\n{long}: int\nlet y = sum(v in xs: v + {long} + {long} + {long})\n");
    assert_statements(&[
        (
            source.as_str(),
            &["let(y, sum(over(xs).named(\"v\"), SpecBuilder::body1));"],
        ),
        (
            source.as_str(),
            &[
                "private static Expr body1(DynExpr v) {",
                &format!("  return v.plus({long}).plus({long}).plus({long});"),
                "}",
            ],
        ),
    ]);
    // A body that uses an outer variable takes it, and a wide return value
    // goes into a temporary.
    let captured = format!(
        "xs[2]: int\n{long}: int\nlet y = sum(v in xs: sum(w in xs: v + w + {long} + {long} + {long}))\n"
    );
    assert_statements(&[
        (
            captured.as_str(),
            &[
                "let(y, sum(over(xs).named(\"v\"), v -> sum(over(xs).named(\"w\"), w -> body1(w, v))));",
            ],
        ),
        (
            captured.as_str(),
            &[
                "private static Expr body1(DynExpr w, DynExpr v) {",
                &format!("  var e1 = v.plus(w).plus({long}).plus({long});"),
                &format!("  return e1.plus({long});"),
                "}",
            ],
        ),
    ]);
}

#[test]
fn layouts_are_those_of_google_java_format() {
    let long = "n".repeat(24);
    let m55 = "m".repeat(55);
    let m70 = "m".repeat(70);
    let fill = "\"abcdefg\" ".repeat(12);
    let pattern = format!(
        "sections {{\n  \"{{{long}:int}} {{{long}:int}} {{{long}:int}}\" optional {{\n    x: int\n  }}\n}}\n"
    );
    assert_statements(&[
        // The arguments on one line after a break.
        (
            &format!("{long}: int\nx: int\ncheck x > {long} \"{m55}\"\n"),
            &["check(", &format!("    x.gt({long}), \"{m55}\");")],
        ),
        // One argument on each line.
        (
            &format!("{long}: int\nx: int\ncheck x > {long} \"{m70}\"\n"),
            &[
                "check(",
                &format!("    x.gt({long}),"),
                &format!("    \"{m70}\");"),
            ],
        ),
        // Lines filled with short arguments.
        (
            &format!("expect {fill}\n"),
            &[
                "expect(",
                "    \"abcdefg\", \"abcdefg\", \"abcdefg\", \"abcdefg\", \"abcdefg\", \"abcdefg\", \
                 \"abcdefg\", \"abcdefg\",",
                "    \"abcdefg\", \"abcdefg\", \"abcdefg\", \"abcdefg\");",
            ],
        ),
        // A break after `=`, then one argument on each line, and a block
        // lambda after other arguments.
        (
            &pattern,
            &[
                "var e1 =",
                "    pattern(",
                &format!("        capture({long}, intType()),"),
                "        \" \",",
                &format!("        capture({long}, intType()),"),
                "        \" \",",
                &format!("        capture({long}, intType()));"),
                "arm(",
                "    e1,",
                "    optional(),",
                "    () -> {",
                "      read(x, intType());",
                "    });",
            ],
        ),
    ]);
}

#[test]
fn identifiers_are_lower_camel_case_and_never_reserved() {
    assert_statements(&[
        (
            "if: int\nvar: int\ne1: int\ncount: int\nn_jobs: int\nHTTPServer: int\nargs: int\n\
             N_MAX: int\n",
            &[
                "read(if2, intType());",
                "read(var2, intType());",
                "read(e1v, intType());",
                "read(count, intType());",
                "read(nJobs, intType());",
                "read(httpServer, intType());",
                "read(args2, intType());",
                "read(nMax, intType());",
            ],
        ),
        (
            "nJobs: int\nn_jobs: int\n",
            &["read(nJobs, intType());", "read(nJobs2, intType());"],
        ),
        (
            "type record = 1..2\ny: record\n",
            &[
                "var record2 = defineType(\"record\", range(1).to(2));",
                "read(y, record2);",
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
    assert_statements(&[(
        "_e: int\ne: int\n",
        &["read(e, intType());", "read(ev, intType());"],
    )]);
}

#[test]
fn strings_are_ascii_and_long_ones_are_constants() {
    let code = code(
        "x: word\ncheck x == \"a\\tb\\\"c\\\\\u{e9}\"\nlet y = \"a very long string that \
         does not fit on one line of a hundred columns at all, not even close, no, not at all\"\n",
    );
    for expected in [
        "  private static final String T1 =\n      \
         \"a very long string that does not fit on one line of a hundred columns at all, not even \
         close\"\n          + \", no, not at all\";\n",
        "    check(x.eq(\"a\\tb\\\"c\\\\\\u00e9\"));\n",
        "    let(y, lit(T1));\n",
    ] {
        assert!(code.contains(expected), "{expected:?} is not in:\n{code}");
    }
    assert!(code.is_ascii(), "{code}");
}

#[test]
fn java_has_builders_but_no_view_printer() {
    assert!(!Language::Java.has_view());
    assert_eq!(Language::Java.name(), "java");
    assert_eq!("java".parse::<Language>(), Ok(Language::Java));
    let parsed = parse(b"x: int\n", "test.iris", LanguageVersion::V0_1).unwrap();
    let analysis = analyze(&parsed);
    assert!(matches!(
        view_code(&analysis, Language::Java),
        Err(ViewError::NoPrinter(Language::Java))
    ));
}
