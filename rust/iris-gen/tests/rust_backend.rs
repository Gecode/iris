//! The Rust text of each production and each identifier rule.
use iris_gen::{Language, builder_code};
use iris_sema::analyze;
use iris_syntax::{LanguageVersion, parse};

mod equal_operands;

fn code(source: &str) -> String {
    let parsed = parse(source.as_bytes(), "test.iris", LanguageVersion::V0_1).unwrap();
    let analysis = analyze(&parsed);
    builder_code(&parsed, &analysis, Language::Rust).expect("the spec is accepted")
}

/// Whether the builder of each spec holds its expected statements, in order.
#[track_caller]
fn assert_statements(cases: &[(&str, &[&str])]) {
    for (source, statements) in cases {
        let code = code(source);
        let lines: Vec<&str> = code.lines().map(str::trim).collect();
        let found = lines
            .windows(statements.len())
            .any(|window| window == *statements);
        assert!(found, "{source:?} lacks {statements:?} in:\n{code}");
    }
}

#[test]
fn the_format_chooses_the_constructor() {
    assert_statements(&[
        ("format dimacs\n", &["SpecBuilder::new(\"dimacs\")"]),
        (
            "format \"two words\"\nskip line\n",
            &["let mut b = SpecBuilder::new(\"two words\");"],
        ),
        ("skip line\n", &["let mut b = SpecBuilder::unnamed();"]),
    ]);
}

#[test]
fn settings() {
    assert_statements(&[
        ("comment \"c\" \"#\"\n", &["b.comment([\"c\", \"#\"]);"]),
        ("stop \"EOF\"\n", &["b.stop([\"EOF\"]);"]),
        ("tokens \"abc\"\n", &["b.tokens(\"abc\");"]),
        ("separators \",;\"\n", &["b.separators(\",;\");"]),
        ("ignore trailing\n", &["b.ignore_trailing();"]),
    ]);
}

#[test]
fn declarations_and_base_types() {
    assert_statements(&[
        ("x: int\n", &["b.read(x, int());"]),
        ("x: float\n", &["b.read(x, float());"]),
        ("x: num\n", &["b.read(x, num());"]),
        ("x: word\n", &["b.read(x, word());"]),
        ("x: rest\n", &["b.read(x, rest());"]),
        ("_: int\n", &["b.read(discard(), int());"]),
    ]);
}

#[test]
fn ranges() {
    assert_statements(&[
        ("x: 1..9\n", &["b.read(x, 1..=9);"]),
        ("x: 1..<9\n", &["b.read(x, 1..9);"]),
        ("x: ..9\n", &["b.read(x, ..=9);"]),
        ("x: 1..\n", &["b.read(x, 1..);"]),
    ]);
}

#[test]
fn arrays_lists_and_alphabets() {
    assert_statements(&[
        ("x[3]: int\n", &["b.read(x, int().dim(3));"]),
        ("x[2][3]: int\n", &["b.read(x, int().dim(2).dim(3));"]),
        (
            "x[3]: int offset 1\n",
            &["b.read(x, int().dim(3).offset(1));"],
        ),
        (
            "x[2][2]: int layout \"LOWER_COL\"\n",
            &["b.read(x, int().dim(2).dim(2).layout(\"LOWER_COL\"));"],
        ),
        ("list xs: int until 0\n", &["b.list(xs, int().until(0));"]),
        (
            "list xs: int until eol\n",
            &["b.list(xs, int().until(eol()));"],
        ),
        (
            "x[2][2]: alphabet \"abc\"\n",
            &["b.read(x, ty(alphabet(\"abc\")).dim(2).dim(2));"],
        ),
        (
            "x[2][2]: alphabet \"abc\" blank \".\"\n",
            &["b.read(x, ty(alphabet(\"abc\").blank(\".\")).dim(2).dim(2));"],
        ),
        (
            "x[2][2]: alphabet \"abc\" strip \" \"\n",
            &["b.read(x, ty(alphabet(\"abc\").strip(\" \")).dim(2).dim(2));"],
        ),
        (
            "x[2][2]: alphabet \"abc\" base 0\n",
            &["b.read(x, ty(alphabet(\"abc\").base(0)).dim(2).dim(2));"],
        ),
    ]);
}

