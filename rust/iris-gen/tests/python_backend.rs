//! The Python text of each production, each identifier rule and each layout
//! rule that the backend takes from `ruff format` and ruff's default lints.
use gecode_iris_gen::{Language, ViewError, builder_code, view_code};
use gecode_iris_sema::analyze;
use gecode_iris_syntax::{LanguageVersion, parse};

fn code(source: &str) -> String {
    let parsed = parse(source.as_bytes(), "test.iris", LanguageVersion::V0_1).unwrap();
    let analysis = analyze(&parsed);
    builder_code(&parsed, &analysis, Language::Python).expect("the spec is accepted")
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
            window
                .iter()
                .zip(*statements)
                .all(|(line, expected)| line.get(indent..) == Some(*expected))
        });
        assert!(found, "{source:?} lacks {statements:?} in:\n{code}");
    }
}

#[test]
fn the_program_is_a_module_with_a_builder_function() {
    assert_eq!(
        code("format dimacs\nskip line\n"),
        "import gecode_iris.build as ib\n\n\n\
         def builder() -> ib.SpecBuilder:\n    \
         b = ib.SpecBuilder(\"dimacs\")\n    \
         b.skip_line()\n    \
         return b\n\n\n\
         if __name__ == \"__main__\":\n    \
         print(builder().source(), end=\"\")\n"
    );
    assert_statements(&[
        ("", &["return ib.SpecBuilder()"]),
        (
            "format \"two words\"\n",
            &["return ib.SpecBuilder(\"two words\")"],
        ),
        (
            "skip line\nformat late\n",
            &["b.skip_line()", "b.format(\"late\")"],
        ),
    ]);
}

#[test]
fn settings() {
    assert_statements(&[
        ("comment \"c\" \"#\"\n", &["b.comment(\"c\", \"#\")"]),
        ("stop \"EOF\"\n", &["b.stop(\"EOF\")"]),
        ("tokens \"abc\"\n", &["b.tokens(\"abc\")"]),
        ("separators \",;\"\n", &["b.separators(\",;\")"]),
        ("ignore trailing\n", &["b.ignore_trailing()"]),
    ]);
}

#[test]
fn declarations_and_base_types() {
    assert_statements(&[
        (
            "x: int\n",
            &["x = b.forward(\"x\", ib.DynField)", "b.read(x, ib.int_)"],
        ),
        ("x: float\n", &["b.read(x, ib.float_)"]),
        ("x: num\n", &["b.read(x, ib.num)"]),
        ("x: word\n", &["b.read(x, ib.word)"]),
        ("x: rest\n", &["b.read(x, ib.rest)"]),
        ("_: int\n", &["b.read(ib.discard(), ib.int_)"]),
    ]);
}

#[test]
fn ranges() {
    assert_statements(&[
        ("x: 1..9\n", &["b.read(x, ib.int_range(1, 9))"]),
        ("x: 1..<9\n", &["b.read(x, ib.int_range(1, below=9))"]),
        ("x: ..9\n", &["b.read(x, ib.int_range(hi=9))"]),
        ("x: ..<9\n", &["b.read(x, ib.int_range(below=9))"]),
        ("x: 1..\n", &["b.read(x, ib.at_least(1))"]),
        ("x: -1..1\n", &["b.read(x, ib.int_range(-ib.lit(1), 1))"]),
    ]);
}

