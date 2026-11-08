//! The C++ text of each production, each identifier rule and each layout rule
//! that the backend takes from `clang-format` in LLVM style.
use gecode_iris_gen::{Language, ViewError, builder_code, view_code};
use gecode_iris_sema::analyze;
use gecode_iris_syntax::{LanguageVersion, parse};

fn code(source: &str) -> String {
    let parsed = parse(source.as_bytes(), "test.iris", LanguageVersion::V0_1).unwrap();
    let analysis = analyze(&parsed);
    builder_code(&parsed, &analysis, Language::Cpp).expect("the spec is accepted")
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
            window
                .iter()
                .zip(*statements)
                .all(|(line, expected)| line.get(indent..) == Some(*expected))
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
        "#include <iris/build.hpp>\n\n\
         #include <exception>\n\
         #include <iostream>\n\n\
         namespace {\n\n\
         using namespace iris::build;\n\n\
         SpecBuilder builder() {\n  \
         SpecBuilder b(\"dimacs\");\n  \
         b.skip_line();\n  \
         return b;\n\
         }\n\n\
         } // namespace\n\n\
         int main() {\n  \
         try {\n    \
         std::cout << builder().source();\n  \
         } catch (const std::exception &error) {\n    \
         std::cerr << error.what() << '\\n';\n    \
         return 1;\n  \
         }\n  \
         return 0;\n\
         }\n"
    );
    assert_statements(&[
        ("", &["SpecBuilder b;", "return b;"]),
        ("format \"two words\"\n", &["SpecBuilder b(\"two words\");"]),
        (
            "skip line\nformat late\n",
            &["b.skip_line();", "b.format(\"late\");"],
        ),
    ]);
}

#[test]
fn handles_come_before_the_statements() {
    assert_statements(&[
        (
            "let y = x\nx: int\n",
            &[
                "SpecBuilder b;",
                "auto x = b.forward<DynField>(\"x\");",
                "auto y = b.forward<DynField>(\"y\");",
                "b.let(y, x);",
                "b.read(x, int_);",
            ],
        ),
        (
            "repeat 2 as jobs {\n  each job\n  m: int\n}\nlet t = size(jobs)\n",
            &[
                "auto m = b.forward<DynField>(\"m\");",
                "auto t = b.forward<DynField>(\"t\");",
                "auto job = b.forward_record(\"job\", \"jobs\");",
                "b.repeat(2, [&](auto &s1) {",
                "  s1.record_as(job);",
                "  s1.read(m, int_);",
                "});",
                "b.let(t, size(job.list()));",
            ],
        ),
    ]);
}

#[test]
fn settings() {
    assert_statements(&[
        ("comment \"c\" \"#\"\n", &["b.comment({\"c\", \"#\"});"]),
        ("stop \"EOF\"\n", &["b.stop(\"EOF\");"]),
        ("tokens \"ab\"\n", &["b.tokens(\"ab\");"]),
        ("separators \",\"\n", &["b.separators(\",\");"]),
        ("ignore trailing\n", &["b.ignore_trailing();"]),
    ]);
}

#[test]
fn declarations_and_types() {
    assert_statements(&[
        (
            "a: int\nb: float\nc: num\nd: word\ne: rest\n",
            &[
                "b.read(a, int_);",
                "b.read(b_2, float_);",
                "b.read(c, num);",
                "b.read(d, word);",
                "b.read(e, rest);",
            ],
        ),
        ("x: 1..\n", &["b.read(x, Range{.from = 1});"]),
        ("x: ..9\n", &["b.read(x, Range{.to = 9});"]),
        (
            "n: int\nx: 0..<n\n",
            &["b.read(x, Range{.from = 0, .below = n});"],
        ),
        (
            "n: int\nxs[n][2]: int\n",
            &["b.read(xs, int_.dim(n).dim(2));"],
        ),
        (
            "list ys: float until eol\n",
            &["b.list(ys, float_.until(eol));"],
        ),
        ("list ys: int until 0\n", &["b.list(ys, int_.until(0));"]),
        (
            "q[1][2]: alphabet \"ab\" blank \".\" base 1\n",
            &["b.read(q, alphabet(\"ab\").blank(\".\").base(1).dim(1).dim(2));"],
        ),
        (
            "type small = 1..9\nxs[2]: small\n",
            &[
                "auto small = b.define_type(\"small\", Range{.from = 1, .to = 9});",
                "b.read(xs, ty(small).dim(2));",
            ],
        ),
        ("type unused = int\n", &["b.define_type(\"unused\", int_);"]),
        ("_: int\n", &["b.read(discard, int_);"]),
    ]);
}