#[test]
fn values_and_checks() {
    assert_statements(&[
        ("let x = 1\n", &["b.let_(x, 1);"]),
        ("param x\n", &["b.param(x);"]),
        ("param x = 1\n", &["b.param_default(x, 1);"]),
        (
            "type t = 1..9\nx: t\n",
            &["let t = b.define_type(\"t\", 1..=9);", "b.read(x, t);"],
        ),
        ("check 1 == 1\n", &["b.check(lit(1).eq(1));"]),
        (
            "check 1 == 1 \"message\"\n",
            &["b.check(lit(1).eq(1)).message(\"message\");"],
        ),
    ]);
}

#[test]
fn lines_and_patterns() {
    assert_statements(&[
        ("expect \"a\" \"b\"\n", &["b.expect([\"a\", \"b\"]);"]),
        ("find \"a\"\n", &["b.find(\"a\");"]),
        ("skip line\n", &["b.skip_line();"]),
        ("skip 2 lines\n", &["b.skip_lines(2);"]),
        ("index 2\n", &["b.index(2);"]),
        (
            "line \"a {x:int} b\"\n",
            &["b.line(Pattern::new().text(\"a \").part(bind(x, int())).text(\" b\"));"],
        ),
        (
            "line \"{x}\"\n",
            &["b.line(Pattern::new().part(bind(x, untyped())));"],
        ),
        (
            "n: int\nline \"{$n}\"\n",
            &["b.line(Pattern::new().part(reference(n)));"],
        ),
        (
            "line \"{x:int}{_}{_:int}{}{:int}...\"\n",
            &[
                "let e1 = Pattern::new()",
                ".part(bind(x, int()))",
                ".part(discard())",
                ".part(discard_as(int()))",
                ".part(blank())",
                ".part(validate(int()))",
                ".part(rest_of_line());",
                "b.line(e1);",
            ],
        ),
    ]);
}

#[test]
fn repeats() {
    assert_statements(&[
        (
            "repeat 2 {\n  x: int\n}\n",
            &["b.repeat(2, |s| {", "s.read(x, int());", "});"],
        ),
        (
            "repeat until eof {\n  x: int\n}\n",
            &[
                "b.repeat_until_eof().body(|s| {",
                "s.read(x, int());",
                "});",
            ],
        ),
        (
            "repeat until \"END\" {\n  x: int\n}\n",
            &[
                "b.repeat_until([\"END\"]).body(|s| {",
                "s.read(x, int());",
                "});",
            ],
        ),
        (
            "repeat 2 as r {\n  each c\n  x: int\n}\n",
            &[
                "b.repeat(2, |s| {",
                "s.each_name(\"c\");",
                "s.as_name(\"r\");",
                "s.read(x, int());",
                "});",
            ],
        ),
        (
            "repeat 2 as r {\n  x: int\n}\nlet y = r[0].x\n",
            &[
                "let record_1 = b.forward_record(\"record_1\", \"r\");",
                "b.repeat(2, |s| {",
                "s.record_as(record_1);",
                "s.read(x, int());",
                "});",
                "b.let_(y, record_1.list().at(0).get(x));",
            ],
        ),
    ]);
}

#[test]
fn sections() {
    assert_statements(&[
        (
            "sections {\n  \"a\" {\n    x: int\n  }\n}\n",
            &[
                "b.sections().body(|a| {",
                "a.arm(\"a\").body(|s| {",
                "s.read(x, int());",
                "});",
                "});",
            ],
        ),
        (
            "sections until \"END\" {\n  \"a\" optional {\n    x: int\n  }\n  \"b\" last\n}\n",
            &[
                "b.sections().until([\"END\"]).body(|a| {",
                "a.arm(\"a\").optional().body(|s| {",
                "s.read(x, int());",
                "});",
                "a.arm(\"b\").last();",
                "});",
            ],
        ),
        (
            "sections skip other {\n  \"a\"\n}\n",
            &[
                "b.sections().skip_other().body(|a| {",
                "a.arm(\"a\");",
                "});",
            ],
        ),
    ]);
}