#[test]
fn arrays_lists_and_alphabets() {
    assert_statements(&[
        ("x[3]: int\n", &["b.read(x, ib.int_.dim(3))"]),
        ("x[2][3]: int\n", &["b.read(x, ib.int_.dim(2).dim(3))"]),
        (
            "x[3]: int offset 1\n",
            &["b.read(x, ib.int_.dim(3).offset(1))"],
        ),
        (
            "x[2][2]: int layout \"LOWER_COL\"\n",
            &["b.read(x, ib.int_.dim(2).dim(2).layout(\"LOWER_COL\"))"],
        ),
        ("list xs: int until 0\n", &["b.list(xs, ib.int_.until(0))"]),
        (
            "list xs: int until eol\n",
            &["b.list(xs, ib.int_.until(ib.eol))"],
        ),
        (
            "list xs: int until 0 offset 1\n",
            &["b.list(xs, ib.int_.until(0).offset(1))"],
        ),
        (
            "x[2][3]: alphabet \"abc\"\n",
            &["b.read(x, ib.alphabet(\"abc\").dim(2).dim(3))"],
        ),
        (
            "x[2][2]: alphabet \"abc\" blank \".\" strip \" \" base 0\n",
            &["b.read(x, ib.alphabet(\"abc\", blank=\".\", strip=\" \", base=0).dim(2).dim(2))"],
        ),
        // Options out of the keywords' order chain in their own order.
        (
            "x[2][2]: alphabet \"abc\" base 0 blank \".\"\n",
            &["b.read(x, ib.alphabet(\"abc\").base(0).blank(\".\").dim(2).dim(2))"],
        ),
    ]);
}

#[test]
fn values_and_checks() {
    assert_statements(&[
        ("let x = 1\n", &["b.let(x, 1)"]),
        ("param x\n", &["b.param(x)"]),
        ("param x = 1\n", &["b.param(x, 1)"]),
        (
            "type t = 1..9\nx: t\n",
            &[
                "t = b.define_type(\"t\", ib.int_range(1, 9))",
                "b.read(x, t)",
            ],
        ),
        (
            "type t = 1..9\n",
            &["b.define_type(\"t\", ib.int_range(1, 9))"],
        ),
        ("check 1 == 1\n", &["b.check(ib.lit(1) == 1)"]),
        (
            "check 1 == 1 \"message\"\n",
            &["b.check(ib.lit(1) == 1, \"message\")"],
        ),
        // A float or string condition is an expression, not a Python value.
        ("check \"s\"\n", &["b.check(ib.lit(\"s\"))"]),
    ]);
}

#[test]
fn lines_and_patterns() {
    assert_statements(&[
        ("expect \"a\" \"b\"\n", &["b.expect(\"a\", \"b\")"]),
        ("find \"a\"\n", &["b.find(\"a\")"]),
        ("line \"\"\n", &["b.line(\"\")"]),
        ("skip line\n", &["b.skip_line()"]),
        ("skip 2 lines\n", &["b.skip_lines(2)"]),
        ("index 2\n", &["b.index(2)"]),
        (
            "line \"a {x:int} b\"\n",
            &["b.line((\"a \", ib.bind(x, ib.int_), \" b\"))"],
        ),
        ("line \"{x}\"\n", &["b.line((ib.bind(x),))"]),
        ("n: int\nline \"{$n}\"\n", &["b.line((ib.ref(n),))"]),
        (
            "line \"{x:int}{_}{_:int}{}{:int}...\"\n",
            &[
                "e1 = (",
                "    ib.bind(x, ib.int_),",
                "    ib.discard(),",
                "    ib.discard_as(ib.int_),",
                "    ib.blank(),",
                "    ib.validate(ib.int_),",
                "    ib.rest_of_line,",
                ")",
                "b.line(e1)",
            ],
        ),
    ]);
}

#[test]
fn repeats() {
    assert_statements(&[
        (
            "repeat 2 {\n  x: int\n}\n",
            &["with b.repeat(2) as s1:", "    s1.read(x, ib.int_)"],
        ),
        (
            "repeat until eof {\n  x: int\n}\n",
            &[
                "with b.repeat_until_eof() as s1:",
                "    s1.read(x, ib.int_)",
            ],
        ),
        (
            "repeat until \"END\" \"STOP\" {\n  x: int\n}\n",
            &["with b.repeat_until(\"END\", \"STOP\") as s1:"],
        ),
        (
            "t: word\nrepeat until \"END\" store t {\n  x: int\n}\n",
            &["with b.repeat_until(\"END\", store=t) as s1:"],
        ),
        (
            "repeat 2 as r {\n  each c\n  x: int\n}\n",
            &[
                "with b.repeat(2) as s1:",
                "    s1.each_name(\"c\")",
                "    s1.as_name(\"r\")",
                "    s1.read(x, ib.int_)",
            ],
        ),
        (
            "repeat 2 as r {\n  x: int\n}\nlet y = r[0].x\n",
            &[
                "record_1 = b.forward_record(\"record_1\", \"r\")",
                "with b.repeat(2) as s1:",
                "    s1.record_as(record_1)",
                "    s1.read(x, ib.int_)",
                "b.let(y, record_1.list[0][x])",
            ],
        ),
        // A guard that is never entered writes an empty block.
        ("repeat 2 {\n}\n", &["b.repeat(2)"]),
    ]);
}

