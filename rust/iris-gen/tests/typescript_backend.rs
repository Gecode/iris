//! The TypeScript text of each production, each identifier rule and each
//! layout rule that the backend takes from oxfmt, oxlint and the types of
//! `gecode-iris/build`.
use gecode_iris_gen::{Language, builder_code, view_code};
use gecode_iris_sema::analyze;
use gecode_iris_syntax::{LanguageVersion, parse};

fn code(source: &str) -> String {
    let parsed = parse(source.as_bytes(), "test.iris", LanguageVersion::V0_1).unwrap();
    let analysis = analyze(&parsed);
    builder_code(&parsed, &analysis, Language::TypeScript).expect("the spec is accepted")
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
fn the_program_exports_a_builder_and_prints_its_source() {
    assert_eq!(
        code("format dimacs\nskip line\n"),
        "import { SpecBuilder } from \"gecode-iris/build\";\n\n\
         export function builder(): SpecBuilder {\n  \
         const b = new SpecBuilder(\"dimacs\");\n  \
         b.skipLine();\n  \
         return b;\n\
         }\n\n\
         if (import.meta.main) {\n  \
         process.stdout.write(builder().source());\n\
         }\n"
    );
    assert_statements(&[
        ("", &["const b = new SpecBuilder();", "return b;"]),
        (
            "format \"two words\"\nskip line\n",
            &["const b = new SpecBuilder(\"two words\");"],
        ),
        (
            "skip line\nformat late\n",
            &["b.skipLine();", "b.format(\"late\");"],
        ),
    ]);
}

#[test]
fn handles_come_before_the_statements() {
    assert_statements(&[
        (
            "n: int\nrepeat n as rs {\n  each r\n  k: int\n}\ncheck !defined(q) && r.k > 0\n",
            &[
                "const b = new SpecBuilder();",
                "const n = b.forward(\"n\");",
                "const k = b.forward(\"k\");",
                "const q = b.undefined(\"q\");",
                "const r = b.forwardRecord(\"r\", \"rs\");",
                "b.read(n, int);",
                "b.repeat(n, (s1) => {",
                "  s1.recordAs(r);",
                "  s1.read(k, int);",
                "});",
            ],
        ),
        // A repeat whose record no check outside it uses names it in place.
        (
            "repeat 2 as items {\n  each item\n  y: int\n}\nrepeat 1 {\n  each _\n}\n",
            &[
                "b.repeat(2, (s1) => {",
                "  s1.eachName(\"item\");",
                "  s1.asName(\"items\");",
                "  s1.read(y, int);",
                "});",
                "b.repeat(1, (s1) => {",
                "  s1.eachName(\"_\");",
                "});",
            ],
        ),
    ]);
}

#[test]
fn settings() {
    assert_statements(&[(
        "comment \"c\" \"#\"\nstop \"end\"\ntokens \"+-\"\nseparators \",\"\nignore trailing\n",
        &[
            "b.comment(\"c\", \"#\");",
            "b.stop(\"end\");",
            "b.tokens(\"+-\");",
            "b.separators(\",\");",
            "b.ignoreTrailing();",
        ],
    )]);
}

#[test]
fn declarations_and_types() {
    assert_statements(&[(
        "n: int\nx[n][2]: 0..<n\ny[2][2]: 1..9 layout \"UPPER_ROW\" offset 2\n\
         list z: word until \"end\"\nlist w: float until eol offset 1\n_: num\n\
         type d = 1..9\ntype e = d\nv: e\nu: ..5\nt: ..<5\nr: 1..<5\n\
         type g = alphabet \"01\" blank \".\" base 2\ns[2][2]: g\nq: rest\n",
        &[
            "b.read(n, int);",
            "b.read(x, range(0).below(n).dim(n).dim(2));",
            "b.read(y, range(1, 9).dim(2).dim(2).layout(\"UPPER_ROW\").offset(2));",
            "b.list(z, word.until(\"end\"));",
            "b.list(w, float.until(eol).offset(1));",
            "b.read(discard(), num);",
            "const d = b.defineType(\"d\", range(1, 9));",
            "const e = b.defineType(\"e\", d);",
            "b.read(v, e);",
            "b.read(u, atMost(5));",
            "b.read(t, below(5));",
            "b.read(r, range(1).below(5));",
            "const g = b.defineType(\"g\", alphabet(\"01\").blank(\".\").base(2));",
            "b.read(s, g.dim(2).dim(2));",
            "b.read(q, rest);",
        ],
    )]);
    // A type that nothing uses needs no constant.
    assert_statements(&[("type d = 1..9\n", &["b.defineType(\"d\", range(1, 9));"])]);
}

#[test]
fn values_and_checks() {
    assert_statements(&[(
        "n: int\nlet m = n + 1\nparam k\nparam j = 3\ncheck m > 0 \"m is positive\"\n\
         check !defined(q)\nexpect \"a\" \"b\"\nskip n lines\nindex n\nlet s = \"text\"\n",
        &[
            "b.read(n, int);",
            "b.let(m, n.plus(1));",
            "b.param(k);",
            "b.param(j, 3);",
            "b.check(m.gt(0), \"m is positive\");",
            "b.check(defined(q).not());",
            "b.expect(\"a\", \"b\");",
            "b.skipLines(n);",
            "b.index(n);",
            "b.let(s, \"text\");",
        ],
    )]);
}

#[test]
fn lines_and_patterns() {
    assert_statements(&[(
        "line \"n = {n} {$n} {:int} {} {_:int} {rest}\"\nline \"plain\"\n\
         find \"x {m:int}\"\nline \"\"\n",
        &[
            "b.line(p`n = ${bind(n)} ${ref(n)} ${validate(int)} ${blank()} ${discardAs(int)} ${bind(rest2)}`);",
            "b.line(\"plain\");",
            "b.find(p`x ${bind(m, int)}`);",
            "b.line(\"\");",
        ],
    )]);
    // `rest` names the rest of the line only as a type.
    assert_statements(&[(
        "line \"{x:word} {:rest}\"\n",
        &["b.line(p`${bind(x, word)} ${validate(rest)}`);"],
    )]);
}

#[test]
fn blocks_are_arrow_functions_numbered_by_depth() {
    assert_statements(&[
        (
            "n: int\nrepeat n {\n  repeat 2 {\n    x: int\n  }\n}\n",
            &[
                "b.repeat(n, (s1) => {",
                "  s1.repeat(2, (s2) => {",
                "    s2.read(x, int);",
                "  });",
                "});",
            ],
        ),
        // An empty block leaves out its arrow function where the builder
        // allows it.
        (
            "repeat until eof {}\nrepeat until \"end\" \"stop\" store s {}\nsections {}\n",
            &[
                "b.repeatUntilEof();",
                "b.repeatUntil([\"end\", \"stop\"], { store: s });",
                "b.sections();",
            ],
        ),
        (
            "instances {\n  n: int\n}\n",
            &["b.instances((s1) => {", "  s1.read(n, int);", "});"],
        ),
        // `if` needs one.
        ("n: int\nif n > 0 {}\n", &["b.if(n.gt(0), () => {});"]),
    ]);
}

#[test]
fn sections_take_their_clauses_and_arms_their_options() {
    assert_statements(&[
        (
            "sections until \"X\" store a skip other {\n  \"A\" optional\n  \
             \"B\" last { x: int }\n  \"C\" optional {}\n}\n",
            &[
                "b.sections({ until: \"X\", store: a, skipOther: true }, (a1) => {",
                "  a1.arm(\"A\", { optional: true });",
                "  a1.arm(\"B\", { last: true }, (s2) => {",
                "    s2.read(x, int);",
                "  });",
                "  a1.arm(\"C\", { optional: true }, () => {});",
                "});",
            ],
        ),
        // Clauses in another order are a list.
        (
            "sections skip other until \"Y\" \"Z\" {\n  \"\"\n}\n",
            &[
                "b.sections([{ skipOther: true }, { until: [\"Y\", \"Z\"] }], (a1) => {",
                "  a1.arm(\"\");",
                "});",
            ],
        ),
        (
            "sections skip other {}\n",
            &["b.sections({ skipOther: true });"],
        ),
    ]);
}

#[test]
fn if_chains_are_one_statement() {
    assert_statements(&[
        (
            "n: int\nif n > 1 { b: int } else { c: int }\n",
            &[
                "b.if(n.gt(1), (s1) => {",
                "  s1.read(b2, int);",
                "}).else((s1) => {",
                "  s1.read(c, int);",
                "});",
            ],
        ),
        (
            "n: int\nif n > 2 {} else {}\n",
            &["b.if(n.gt(2), () => {}).else(() => {});"],
        ),
        // A chain of more than two calls puts each after the first on a line
        // of its own.
        (
            "n: int\nif n > 3 { d: int } else if n > 4 { e: int } else { f: int }\n",
            &[
                "b.if(n.gt(3), (s1) => {",
                "  s1.read(d, int);",
                "})",
                "  .elseIf(n.gt(4), (s1) => {",
                "    s1.read(e, int);",
                "  })",
                "  .else((s1) => {",
                "    s1.read(f, int);",
                "  });",
            ],
        ),
        (
            "n: int\nif n > 5 {} else if n > 6 {} else if: int\n",
            &[
                "b.if(n.gt(5), () => {})",
                "  .elseIf(n.gt(6), () => {})",
                "  .elseReadIf(if2, int);",
            ],
        ),
    ]);
}

#[test]
fn deep_blocks_become_functions_with_what_they_use() {
    let source = format!(
        "x: int\n{}type t = 1..3\n{}y: t\nlet z = defined(zz)\n{}",
        repeated("repeat 1 {\n", 2),
        repeated("repeat 1 {\n", 8),
        repeated("}\n", 10)
    );
    assert_statements(&[
        (
            source.as_str(),
            &[
                "function block1(",
                "  s1: NestedScope<HandleDepth, Presence>,",
                "  y: DynField,",
                "  t: Type<\"int\", 0>,",
                "  z: DynField,",
                "  zz: DynField,",
                "): void {",
                "  s1.read(y, t);",
                "  s1.let(z, defined(zz));",
                "}",
            ],
        ),
        (
            source.as_str(),
            &[
                "s9.repeat(1, (s10) => {",
                "  block1(s10, y, t, z, zz);",
                "});",
            ],
        ),
        (
            source.as_str(),
            &["const t = s2.defineType(\"t\", range(1, 3));"],
        ),
    ]);
    // Each function between the definition and the use takes the type.
    let twice = format!(
        "type t = 1..3\n{}y: t\n{}",
        repeated("repeat 1 {\n", 19),
        repeated("}\n", 19)
    );
    assert_statements(&[
        (
            twice.as_str(),
            &[
                "function block1(s1: NestedScope<HandleDepth, Presence>, y: DynField, t: Type<\"int\", 0>): void {",
                "  s1.repeat(1, (s2) => {",
            ],
        ),
        (twice.as_str(), &["block2(s10, y, t);"]),
        (
            twice.as_str(),
            &[
                "function block2(s1: NestedScope<HandleDepth, Presence>, y: DynField, t: Type<\"int\", 0>): void {",
                "  s1.read(y, t);",
                "}",
            ],
        ),
    ]);
    // An arm list and a branch take the classes of their scopes.
    let arms = format!(
        "{}sections {{\n\"a\" {{\nif 1 {{\ny: int\n}}\n}}\n}}\n{}",
        repeated("repeat 1 {\n", 9),
        repeated("}\n", 9)
    );
    assert_statements(&[
        (
            arms.as_str(),
            &["s9.sections((a10) => {", "  block1(a10, y);", "});"],
        ),
        (
            arms.as_str(),
            &["function block1(a1: Arms<HandleDepth, Presence>, y: DynField): void {"],
        ),
        (
            arms.as_str(),
            &[
                "a1.arm(\"a\", (s2) => {",
                "  s2.if(1, (s3) => {",
                "    s3.read(y, int);",
                "  });",
                "});",
            ],
        ),
    ]);
}

#[test]
fn branches_far_right_become_functions() {
    // Each link after the first of a chain of three is two columns further
    // in, so the statements of nine nested ones start far right with only
    // nine arrow functions around them.
    let source = format!(
        "n: int\n{}x: int\n{}",
        repeated("if n > 0 {} else if n < 0 {} else {\n", 9),
        repeated("}\n", 9)
    );
    assert_statements(&[
        (
            source.as_str(),
            &[
                "s8.if(n.gt(0), () => {})",
                "  .elseIf(n.lt(0), () => {})",
                "  .else((s9) => {",
                "    block1(s9, x);",
                "  });",
            ],
        ),
        (
            source.as_str(),
            &[
                "function block1(s1: Scope<HandleDepth, Presence>, x: DynField): void {",
                "  s1.read(x, int);",
                "}",
            ],
        ),
    ]);
}

#[test]
fn operators_are_methods_and_functions() {
    assert_statements(&[(
        "n: int\nlist a: int until eol\nlet x = -n + 2 * (n - 1) / 3 % 4 ^ 2\n\
         let y = !(n == 1) || n != 2 && n < 3\nlet z = \"s\" == n\nlet w = 1.5 < n\n\
         let v = a[0] + a[n]\nlet t = 9007199254740993\nlet s = 1e300\nlet r = -9007199254740991\n",
        &[
            "b.let(x, n.neg().plus(rem(div(lit(2).times(n.minus(1)), 3), pow(4, 2))));",
            // oxfmt breaks a chain of more than two calls with a call among
            // their arguments, so one operand goes into a constant.
            "const e1 = n.ne(2).and(n.lt(3));",
            "b.let(y, n.eq(1).not().or(e1));",
            "b.let(z, lit(\"s\").eq(n));",
            "b.let(w, floatLit(1.5).lt(n));",
            "b.let(v, a.at(0).plus(a.at(n)));",
            // A number that is not a safe integer is a bigint.
            "b.let(t, 9007199254740993n);",
            "b.let(s, floatLit(1e300));",
            "b.let(r, lit(9007199254740991).neg());",
        ],
    )]);
    // An operand that a method refuses goes through `dynamic()`.
    assert_statements(&[(
        "x: word\nlet a = x[0]\nlet b = -\"s\"\nlet c = \"s\" < 1\nlet d = (1 < 2)[0]\n",
        &[
            "b.let(a, x.at(0));",
            "b.let(b2, lit(\"s\").dynamic().neg());",
            "b.let(c, lit(\"s\").dynamic().lt(1));",
            "b.let(d, lit(1).lt(2).dynamic().at(0));",
        ],
    )]);
}

#[test]
fn host_constants_are_int_expressions() {
    assert_statements(&[(
        "let u = gecode.int_max - gecode.int_min\nlet v = -gecode.int_max\n",
        &[
            "b.let(u, gecode.intMax.minus(gecode.intMin));",
            "b.let(v, gecode.intMax.neg());",
        ],
    )]);
}

#[test]
fn functions_records_and_comprehensions() {
    assert_statements(&[(
        "list a: int until eol\nrepeat 2 as rs {\n each r\n k: int\n}\n\
         let m = min(a[0], 1) + max(1, 2) + sum(a) + size(a)\n\
         let c = all(x in a: x > 0) && any(o in rs: o.k > 1)\n\
         let s = sum(x in a: count(y in a: y == x))\n\
         let q = defined(rs) && count(rs) > 0\ncheck distinct(r.k) && rs[0].k > 0\n",
        &[
            "b.let(m, min(a.at(0), 1).plus(max(1, 2)).plus(sum(a)).plus(size(a)));",
            "const e1 = all(over(a, \"x\"), (x) => x.gt(0));",
            "const e2 = any(over(r.list, \"o\"), (o) => o.get(k).gt(1));",
            "b.let(c, e1.and(e2));",
            "const e4 = sum(over(a, \"x\"), (x) => {",
            "  const e3 = count(over(a, \"y\"), (y) => y.eq(x));",
            "  return e3;",
            "});",
            "b.let(s, e4);",
            "b.let(q, defined(r.list).and(count(r.list).gt(0)));",
            "b.check(distinct(r.get(k)).and(r.list.at(0).get(k).gt(0)));",
        ],
    )]);
    // A variable that the body leaves out is no parameter.
    assert_statements(&[(
        "list a: int until eol\nlet n = count(x in a: 1)\n",
        &["const e1 = count(over(a, \"x\"), () => 1);"],
    )]);
}

#[test]
fn deep_comprehension_bodies_become_functions() {
    let variables: Vec<String> = (0..18).map(|index| format!("v{index}")).collect();
    let mut source = String::from("list a: int until eol\nlet s = ");
    for variable in &variables {
        source.push_str(&format!("sum({variable} in a: "));
    }
    source.push_str(&variables.join(" + "));
    source.push_str(&")".repeat(variables.len()));
    source.push('\n');
    let mut parameters = vec!["function body1(".to_owned()];
    parameters.extend(
        variables
            .iter()
            .rev()
            .take(1)
            .chain(&variables[..17])
            .map(|variable| format!("  {variable}: Lookup<\"dyn\">,")),
    );
    parameters.push("): Expr {".to_owned());
    let parameters: Vec<&str> = parameters.iter().map(String::as_str).collect();
    let mut call = vec![
        "const e2 = sum(e3, (v17) => {".to_owned(),
        "  return body1(".to_owned(),
    ];
    call.extend(
        variables
            .iter()
            .rev()
            .take(1)
            .chain(&variables[..17])
            .map(|variable| format!("    {variable},")),
    );
    call.extend(["  );".to_owned(), "});".to_owned()]);
    let call: Vec<&str> = call.iter().map(String::as_str).collect();
    assert_statements(&[
        (source.as_str(), &parameters),
        (source.as_str(), &call),
        (source.as_str(), &["const e3 = over(a, \"v17\");"]),
    ]);
    // A body far right that fits is a call on one line; one that uses
    // handles takes them.
    let deep = format!(
        "list a: int until eol\nn: int\n{}let s = sum(x in a: x + n)\n{}",
        repeated("repeat 1 {\n", 18),
        repeated("}\n", 18)
    );
    assert_statements(&[(
        deep.as_str(),
        &[
            "const e1 = sum(over(a, \"x\"), (x) => x.plus(n));",
            "s9.let(s, e1);",
        ],
    )]);
}

#[test]
fn layouts_are_those_of_oxfmt() {
    let long = "n".repeat(24);
    let m70 = "m".repeat(70);
    let m120 = "m".repeat(120);
    let fill = "\"abcdefg\" ".repeat(12);
    assert_statements(&[
        // One argument on each line, with a trailing comma.
        (
            &format!("{long}: int\nx: int\ncheck x > {long} \"{m70}\"\n"),
            &[
                "b.check(",
                &format!("  x.gt({long}),"),
                &format!("  \"{m70}\","),
                ");",
            ],
        ),
        // A string too wide for any line goes into a module constant, which
        // keeps it whole.
        (
            &format!("x: int\ncheck x > 0 \"{m120}\"\n"),
            &["const t1 =", &format!("  \"{m120}\";")],
        ),
        (
            &format!("x: int\ncheck x > 0 \"{m120}\"\n"),
            &["b.check(x.gt(0), t1);"],
        ),
        // Short arguments too, never filled.
        (
            &format!("expect {fill}\n"),
            &[
                "b.expect(",
                "  \"abcdefg\",",
                "  \"abcdefg\",",
                "  \"abcdefg\",",
                "  \"abcdefg\",",
                "  \"abcdefg\",",
                "  \"abcdefg\",",
                "  \"abcdefg\",",
                "  \"abcdefg\",",
                "  \"abcdefg\",",
                "  \"abcdefg\",",
                "  \"abcdefg\",",
                "  \"abcdefg\",",
                ");",
            ],
        ),
        // A block's opener stays on one line: its widest argument goes into
        // a constant.
        (
            &format!(
                "sections {{\n  \"{{{long}:int}} {{{long}:int}} {{{long}:int}}\" optional {{\n    \
                 x: int\n  }}\n}}\n"
            ),
            &[
                &format!(
                    "const e1 = p`${{bind({long}, int)}} ${{bind({long}, int)}} ${{bind({long}, int)}}`;"
                ),
                "a1.arm(e1, { optional: true }, (s2) => {",
                "  s2.read(x, int);",
                "});",
            ],
        ),
    ]);
    // A constant puts one argument of a call on each line.
    let [a, b, c] = ["a", "b", "c"].map(|letter| letter.repeat(20));
    assert_statements(&[(
        &format!(
            "x: int\n{a}: int\n{b}: int\n{c}: int\nlet z = min({a}, {b}, {c}, {a}, {b}, x) + 1\n"
        ),
        &[
            "const e1 = min(",
            &format!("  {a},"),
            &format!("  {b},"),
            &format!("  {c},"),
            &format!("  {a},"),
            &format!("  {b},"),
            "  x,",
            ");",
            "b.let(z, e1.plus(1));",
        ],
    )]);
    // An object or an array in a constant keeps its literal types, and
    // one too wide puts each element on a line of its own.
    let terminator = "t".repeat(14);
    assert_statements(&[(
        &format!(
            "sections skip other until \"{terminator}\" \"{terminator}\" store {long} {{\n  \
             \"a\"\n}}\n"
        ),
        &[
            "const e1 = [",
            "  { skipOther: true },",
            &format!("  {{ until: [\"{terminator}\", \"{terminator}\"], store: {long} }},"),
            "] as const;",
            "b.sections(e1, (a1) => {",
        ],
    )]);
    // An element too wide for its own line puts its widest strings into
    // constants.
    let [t, u] = ["t", "u"].map(|letter| letter.repeat(60));
    assert_statements(&[(
        &format!("sections skip other until \"{t}\" \"{u}\" store n {{\n  \"a\"\n}}\n"),
        &["b.sections([{ skipOther: true }, { until: [t2, t1], store: n }], (a1) => {"],
    )]);
}

#[test]
fn identifiers_keep_their_spelling_and_are_never_reserved() {
    assert_statements(&[(
        "if: int\nin: int\nb: int\ne1: int\ns1: int\nt1: int\nint: int\nsum: int\n\
         undefined: int\nglobalThis: int\n_name: int\nname: int\nn_jobs: int\nN_MAX: int\n",
        &[
            "const if2 = b.forward(\"if\");",
            "const in2 = b.forward(\"in\");",
            "const b2 = b.forward(\"b\");",
            "const e1v = b.forward(\"e1\");",
            "const s1v = b.forward(\"s1\");",
            "const t1v = b.forward(\"t1\");",
            "const int2 = b.forward(\"int\");",
            "const sum2 = b.forward(\"sum\");",
            "const undefined2 = b.forward(\"undefined\");",
            "const globalThis2 = b.forward(\"globalThis\");",
            // oxlint refuses an underscore at either end.
            "const name = b.forward(\"_name\");",
            "const name2 = b.forward(\"name\");",
            "const n_jobs = b.forward(\"n_jobs\");",
            "const N_MAX = b.forward(\"N_MAX\");",
        ],
    )]);
    // Values, types and variables never share an identifier, which
    // `no-shadow` would refuse.
    assert_statements(&[(
        "type x = 1..2\ny: x\nlist l: int until eol\nlet z = sum(x in l: x)\n\
         let w = sum(e1 in l: e1 + 1) + sum(b in l: b)\n",
        &[
            "const x2 = b.defineType(\"x\", range(1, 2));",
            "b.read(y, x2);",
            "b.list(l, int.until(eol));",
            "const e1 = sum(over(l, \"x\"), (x) => x);",
            "b.let(z, e1);",
            "const e2 = sum(over(l, \"e1\"), (e1v) => e1v.plus(1));",
            "const e3 = sum(over(l, \"b\"), (b2) => b2);",
            "b.let(w, e2.plus(e3));",
        ],
    )]);
}

#[test]
fn bare_prefixes_of_numbered_names_take_a_letter_before_a_number() {
    for prefix in NUMBERED_PREFIXES {
        for source in prefix_collisions(prefix) {
            code_in_time(&source);
        }
    }
    // The program's own prefixes: `s1`, `a1`, `e1`, `t1`, `block1` and
    // `body1`.
    for prefix in ["s", "a", "e", "t", "block", "body"] {
        assert_statements(&[(
            &format!("_{prefix}: int\n{prefix}: int\n"),
            &[
                &format!("const {prefix} = b.forward(\"_{prefix}\");"),
                &format!("const {prefix}v = b.forward(\"{prefix}\");"),
            ],
        )]);
    }
}

#[test]
fn floats_near_math_constants_disable_the_rule_that_refuses_them() {
    const DISABLE: &str = "// oxlint-disable oxc/approx-constant\n";
    // oxlint's `oxc/approx-constant` knows the constants of `Math`, and
    // refuses a literal of five characters or more that starts one or
    // rounds one.
    for (source, near) in [
        ("let x = 3.141592653589793\n", true),
        ("let x = 2.718281828459045\n", true),
        ("let x = -3.142\n", true),
        ("let x = 0.6931\n", true),
        ("let x = 1.4142\n", true),
        ("n: int\nrepeat n {\n  let x = 2.7183\n}\n", true),
        ("let x = 3.14\n", false),
        ("let x = 2.72\n", false),
        ("let x = 3.15\n", false),
        ("let x = 1.5707963\n", false),
        ("let x = 2.5\n", false),
    ] {
        let code = code(source);
        assert_eq!(code.starts_with(DISABLE), near, "{source:?}:\n{code}");
        assert_eq!(code.matches("oxlint-disable").count(), usize::from(near));
    }
    // The literal keeps its exact value.
    assert_statements(&[(
        "let x = 3.141592653589793\n",
        &["b.let(x, floatLit(3.141592653589793));"],
    )]);
}

#[test]
fn strings_are_ascii_with_the_quotes_oxfmt_chooses() {
    assert_statements(&[(
        "expect \"it's\" \"say \\\"hi\\\"\" \"é\\t\" \"`${x}`\"\nline \"`${x}` {y}\"\n",
        &[
            "b.expect(\"it's\", 'say \"hi\"', \"\\u00e9\\t\", \"`${x}`\");",
            // `$` before a capture is template text.
            "b.line(p`\\`$${bind(x)}\\` ${bind(y)}`);",
        ],
    )]);
}

#[test]
fn typescript_has_builders_and_a_view_printer() {
    assert!(Language::TypeScript.has_view());
    assert_eq!(Language::TypeScript.name(), "typescript");
    assert_eq!("typescript".parse::<Language>(), Ok(Language::TypeScript));
    let parsed = parse(b"x: int\n", "test.iris", LanguageVersion::V0_1).unwrap();
    let analysis = analyze(&parsed);
    assert!(view_code(&parsed, &analysis, Language::TypeScript).is_ok());
}

/// oxlint refuses an unused import, so the program imports what it names
/// and the types of its functions' signatures.
#[test]
fn the_program_imports_exactly_what_it_names() {
    let flat = code(
        "n: int\nx: 0..<n\nz: 1..n\nlet y = -n % 2\ncheck n <= 1\nlist l: int until eol\n\
         let s = sum(it in l: it)\n",
    );
    assert!(
        flat.starts_with(
            "import { SpecBuilder, eol, int, over, range, rem, sum } from \"gecode-iris/build\";\n\n"
        ),
        "{flat}"
    );
    let deep = format!(
        "type t = 1..3\nlist l: int until eol\n{}y: t\nlet s = sum(v in l: v)\n{}",
        repeated("repeat 1 {\n", 10),
        repeated("}\n", 10)
    );
    let nested = code(&deep);
    let imports: Vec<&str> = nested.lines().take_while(|line| !line.is_empty()).collect();
    assert_eq!(
        imports,
        [
            "import {",
            "  SpecBuilder,",
            "  eol,",
            "  int,",
            "  over,",
            "  range,",
            "  sum,",
            "  type DynField,",
            "  type HandleDepth,",
            "  type NestedScope,",
            "  type Presence,",
            "  type Type,",
            "} from \"gecode-iris/build\";",
        ]
    );
}