#[test]
fn instances() {
    assert_statements(&[(
        "instances {\n  x: int\n}\n",
        &["b.instances(|s| {", "s.read(x, int());", "});"],
    )]);
}

#[test]
fn if_chains() {
    assert_statements(&[
        (
            "x: int\nif x > 0 {\n  y: int\n}\n",
            &[
                "b.if_(x.gt(0), |s| {",
                "s.read(y, int());",
                "})",
                ".finish();",
            ],
        ),
        (
            "x: int\nif x > 0 {\n  y: int\n} else {\n  z: int\n}\n",
            &[
                "b.if_(x.gt(0), |s| {",
                "s.read(y, int());",
                "})",
                ".else_(|s| {",
                "s.read(z, int());",
                "});",
            ],
        ),
        (
            "x: int\nif x > 0 {\n  y: int\n} else if x < 0 {\n  z: int\n}\n",
            &[
                "b.if_(x.gt(0), |s| {",
                "s.read(y, int());",
                "})",
                ".else_if(x.lt(0), |s| {",
                "s.read(z, int());",
                "})",
                ".finish();",
            ],
        ),
        (
            "x: int\nif x > 0 {\n  y: int\n} else if: int\n",
            &[
                "b.if_(x.gt(0), |s| {",
                "s.read(y, int());",
                "})",
                ".else_read_if(if_2, int());",
            ],
        ),
    ]);
}

#[test]
fn empty_branches_follow_rustfmt() {
    assert_statements(&[
        (
            "x: int\nif x > 0 {\n}\n",
            &["b.if_(x.gt(0), |_| {}).finish();"],
        ),
        (
            "x: int\nif x > 0 {\n} else {\n}\n",
            &["b.if_(x.gt(0), |_| {}).else_(|_| {});"],
        ),
        (
            "x: int\nif x > 0 {\n} else {\n  y: int\n}\n",
            &[
                "b.if_(x.gt(0), |_| {}).else_(|s| {",
                "s.read(y, int());",
                "});",
            ],
        ),
        (
            "x: int\nif x > 0 {\n} else if x > 1 {\n  y: int\n}\n",
            &[
                "b.if_(x.gt(0), |_| {})",
                ".else_if(x.gt(1), |s| {",
                "s.read(y, int());",
                "})",
                ".finish();",
            ],
        ),
    ]);
}

#[test]
fn operators() {
    assert_statements(&[
        ("let x = 1 + 2 * 3\n", &["b.let_(x, lit(1) + lit(2) * 3);"]),
        ("let x = (1 + 2) * 3\n", &["b.let_(x, (lit(1) + 2) * 3);"]),
        (
            "let x = 1 - (2 - 3)\n",
            &["b.let_(x, lit(1) - (lit(2) - 3));"],
        ),
        ("x: int\nlet y = -x\n", &["b.let_(y, -x);"]),
        ("x: int\nlet y = !x\n", &["b.let_(y, !x);"]),
        ("x: int\nlet y = x % 2\n", &["b.let_(y, x % 2);"]),
        ("x: int\nlet y = x ^ 2\n", &["b.let_(y, x.pow(2));"]),
        (
            "x: int\nlet y = x > 1 && x < 3\n",
            &["b.let_(y, x.gt(1) & x.lt(3));"],
        ),
        (
            "x: int\nlet y = x > 1 || x < 3\n",
            &["b.let_(y, x.gt(1) | x.lt(3));"],
        ),
        ("let y = gecode.int_max\n", &["b.let_(y, gecode::INT_MAX);"]),
        ("let y = gecode.int_min\n", &["b.let_(y, gecode::INT_MIN);"]),
        ("let y = 2.5\n", &["b.let_(y, 2.5);"]),
        ("let y = \"s\"\n", &["b.let_(y, \"s\");"]),
    ]);
}