#[test]
fn sections() {
    assert_statements(&[
        (
            "sections {\n  \"a\" {\n    x: int\n  }\n  \"b\"\n}\n",
            &[
                "with b.sections() as a1:",
                "    with a1.arm(\"a\") as s2:",
                "        s2.read(x, ib.int_)",
                "    a1.arm(\"b\")",
            ],
        ),
        (
            "t: word\nsections until \"END\" store t {\n  \"a\" optional {\n    x: int\n  }\n  \"b\" last\n}\n",
            &[
                "with b.sections(until=\"END\", store=t) as a1:",
                "    with a1.arm(\"a\", optional=True) as s2:",
                "        s2.read(x, ib.int_)",
                "    a1.arm(\"b\", last=True)",
            ],
        ),
        (
            "sections until \"A\" \"B\" skip other {\n  \"a\"\n}\n",
            &[
                "with b.sections(until=(\"A\", \"B\"), skip_other=True) as a1:",
                "    a1.arm(\"a\")",
            ],
        ),
        // Clauses out of the keywords' order chain.
        (
            "sections skip other until \"A\" {\n  \"a\"\n}\n",
            &["with b.sections().skip_other().until(\"A\") as a1:"],
        ),
        // An arm's empty block is entered, because an arm without one has none.
        (
            "sections {\n  \"a\" {\n  }\n  \"b\"\n}\n",
            &[
                "with b.sections() as a1:",
                "    with a1.arm(\"a\"):",
                "        pass",
            ],
        ),
        ("sections {\n}\n", &["b.sections()"]),
    ]);
}

#[test]
fn instances() {
    assert_statements(&[(
        "instances {\n  x: int\n}\n",
        &["with b.instances() as s1:", "    s1.read(x, ib.int_)"],
    )]);
}

#[test]
fn if_chains() {
    assert_statements(&[
        (
            "x: int\nif x > 0 {\n  y: int\n} else if x < 0 {\n  z: int\n} else {\n  w: int\n}\n",
            &[
                "with b.if_(x > 0) as s1:",
                "    s1.read(y, ib.int_)",
                "with b.else_if(x < 0) as s1:",
                "    s1.read(z, ib.int_)",
                "with b.else_() as s1:",
                "    s1.read(w, ib.int_)",
            ],
        ),
        (
            "x: int\nif x > 0 {\n  y: int\n} else if: int\n",
            &[
                "if_ = b.forward(\"if\", ib.DynField)",
                "b.read(x, ib.int_)",
                "with b.if_(x > 0) as s1:",
                "    s1.read(y, ib.int_)",
                "b.else_read_if(if_, ib.int_)",
            ],
        ),
        (
            "x: int\nif x > 0 {\n} else {\n  y: int\n}\n",
            &[
                "b.if_(x > 0)",
                "with b.else_() as s1:",
                "    s1.read(y, ib.int_)",
            ],
        ),
    ]);
}

#[test]
fn a_block_that_is_all_of_its_body_joins_its_with() {
    assert_statements(&[
        (
            "instances {\n  repeat 2 {\n    x: int\n  }\n}\n",
            &[
                "with b.instances() as s1, s1.repeat(2) as s2:",
                "    s2.read(x, ib.int_)",
            ],
        ),
        (
            "sections {\n  \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\" {\n    repeat 2 {\n      x: int\n    }\n  }\n}\n",
            &[
                "with (",
                "    b.sections() as a1,",
                "    a1.arm(\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\") as s2,",
                "    s2.repeat(2) as s3,",
                "):",
                "    s3.read(x, ib.int_)",
            ],
        ),
        // A repeat's names are statements of its body, so nothing joins.
        (
            "repeat 2 as r {\n  repeat 3 {\n    x: int\n  }\n}\n",
            &[
                "with b.repeat(2) as s1:",
                "    s1.as_name(\"r\")",
                "    with s1.repeat(3) as s2:",
            ],
        ),
    ]);
}