#[test]
fn values_and_checks() {
    assert_statements(&[
        ("param p = 3\n", &["b.param(p, 3);"]),
        ("param p\n", &["b.param(p);"]),
        ("x: int\ncheck x > 0\n", &["b.check(x > 0);"]),
        (
            "x: int\ncheck x > 0 \"positive\"\n",
            &["b.check(x > 0, \"positive\");"],
        ),
        ("expect \"begin\"\n", &["b.expect(\"begin\");"]),
        ("expect \"a\" \"b\"\n", &["b.expect({\"a\", \"b\"});"]),
        ("skip 2 lines\n", &["b.skip_lines(2);"]),
    ]);
}

#[test]
fn lines_and_patterns() {
    assert_statements(&[
        ("line \"begin\"\n", &["b.line(\"begin\");"]),
        ("find \"x\"\n", &["b.find(\"x\");"]),
        (
            "line \"size {n:int} of {name:word}\"\n",
            &["b.line(pattern(\"size {} of {}\", bind(n, int_), bind(name, word)));"],
        ),
        (
            "line \"{x}\"\n",
            &["b.line(pattern(\"{}\", bind(x, untyped)));"],
        ),
        (
            "n: int\nline \"{$n}\"\n",
            &["b.line(pattern(\"{}\", ref(n)));"],
        ),
        (
            "line \"{x:int}{_}{_:int}{}{:int}...\"\n",
            &[
                "auto e1 = pattern(\"{}{}{}{}{}{}\", bind(x, int_), discard, discard_as(int_),",
                "                  blank, validate(int_), rest_of_line);",
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
            &["b.repeat(2, [&](auto &s1) { s1.read(x, int_); });"],
        ),
        (
            "repeat until eof {\n  x: int\n}\n",
            &["b.repeat_until_eof([&](auto &s1) { s1.read(x, int_); });"],
        ),
        (
            "repeat until \"end\" store last {\n  w: word\n}\n",
            &["b.repeat_until(\"end\", {.store = last}, [&](auto &s1) { s1.read(w, word); });"],
        ),
        (
            "repeat 2 as rows {\n  each row\n  x: int\n  y: int\n}\n",
            &[
                "b.repeat(2, [&](auto &s1) {",
                "  s1.each_name(\"row\");",
                "  s1.as_name(\"rows\");",
                "  s1.read(x, int_);",
                "  s1.read(y, int_);",
                "});",
            ],
        ),
        ("repeat 2 {\n}\n", &["b.repeat(2, [&](auto &) {});"]),
    ]);
}

#[test]
fn sections_take_one_option_per_clause() {
    assert_statements(&[
        (
            "sections {\n  \"a\" {\n    y: int\n  }\n}\n",
            &[
                "b.sections(",
                "    [&](auto &a1) { a1.arm(\"a\", [&](auto &s2) { s2.read(y, int_); }); });",
            ],
        ),
        (
            "x: int\nsections until \"x\" store x skip other {\n  \"b\"\n}\n",
            &[
                "b.sections({{.until = \"x\", .store = x}, {.skip_other = true}},",
                "           [&](auto &a1) { a1.arm(\"b\"); });",
            ],
        ),
        (
            "sections skip other {\n  \"b\" optional\n  \"c\" last\n}\n",
            &[
                "b.sections({.skip_other = true}, [&](auto &a1) {",
                "  a1.optional_arm(\"b\");",
                "  a1.arm(\"c\", {.last = true});",
                "});",
            ],
        ),
    ]);
}

#[test]
fn instances() {
    assert_statements(&[(
        "instances {\n  x: int\n}\n",
        &["b.instances([&](auto &s1) { s1.read(x, int_); });"],
    )]);
}

#[test]
fn if_chains_keep_the_if_in_a_variable() {
    assert_statements(&[
        (
            "x: int\nif x > 1 {\n  a: int\n}\n",
            &["b.if_(x > 1, [&](auto &s1) { s1.read(a, int_); });"],
        ),
        (
            "x: int\nif x > 1 {\n  a: int\n} else if x < 0 {\n  b: word\n} else {\n  c: num\n}\n",
            &[
                "auto c1 = b.if_(x > 1, [&](auto &s1) { s1.read(a, int_); });",
                "c1.else_if(x < 0, [&](auto &s1) { s1.read(b_2, word); });",
                "c1.else_([&](auto &s1) { s1.read(c, num); });",
            ],
        ),
        (
            "x: int\nif x > 1 {\n  a: int\n} else if: word\n",
            &[
                "auto c1 = b.if_(x > 1, [&](auto &s1) { s1.read(a, int_); });",
                "c1.else_read_if(if_2, word);",
            ],
        ),
        (
            "x: int\nif x > 0 {\n} else {\n  y: int\n}\n",
            &[
                "auto c1 = b.if_(x > 0, [&](auto &) {});",
                "c1.else_([&](auto &s1) { s1.read(y, int_); });",
            ],
        ),
    ]);
}

#[test]
fn deep_blocks_become_helper_lambdas_with_their_types() {
    let source = format!(
        "type t = 1..3\nx: int\n{}y: t\n{}",
        repeated("repeat 1 {\n", 11),
        repeated("}\n", 11)
    );
    assert_statements(&[(
        source.as_str(),
        &[
            "auto block_1 = [&](Scope<10, Presence::Required> &s10, const auto &t) {",
            "  s10.repeat(1, [&](auto &s11) { s11.read(y, t); });",
            "};",
        ],
    )]);
    assert_statements(&[(
        source.as_str(),
        &[
            "auto t = b.define_type(\"t\", Range{.from = 1, .to = 3});",
            "b.read(x, int_);",
        ],
    )]);
    assert_statements(&[(
        source.as_str(),
        &["s9.repeat(1, [&](auto &s10) { block_1(s10, t); });"],
    )]);
    let maybe = format!(
        "x: int\nif x > 0 {{\n{}y: int\n{}}}\n",
        repeated("repeat 1 {\n", 10),
        repeated("}\n", 10)
    );
    assert_statements(&[(
        maybe.as_str(),
        &["auto block_1 = [&](Scope<9, Presence::Maybe> &s10) {"],
    )]);
    assert_statements(&[(
        maybe.as_str(),
        &["s8.repeat(1, [&](auto &s9) { s9.repeat(1, block_1); });"],
    )]);
}

#[test]
fn operators_satisfy_the_builder_concepts() {
    assert_statements(&[
        (
            "x: int\nlet y = -x + 2 * x - (x - 1)\n",
            &["b.let(y, -x + lit(2) * x - (x - 1));"],
        ),
        ("x: int\nlet y = 1.5 < x\n", &["b.let(y, lit(1.5) < x);"]),
        (
            "x: int\nlet y = 2 ^ x\n",
            &["b.let(y, iris::build::pow(2, x));"],
        ),
        (
            "x: int\nlet y = gecode.int_max - x\n",
            &["b.let(y, host::int_max - x);"],
        ),
        (
            "x: int\nlet y = !(x > 1) || x == 2 && x != 3\n",
            &["b.let(y, !(x > 1) || (x == 2 && x != 3));"],
        ),
        ("x: int\nlet y = -(-x)\n", &["b.let(y, -(-x));"]),
        (
            "x: int\nlet y = (x == 1) == (x == 2)\n",
            &["b.let(y, (x == 1) == (x == 2));"],
        ),
        (
            "let y = \"a\" == \"b\"\n",
            &["b.let(y, lit(\"a\") == \"b\");"],
        ),
        ("x: float\nlet y = x % 2\n", &["b.let(y, x % 2);"]),
        (
            "let x = 1.5\nlet y = x - 1 == \"a\"\n",
            &["b.let(y, (x - 1).dynamic() == \"a\");"],
        ),
    ]);
}

#[test]
fn host_constants_become_expressions_where_one_is_needed() {
    assert_statements(&[
        (
            "let y = size(gecode.int_max)\n",
            &["b.let(y, size(host::int_max.dynamic()));"],
        ),
        (
            "let y = count(gecode.int_min)\n",
            &["b.let(y, count(host::int_min.dynamic()));"],
        ),
        (
            "let y = gecode.int_max[0]\n",
            &["b.let(y, host::int_max.dynamic()[0]);"],
        ),
        (
            "let y = gecode.int_max == \"s\"\n",
            &["b.let(y, host::int_max.dynamic() == \"s\");"],
        ),
        (
            "let y = sum(v in gecode.int_max: v)\n",
            &[
                "auto e1 = sum(over(host::int_max.dynamic()).named(\"v\"),",
                "              [&](const auto &v) { return v; });",
                "b.let(y, e1);",
            ],
        ),
        (
            "let y = -gecode.int_min + gecode.int_max\n",
            &["b.let(y, -host::int_min + host::int_max);"],
        ),
    ]);
}

#[test]
fn functions_records_and_comprehensions() {
    assert_statements(&[
        (
            "xs[2]: int\nlet a = min(xs[0], xs[1])\n",
            &["b.let(a, min(xs[0], xs[1]));"],
        ),
        ("xs[2]: int\nlet a = size(xs)\n", &["b.let(a, size(xs));"]),
        (
            "xs[2]: int\nlet a = defined(xs)\n",
            &["b.let(a, defined(xs));"],
        ),
        (
            "xs[2]: int\nlet c = all(v in xs: v > 0)\n",
            &["b.let(c, all(over(xs).named(\"v\"), [&](const auto &v) { return v > 0; }));"],
        ),
        (
            "xs[2]: int\nlet c = count(v in xs: 1)\n",
            &["b.let(c, count(over(xs).named(\"v\"), [&](const auto &) { return 1; }));"],
        ),
        (
            "repeat 2 as jobs {\n  each job\n  repeat 2 as steps {\n    each step\n    \
             m: int\n  }\n}\nlet y = jobs[0].steps[1].m\n",
            &["b.let(y, job.list()[0].records(step)[1].get(m));"],
        ),
        (
            "repeat 2 as jobs {\n  each job\n  repeat 2 as steps {\n    each step\n    \
             m: int\n  }\n}\nlet x = sum(j in jobs: sum(s in j.steps: s.m))\n",
            &[
                "auto e2 = sum(over(job.list()).named(\"j\"), [&](const auto &j) {",
                "  auto e1 = sum(over(j.records(step)).named(\"s\"),",
                "                [&](const auto &s) { return s.get(m); });",
                "  return e1;",
                "});",
                "b.let(x, e2);",
            ],
        ),
    ]);
}

#[test]
fn comprehensions_too_deep_become_helper_lambdas() {
    let source = format!(
        "xs[2]: int\n{}let y = sum(v in xs: v + 1)\n{}",
        repeated("repeat 1 {\n", 9),
        repeated("}\n", 9)
    );
    assert_statements(&[(
        source.as_str(),
        &["auto body_1 = [&](const Expr<Dynamic, Open> &v) { return v + 1; };"],
    )]);
    assert_statements(&[(
        source.as_str(),
        &["s9.let(y, sum(over(xs).named(\"v\"), body_1));"],
    )]);
}

#[test]
fn layouts_have_the_fewest_lines_clang_format_allows() {
    let long = "n".repeat(24);
    let cases = [
        // Aligned after the bracket.
        (
            format!(
                "{long}: int\nx: int\ncheck x > {long} \"{}\"\n",
                "m".repeat(40)
            ),
            vec![
                format!("b.check(x > {long},"),
                format!("        \"{}\");", "m".repeat(40)),
            ],
        ),
        // A break after `=` before the call.
        (
            format!("{long}: int\nx: int\nif {long} > 0 {{\n  x: int\n}} else {{\n  x: int\n}}\n"),
            vec![
                "auto c1 =".to_owned(),
                format!("    b.if_({long} > 0, [&](auto &s1) {{ s1.read(x, int_); }});"),
            ],
        ),
        // A break after the bracket, with the arguments packed.
        (
            format!(
                "{long}: int\nx: int\ncheck x > {long} \"{}\"\n",
                "m".repeat(68)
            ),
            vec![
                "b.check(".to_owned(),
                format!("    x > {long},"),
                format!("    \"{}\");", "m".repeat(68)),
            ],
        ),
        // An argument that clang-format could split goes into a temporary
        // rather than take a break after the bracket.
        (
            format!("type {} = 1..9\nx: {}\n", "n".repeat(33), "n".repeat(33)),
            vec![
                "auto e1 = Range{.from = 1, .to = 9};".to_owned(),
                format!("auto {long} ="),
                format!("    b.define_type(\"{}\", e1);", "n".repeat(33)),
            ],
        ),
    ];
    for (source, lines) in &cases {
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        assert_statements(&[(source.as_str(), &lines)]);
    }
}

#[test]
fn identifiers_are_lower_snake_case_and_never_reserved() {
    assert_statements(&[
        (
            "if: int\nint: int\ne1: int\ns1: int\ncount: int\nnJobs: int\nHTTPServer: int\n\
             std: int\nmain: int\nb: int\n",
            &[
                "b.read(if_2, int_);",
                "b.read(int_2, int_);",
                "b.read(e1_2, int_);",
                "b.read(s1_2, int_);",
                "b.read(count_2, int_);",
                "b.read(n_jobs, int_);",
                "b.read(http_server, int_);",
                "b.read(std_2, int_);",
                "b.read(main_2, int_);",
                "b.read(b_2, int_);",
            ],
        ),
        (
            "type count = 1..2\ny: count\n",
            &[
                "auto count_2 = b.define_type(\"count\", Range{.from = 1, .to = 2});",
                "b.read(y, count_2);",
            ],
        ),
    ]);
}

#[test]
fn prefixes_of_numbered_helpers_take_a_letter_before_a_number() {
    for prefix in NUMBERED_PREFIXES {
        for source in prefix_collisions(prefix) {
            code_in_time(&source);
        }
    }
    assert_statements(&[(
        "_block: int\nblock: int\n_body: int\nbody: int\n",
        &[
            "b.read(block, int_);",
            "b.read(block_v, int_);",
            "b.read(body, int_);",
            "b.read(body_v, int_);",
        ],
    )]);
}

#[test]
fn strings_are_ascii_and_long_ones_are_constants() {
    let code = code(
        "x: word\ncheck x == \"a\\tb\\\"c\\\\\u{e9}\"\nlet y = \"a very long string that \
         does not fit on one line of eighty columns at all, not even close\"\n",
    );
    for expected in [
        "#include <string_view>\n",
        "constexpr std::string_view t1 =\n    \
         \"a very long string that does not fit on one line of eighty columns at all,\"\n    \
         \" not even close\";\n",
        "  b.check(x == \"a\\tb\\\"c\\\\\\303\\251\");\n",
        "  b.let(y, t1);\n",
    ] {
        assert!(code.contains(expected), "{expected:?} is not in:\n{code}");
    }
    assert!(code.is_ascii(), "{code}");
}

#[test]
fn cpp_has_builders_but_no_view_printer() {
    assert!(!Language::Cpp.has_view());
    assert_eq!(Language::Cpp.name(), "cpp");
    assert_eq!("cpp".parse::<Language>(), Ok(Language::Cpp));
    let parsed = parse(b"x: int\n", "test.iris", LanguageVersion::V0_1).unwrap();
    let analysis = analyze(&parsed);
    assert!(matches!(
        view_code(&analysis, Language::Cpp),
        Err(ViewError::NoPrinter(Language::Cpp))
    ));
}