#[test]
fn functions_and_comprehensions() {
    assert_statements(&[
        ("xs[3]: int\nlet y = sum(xs)\n", &["b.let_(y, sum(xs));"]),
        ("xs[3]: int\nlet y = size(xs)\n", &["b.let_(y, size(xs));"]),
        (
            "xs[3]: int\nlet y = count(xs)\n",
            &["b.let_(y, count(xs));"],
        ),
        ("xs[3]: int\nlet y = min(xs)\n", &["b.let_(y, min(xs));"]),
        ("xs[3]: int\nlet y = max(xs)\n", &["b.let_(y, max(xs));"]),
        (
            "xs[3]: int\nlet y = distinct(xs)\n",
            &["b.let_(y, distinct(xs));"],
        ),
        ("xs[3]: int\nlet y = xs[0]\n", &["b.let_(y, xs.at(0));"]),
        (
            "let y = defined(z)\n",
            &[
                "let z = b.undefined(\"z\");",
                "let y: DynField = b.forward(\"y\");",
                "b.let_(y, defined(z));",
            ],
        ),
        (
            "xs[3]: int\nlet y = all(v in xs: v > 0)\n",
            &["b.let_(y, all(over(xs).named(\"v\"), |v| v.clone().gt(0)));"],
        ),
        (
            "xs[3]: int\nlet y = any(v in xs: v > 0)\n",
            &["b.let_(y, any(over(xs).named(\"v\"), |v| v.clone().gt(0)));"],
        ),
        (
            "xs[3]: int\nlet y = sum(v in xs: v)\n",
            &["b.let_(y, sum_over(over(xs).named(\"v\"), |v| v.clone()));"],
        ),
        (
            "xs[3]: int\nlet y = count(v in xs: v > 0)\n",
            &["b.let_(y, count_over(over(xs).named(\"v\"), |v| v.clone().gt(0)));"],
        ),
    ]);
}

#[test]
fn clippy_hazards_are_avoided_or_allowed_by_name() {
    assert_statements(&[
        // `clippy::erasing_op` and `clippy::identity_op` evaluate bare literals.
        ("x: int\nlet y = x * 0\n", &["b.let_(y, x * lit(0));"]),
        ("x: int\nlet y = x * 1\n", &["b.let_(y, x * lit(1));"]),
        // `clippy::precedence`.
        (
            "x: int\nlet y = x + 1 > 0 && x > 0\n",
            &["b.let_(y, (x + 1).gt(0) & x.gt(0));"],
        ),
    ]);
    let code = code("let y = 3.14\n");
    assert!(
        code.contains("#[allow(clippy::approx_constant)]\n"),
        "{code}"
    );
    assert!(!self::code("x: int\n").contains("#[allow"));
}

#[test]
fn only_equal_operands_carry_the_eq_op_allowance() {
    for expression in equal_operands::EQUAL {
        let code = code(&equal_operands::spec(expression));
        assert!(code.contains("#[allow(clippy::eq_op)]\n"), "{code}");
    }
    for expression in equal_operands::DIFFERENT {
        let code = code(&equal_operands::spec(expression));
        assert!(!code.contains("clippy::eq_op"), "{code}");
    }
}

#[test]
fn only_ascii_reaches_the_generated_text() {
    // rustfmt measures display width, so a length is a width only in ASCII.
    assert_statements(&[
        ("line \"å漢\\n\"\n", &["b.line(\"\\u{e5}\\u{6f22}\\n\");"]),
        ("comment \"é\"\n", &["b.comment([\"\\u{e9}\"]);"]),
    ]);
}

#[test]
fn a_comprehension_variable_is_cut_to_keep_its_closure_on_a_line() {
    let long = "v".repeat(50);
    let code = code(&format!(
        "xs[2]: int\nlet y = sum({long}a in xs: sum({long}b in xs: {long}a + {long}b))\n"
    ));
    let cut = "v".repeat(16);
    assert!(code.contains(&format!("|{cut}| ")), "{code}");
    // Two variables cut to the same text stay two names.
    assert!(code.contains(&format!("|{cut}_2| ")), "{code}");
    // What the spec calls them does not change.
    assert!(code.contains(&format!("\"{long}a\"")), "{code}");
    assert!(code.contains(&format!("\"{long}b\"")), "{code}");
}