#[test]
fn deep_blocks_become_helper_functions() {
    let source = format!("{}x: int\n{}", "repeat 1 {\n".repeat(20), "}\n".repeat(20));
    let code = code(&source);
    assert!(code.contains("\n    def block_1(s"), "{code}");
    assert!(code.contains(": ib.ScopeN[None]) -> None:\n"), "{code}");
    assert!(code.contains("block_1(s"), "{code}");
}

#[test]
fn types_defined_in_a_helper_stay_in_it() {
    let source = format!(
        "{}type t = 1..9\ntype u = t\nx: u\n{}",
        "repeat 1 {\n".repeat(21),
        "}\n".repeat(21)
    );
    let code = code(&source);
    assert!(
        code.contains("def block_1(s17: ib.ScopeN[None]) -> None:"),
        "{code}"
    );
    assert_statements(&[(
        &source,
        &[
            "t = s21.define_type(\"t\", ib.int_range(1, 9))",
            "u = s21.define_type(\"u\", t)",
            "s21.read(x, u)",
        ],
    )]);
}

#[test]
fn types_of_one_helper_are_parameters_of_the_next() {
    // `a` is defined in `block_1`, used through `b` in `block_2`, and passed
    // on to `block_3`; `top` is a variable of `builder`, which every helper
    // sees.
    let repeats = |count: usize| "repeat 1 {\n".repeat(count);
    let source = format!(
        "type top = 0..9\n{}type a = 1..9\ntype f = float\ntype n = num\n\
         type w = word\n{}type b = a\n{}x: b\ny: f\nz: n\nv: w\nk: top\n{}",
        repeats(18),
        repeats(20),
        repeats(20),
        "}\n".repeat(58)
    );
    assert_statements(&[
        (&source, &["block_2(s34, a, f, n, w)"]),
        (
            &source,
            &[
                "def block_2(",
                "    s34: ib.ScopeN[None],",
                "    a: ib.IntType[int],",
                "    f: ib.FloatType[float],",
                "    n: ib.NumType[int | float],",
                "    w: ib.StrType[str],",
                ") -> None:",
            ],
        ),
        (&source, &["b_ = s38.define_type(\"b\", a)"]),
        (&source, &["block_3(s51, b_, f, n, w)"]),
        (&source, &["s58.read(x, b_)", "s58.read(y, f)"]),
        (&source, &["s58.read(k, top)"]),
    ]);
    let code = code(&source);
    assert!(!code.contains("top: ib"), "{code}");
}

#[test]
fn operators() {
    assert_statements(&[
        (
            "let x = 1 + 2 * 3\n",
            &["b.let(x, ib.lit(1) + ib.lit(2) * 3)"],
        ),
        ("let x = (1 + 2) * 3\n", &["b.let(x, (ib.lit(1) + 2) * 3)"]),
        (
            "let x = 1 - (2 - 3)\n",
            &["b.let(x, ib.lit(1) - (ib.lit(2) - 3))"],
        ),
        ("x: int\nlet y = -x\n", &["b.let(y, -x)"]),
        ("let y = -1\n", &["b.let(y, -ib.lit(1))"]),
        ("x: int\nlet y = !x\n", &["b.let(y, ~x)"]),
        ("x: int\nlet y = x / 2\n", &["b.let(y, ib.div(x, 2))"]),
        ("x: int\nlet y = x % 2\n", &["b.let(y, ib.rem(x, 2))"]),
        ("x: int\nlet y = x ^ 2\n", &["b.let(y, ib.pow(x, 2))"]),
        // Python's `&` and `|` bind more tightly than its comparisons.
        (
            "x: int\nlet y = x > 1 && x < 3\n",
            &["b.let(y, (x > 1) & (x < 3))"],
        ),
        (
            "x: int\nlet y = x > 1 || x < 3\n",
            &["b.let(y, (x > 1) | (x < 3))"],
        ),
        // Python chains comparisons.
        (
            "x: int\nlet y = (x < 1) == 0\n",
            &["b.let(y, (x < 1) == 0)"],
        ),
        ("let y = gecode.int_max\n", &["b.let(y, ib.gecode.INT_MAX)"]),
        ("let y = gecode.int_min\n", &["b.let(y, ib.gecode.INT_MIN)"]),
        ("let y = 2.5\n", &["b.let(y, 2.5)"]),
        ("let y = \"s\"\n", &["b.let(y, \"s\")"]),
        // `IntExpr` takes no string, and `+` no float or string.
        ("x: int\nlet y = x == \"s\"\n", &["b.let(y, x == \"s\")"]),
        (
            "let y = 1 == \"s\"\n",
            &["b.let(y, ib.lit(1).dynamic() == \"s\")"],
        ),
        (
            "let y = 1 + 2.5\n",
            &["b.let(y, ib.lit(1) + ib.lit(2.5).dynamic())"],
        ),
    ]);
}