#[test]
fn a_narrow_chain_that_passes_the_line_keeps_its_closure_on_its_own_line() {
    let text = "t".repeat(32);
    let spec = format!(
        "{}sections {{\n\"{text}\" optional {{\nx: int\n}}\n}}\n{}",
        "repeat 1 {\n".repeat(7),
        "}\n".repeat(7)
    );
    let chain = format!("a.arm(\"{text}\").optional().body(");
    assert_statements(&[(&spec, &[&chain, "|s| {", "s.read(x, int());", "},", ");"])]);
}

#[test]
fn only_deep_programs_carry_the_attributes_for_depth() {
    let repeats = |count: usize| format!("{}{}", "repeat 1 {\n".repeat(count), "}\n".repeat(count));
    let shallow = code(&repeats(10));
    assert!(!shallow.contains("#[rustfmt::skip]"), "{shallow}");
    assert!(!shallow.contains("recursion_limit"), "{shallow}");
    let deep = code(&repeats(11));
    assert!(deep.contains("#[rustfmt::skip]\npub fn builder"), "{deep}");
    assert!(!deep.contains("recursion_limit"), "{deep}");
    let deepest = code(&repeats(101));
    assert!(
        deepest.starts_with("#![recursion_limit = \"4096\"]\n"),
        "{deepest}"
    );
}

#[test]
fn a_long_if_chain_raises_the_recursion_limit_because_its_type_nests() {
    let chain = |arms: usize| {
        let arms: String = (0..arms).map(|_| " else if x > 1 {\n}").collect();
        format!("x: int\nif x > 0 {{\n}}{arms}\n")
    };
    assert!(!code(&chain(98)).contains("recursion_limit"));
    assert!(code(&chain(99)).contains("recursion_limit"));
}

#[test]
fn identifiers_are_snake_case() {
    assert_statements(&[
        (
            "camelCase: int\n",
            &["let camel_case: DynField = b.forward(\"camelCase\");"],
        ),
        (
            "HTTPServer: int\n",
            &["let http_server: DynField = b.forward(\"HTTPServer\");"],
        ),
    ]);
}

#[test]
fn identifiers_avoid_keywords_and_the_names_of_the_generated_program() {
    assert_statements(&[
        ("fn: int\n", &["let fn_2: DynField = b.forward(\"fn\");"]),
        (
            "Self: int\n",
            &["let self_2: DynField = b.forward(\"Self\");"],
        ),
        ("b: int\n", &["let b_2: DynField = b.forward(\"b\");"]),
        ("int: int\n", &["let int_2: DynField = b.forward(\"int\");"]),
        ("e1: int\n", &["let e1_2: DynField = b.forward(\"e1\");"]),
    ]);
}

#[test]
fn identifiers_stay_distinct() {
    assert_statements(&[(
        "c: int\nC: int\n",
        &[
            "let c: DynField = b.forward(\"c\");",
            "let c_2: DynField = b.forward(\"C\");",
        ],
    )]);
}

#[test]
fn types_and_comprehension_variables_share_the_rules() {
    assert_statements(&[
        (
            "type fn = 1..9\nx: fn\n",
            &[
                "let fn_2 = b.define_type(\"fn\", 1..=9);",
                "b.read(x, fn_2);",
            ],
        ),
        (
            "xs[2]: int\nlet y = sum(s in xs: s)\n",
            &["b.let_(y, sum_over(over(xs).named(\"s\"), |s_2| s_2.clone()));"],
        ),
        (
            "xs[2]: int\nlet y = sum(fn in xs: fn)\n",
            &["b.let_(y, sum_over(over(xs).named(\"fn\"), |fn_2| fn_2.clone()));"],
        ),
    ]);
}

#[test]
fn an_unused_comprehension_variable_is_an_underscore() {
    assert_statements(&[(
        "xs[2]: int\nlet y = sum(v in xs: 1)\n",
        &["b.let_(y, sum_over(over(xs).named(\"v\"), |_| 1));"],
    )]);
}