#[test]
fn ruff_hazards_are_avoided() {
    assert_statements(&[
        // B002 takes `-(-x)` for a mistaken `--x`.
        ("x: int\nlet y = --x\n", &["b.let(y, -(-x).dynamic())"]),
        // PLR0124 takes a name compared with itself for a mistake.
        ("x: int\nlet y = x == x\n", &["b.let(y, x.dynamic() == x)"]),
    ]);
}

#[test]
fn functions_and_comprehensions() {
    assert_statements(&[
        ("xs[3]: int\nlet y = sum(xs)\n", &["b.let(y, ib.sum(xs))"]),
        ("xs[3]: int\nlet y = size(xs)\n", &["b.let(y, ib.size(xs))"]),
        (
            "xs[3]: int\nlet y = count(xs)\n",
            &["b.let(y, ib.count(xs))"],
        ),
        (
            "xs[3]: int\nlet y = min(xs, 1)\n",
            &["b.let(y, ib.min(xs, 1))"],
        ),
        ("xs[3]: int\nlet y = max(xs)\n", &["b.let(y, ib.max(xs))"]),
        (
            "xs[3]: int\nlet y = distinct(xs)\n",
            &["b.let(y, ib.distinct(xs))"],
        ),
        ("xs[3]: int\nlet y = xs[0]\n", &["b.let(y, xs[0])"]),
        (
            "let y = defined(z)\n",
            &[
                "z = b.undefined(\"z\")",
                "y = b.forward(\"y\", ib.DynField)",
                "b.let(y, ib.defined(z))",
            ],
        ),
        (
            "xs[3]: int\nlet y = all(v in xs: v > 0)\n",
            &["b.let(y, ib.all(ib.over(xs, \"v\"), lambda v: v > 0))"],
        ),
        (
            "xs[3]: int\nlet y = any(v in xs: 1)\n",
            &["b.let(y, ib.any(ib.over(xs, \"v\"), lambda _: 1))"],
        ),
        (
            "xs[3]: int\nlet y = sum(v in xs: v)\n",
            &["b.let(y, ib.sum(ib.over(xs, \"v\"), lambda v: v))"],
        ),
        (
            "xs[3]: int\nlet y = count(v in xs: v > 0)\n",
            &["b.let(y, ib.count(ib.over(xs, \"v\"), lambda v: v > 0))"],
        ),
    ]);
}

#[test]
fn a_body_too_wide_for_a_lambda_becomes_a_function() {
    let long = "n".repeat(24);
    let source =
        format!("{long}[3]: int\nlet y = all(v in {long}: v > {long} + {long} + {long})\n");
    let code = code(&source);
    assert!(
        code.contains("    def body_1(v: ib.DynExpr) -> ib.Expr:\n"),
        "{code}"
    );
    assert!(code.contains("        return v > "), "{code}");
    assert!(code.contains(", body_1)"), "{code}");
}

#[test]
fn calls_split_as_ruff_splits_them() {
    let long = "t".repeat(74);
    let longer = "t".repeat(90);
    assert_statements(&[
        // The arguments fit on a line of their own.
        (
            &format!("comment \"{long}\"\n"),
            &["b.comment(", &format!("    \"{long}\""), ")"],
        ),
        // One to a line, with a trailing comma.
        (
            &format!("comment \"{long}\" \"{long}\"\n"),
            &[
                "b.comment(",
                &format!("    \"{long}\","),
                &format!("    \"{long}\","),
                ")",
            ],
        ),
        // A string longer than the line stays whole.
        (
            &format!("comment \"{longer}\"\n"),
            &["b.comment(", &format!("    \"{longer}\""), ")"],
        ),
    ]);
}

#[test]
fn identifiers_follow_python_rules() {
    assert_statements(&[
        // Keywords and names that the program uses get an underscore.
        (
            "class: int\n",
            &["class_ = b.forward(\"class\", ib.DynField)"],
        ),
        (
            "b: int\n",
            &["b_ = b.forward(\"b\", ib.DynField)", "b.read(b_, ib.int_)"],
        ),
        ("ib: int\n", &["ib_ = b.forward(\"ib\", ib.DynField)"]),
        // Soft keywords and builtins: the program uses `int` and `print`.
        (
            "match: int\ncase: int\n",
            &[
                "match_ = b.forward(\"match\", ib.DynField)",
                "case_ = b.forward(\"case\", ib.DynField)",
            ],
        ),
        (
            "list: int\nint: int\nprint: int\nframe: int\n",
            &[
                "list_ = b.forward(\"list\", ib.DynField)",
                "int_ = b.forward(\"int\", ib.DynField)",
                "print_ = b.forward(\"print\", ib.DynField)",
                "frame = b.forward(\"frame\", ib.DynField)",
            ],
        ),
        (
            "type sum = 1..9\nx: sum\n",
            &["sum_ = b.define_type(\"sum\", ib.int_range(1, 9))"],
        ),
        (
            "a: int\nlet x = count(len in a: len > 0)\n",
            &["b.let(x, ib.count(ib.over(a, \"len\"), lambda len_: len_ > 0))"],
        ),
        // E741 refuses `l`, `O` and `I`.
        ("l: int\n", &["l_ = b.forward(\"l\", ib.DynField)"]),
        // A name like a temporary or a scope is taken by the program.
        (
            "e1: int\ns1: int\n",
            &[
                "e1_ = b.forward(\"e1\", ib.DynField)",
                "s1_ = b.forward(\"s1\", ib.DynField)",
            ],
        ),
        // Types share one namespace with values in Python.
        (
            "type b = 1..9\ntype c = b\nx: c\n",
            &[
                "b_ = b.define_type(\"b\", ib.int_range(1, 9))",
                "c = b.define_type(\"c\", b_)",
            ],
        ),
        // A record's identifier comes from its `each` name.
        (
            "repeat 2 as rs {\n  each r\n  x: int\n}\nlet y = rs[0].x\n",
            &["r = b.forward_record(\"r\", \"rs\")"],
        ),
    ]);
}

#[test]
fn strings_are_ascii() {
    assert_statements(&[
        ("comment \"\u{e9}\"\n", &["b.comment(\"\\xe9\")"]),
        ("comment \"\u{20ac}\"\n", &["b.comment(\"\\u20ac\")"]),
        ("comment \"say \\\"hi\\\"\"\n", &["b.comment('say \"hi\"')"]),
    ]);
}

#[test]
fn python_has_no_view_printer() {
    assert!(Language::Rust.has_view());
    assert!(!Language::Python.has_view());
    let parsed = parse(b"x: int\n", "test.iris", LanguageVersion::V0_1).unwrap();
    let analysis = analyze(&parsed);
    let error = view_code(&analysis, Language::Python).unwrap_err();
    assert_eq!(error, ViewError::NoPrinter(Language::Python));
    assert_eq!(error.to_string(), "python has no view generator yet");
    assert!(view_code(&analysis, Language::Rust).is_ok());
}
