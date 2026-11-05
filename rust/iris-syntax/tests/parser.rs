use gecode_iris_syntax::{
    DiagnosticCode as C, DiagnosticPhase, LanguageVersion, Parse, ParseError, SyntaxKind as K,
    SyntaxNode, parse,
};

const FIXTURES: &[(&str, &str)] = &[
    ("sat", include_str!("fixtures/gecode/sat.iris")),
    ("tsp", include_str!("fixtures/gecode/tsp.iris")),
    ("tsptw", include_str!("fixtures/gecode/tsptw.iris")),
    ("job-shop", include_str!("fixtures/gecode/job-shop.iris")),
    ("nonogram", include_str!("fixtures/gecode/nonogram.iris")),
    ("sudoku", include_str!("fixtures/gecode/sudoku.iris")),
];

fn checked(source: &str) -> Parse {
    let result = parse(source.as_bytes(), "spec.iris", LanguageVersion::V0_1).unwrap();
    let root = result.syntax();
    assert_eq!(root.text().to_string().as_bytes(), source.as_bytes());
    let mut offset = 0;
    for token in root
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
    {
        assert_eq!(usize::from(token.text_range().start()), offset);
        offset += token.text().len();
        assert_eq!(usize::from(token.text_range().end()), offset);
    }
    assert_eq!(offset, source.len());
    result
}
fn accepted(source: &str) -> Parse {
    let result = checked(source);
    assert!(
        result.diagnostics().is_empty(),
        "{source:?}: {:?}",
        result.diagnostics()
    );
    result
}
fn rejected(source: &str, code: C, line: usize) -> Parse {
    let result = checked(source);
    let first = result.diagnostics().first().expect("expected a diagnostic");
    assert_eq!(
        (first.code, first.location.line, first.location.column),
        (code, line, None),
        "{source:?}: {:?}",
        result.diagnostics()
    );
    assert_eq!(&*first.location.origin, "spec.iris");
    result
}
fn nodes(root: &SyntaxNode, kind: K) -> Vec<SyntaxNode> {
    root.descendants()
        .filter(|node| node.kind() == kind)
        .collect()
}

#[test]
fn six_gecode_examples() {
    for (name, source) in FIXTURES {
        let result = accepted(source);
        assert!(
            !nodes(
                &result.syntax(),
                if *name == "tsptw" {
                    K::TypeName
                } else {
                    K::Capture
                }
            )
            .is_empty(),
            "{name}"
        );
        assert_eq!(result.version(), LanguageVersion::V0_1);
        assert_eq!(result.origin(), "spec.iris");
    }
}

#[test]
fn source_and_lexical_edges() {
    for source in [
        "",
        " \t\r\u{b}\u{c}\n# é\rstill a comment",
        "let s=\"é\u{feff}\u{ffff}\r\0\" # \u{feff}",
        "\t\r\u{b}\u{c}let s=\"a\r\0b\"# comment\rstill comment\nn:int",
        r#"let s="a\nb\tc\\d\"e""#,
        "let x=1e2foo:int",
        "let a=007 let b=1. let c=1.e2 let d=01e+2 let z=0e-9999",
        "let x=-9223372036854775807-1",
        "let x=2.2250738585072014e-308",
        "let x=2.2250738585072013e-308",
    ] {
        accepted(source);
    }
    for source in [
        "let x=1.2.3",
        "let x=1e",
        "let x=1e+",
        "let x=1ee2",
        "let x=1e9999",
        "let x=1e-9999",
        "let x=2.225073858507201e-308",
        "let x=-9223372036854775808",
    ] {
        rejected(source, C::Number, 1);
    }
    for source in ["let x=@", "let x=1;", "\u{feff}x:int", "é:int", "let x=&1"] {
        rejected(source, C::Character, 1);
    }
    for source in [
        r#"let x="\r""#,
        "let s=\"abc",
        "let s=\"abc\ny:int",
        "let s=\"abc\\\ny:int",
    ] {
        rejected(source, C::String, 1);
    }
    for source in ["let x=+1", "let x=.5", "let x=1//2", "\0"] {
        rejected(source, C::Syntax, 1);
    }
}

#[test]
fn utf8_validation_precedes_everything() {
    let cases: &[(&[u8], usize, usize)] = &[
        (b"let s=\"\xff\"", 1, 8),
        (b"x:int\n#  \xff", 2, 4),
        (b"@\n# \xc3\xa9\t\xe2\x82", 2, 6),
        (b"\x80", 1, 1),
        (b"\xc0\xaf", 1, 1),
        (b"\xed\xa0\x80", 1, 1),
        (b"\xf4\x90\x80\x80", 1, 1),
        (b"\xe2A", 1, 1),
        (b"#\xf0\x9f", 1, 2),
    ];
    for &(bytes, line, column) in cases {
        let ParseError::InvalidUtf8(error) =
            parse(bytes, "spec.iris", LanguageVersion::V0_1).unwrap_err()
        else {
            panic!("expected UTF-8 failure")
        };
        assert_eq!(error.code, C::Utf8);
        assert_eq!(
            (error.location.line, error.location.column),
            (line, Some(column))
        );
        assert_eq!(error.phase, DiagnosticPhase::Source);
        assert!(
            error
                .to_string()
                .starts_with(&format!("spec.iris:{line}:{column}:"))
        );
    }
}

#[test]
fn malformed_inputs_have_expected_first_codes_and_positions() {
    let cases = [
        ("n:int\nlet x=@", C::Character, 2),
        ("n:int\nlet s=\"\\r\"", C::String, 2),
        ("n:int\nlet s=\"abc\ny:int", C::String, 2),
        ("n:int\nlet x=1.2.3", C::Number, 2),
        ("n:int\nlet x=-9223372036854775808", C::Number, 2),
        ("n:int\nbogus", C::Syntax, 2),
        ("repeat 2 {\nx:int\n", C::Syntax, 3),
        ("n:int\nx:0..<\n", C::Syntax, 3),
        ("n:int\nx:..\n", C::Syntax, 3),
        ("if 0 {\ncomment \"#\"}", C::Placement, 2),
        ("n:int\neach x", C::Placement, 2),
        ("let _\n=1", C::Name, 2),
        ("n:int\nline \"{x:int\"", C::Pattern, 2),
        ("n:int\nline \"...x\"", C::Pattern, 2),
        ("n:int\nline \"{x:}\"", C::Pattern, 2),
        ("n:int\nline \"{x:int int}\"", C::Pattern, 2),
        ("bogus\nlet s=\"\\r\"", C::String, 2),
        ("n:int\nformat:int", C::Syntax, 2),
        ("n:int\nline \"{$}\"", C::Pattern, 2),
        ("line \"{x:\\n@}\"", C::Character, 2),
    ];
    for (source, code, line) in cases {
        rejected(source, code, line);
    }
}

#[test]
fn declarations_win_over_contextual_keywords() {
    let source =
        "line:int type:0.. list:word each:int layout:int offset:int blank:int strip:int base:int";
    let result = accepted(source);
    assert_eq!(nodes(&result.syntax(), K::Declaration).len(), 9);
    accepted("repeat 1 { format:int comment:int expect[1]:int if[1]:int }");
    accepted("if 0 {} else if:int");
    accepted("if 0 {} else if[1]:int");
    accepted("a[size(\"[\")]:int b[1][2]:float");
    rejected("repeat[1+]:int after:int", C::Syntax, 1);
    rejected("repeat 1 {format \"nested\"}", C::Syntax, 1);
    let result = accepted("let x=1\n+2 y:int repeat n {node:int x:float y:float}");
    assert_eq!(nodes(&result.syntax(), K::Declaration).len(), 4);
    accepted("let x=size\n(a) let y=a\n[0]\n.f");
}

#[test]
fn statement_and_type_forms() {
    for source in [
        "a:int b:float c:num d:word e:rest f[2][3]:0..10 layout \"FULL_MATRIX\" offset -1",
        "a:..9 b:..<9 c:1.. d:1..<9 e:(check)..(offset)",
        "a[2][2]:alphabet \"ab\" blank \".\" strip \"_\" base -2 base 1 blank \"-\" blank:int",
        "type t=0..10 type s=t a:t if 1 {type inner=s b:inner} c:s",
        "list a:0.. until 0 offset 1 list w:word until eol",
        "expect \"a\" \"b\" param p param q=2 line \"{x}\" find \"...\" skip line skip 3 lines index 0",
        "repeat 2 as xs {# first\neach x a:int} repeat until eof {} repeat until \"END\" \"STOP\" store end as rows {}",
        "sections until \"A\" store a until \"B\" store b skip other skip other {\"x\" optional last {} \"y\" \"z\" {a:int}}",
        "if 1 {} else if 2 {} else {} let x=1 check x \"message\"",
        "instances {} format a format \"\"",
        "comment \"#\" stop \"X\" tokens \"!\" separators \",\" ignore trailing comment \"%\"",
    ] {
        accepted(source);
    }
    // These need name resolution, reading-type checks or record-scope analysis.
    for source in [
        "x:unknown",
        "x:(unknown)",
        "a[1]:rest bogus:int",
        "type a=alphabet \"ab\" line \"{x:a}\"",
        "type t=int let t=1",
        "repeat 1 {each x a:int} let y=x",
        "list x:rest until 0",
    ] {
        accepted(source);
    }
    rejected("a[1]:rest bogus", C::Syntax, 1);
    // A visible type wins dispatch over range expressions; sibling types do not leak.
    rejected("type t=int x:t..2", C::Syntax, 1);
    accepted("if 1 {type t=int a:t} if 1 {b:t..2}");
    accepted("type t=t..2 a:t"); // A definition is visible only after its RHS.
}

#[test]
fn range_upper_bound_uses_the_exact_stop_set() {
    let statements = [
        "comment \"#\"",
        "stop \"X\"",
        "expect \"X\"",
        "tokens \"!\"",
        "separators \",\"",
        "ignore trailing",
        "param p",
        "list a:int until 0",
        "line \"X\"",
        "find \"X\"",
        "skip line",
        "index 0",
        "repeat 0 {}",
        "instances {}",
        "sections {}",
        "if 0 {}",
        "let z=1",
        "check 1",
        "type t=int",
        "layout:word",
        "offset:int",
        "until:int",
        "each:int",
    ];
    for statement in statements {
        accepted(&format!("x:0.. {statement}"));
    }
    rejected("x:0.. each x", C::Placement, 1);
    accepted("a[1][1]:0.. layout \"FULL_MATRIX\" offset 1 list b:1.. until 0");
    accepted("x:0..format");
    accepted("x:0..(check)");
    accepted("x:0.. next:int");
    // eol takes priority over an expression; the + cannot continue it.
    rejected("list x:int until eol +1", C::Syntax, 1);
    rejected("x:0..< check 1", C::Syntax, 1);
}

#[test]
fn format_glue_and_placement() {
    for source in [
        "format dimacs-cnf.v2",
        "format iris-1",
        "format \"iris-1.0\"",
        "instances {} format last format \"final\"",
    ] {
        accepted(source);
    }
    for source in [
        "format:int",
        "format iris-1.0",
        "format a -b",
        "format a- b",
        "format a.#x\nb",
    ] {
        rejected(source, C::Syntax, 1);
    }
    for source in [
        "repeat 0 {comment \"#\"}",
        "if 0 {tokens \"x\"}",
        "if 0 {instances {}}",
        "instances {} instances {}",
        "instances {} a:int",
        "instances {} stop \"X\"",
        "each x",
        "repeat 1 {a:int each x}",
        "repeat 1 {each x each y}",
        "repeat 1 {each:int each x}",
        "repeat 1 {if 0 {each x}}",
    ] {
        rejected(source, C::Placement, 1);
    }
    accepted("repeat 1 {each:int}");
    accepted("index 0");
}

#[test]
fn functions_names_and_host_constants() {
    accepted(
        "let x=defined(((a))) let y=count(a) let z=sum(1,2) let w=distinct(a) let h=gecode.int_max",
    );
    accepted("let x=all(v in a:v>0) let y=any(v in a:v) let z=sum(v in a:v) let n=count(v in a:v)");
    accepted("let x=sum(v # comment\n in a:v)");
    for source in [
        "let x=foo(1)",
        "let x=size()",
        "let x=count(1,2)",
        "let x=defined(a[0])",
        "let x=defined(1)",
        "let x=min()",
        "let x=all(1)",
        "let x=any()",
    ] {
        rejected(source, C::Function, 1);
    }
    for source in ["let x=gecode.1", "let x=gecode.unknown()"] {
        rejected(source, C::Constant, 1);
    }
    rejected("let x=gecode.int_max()", C::Syntax, 1);
    accepted("let x=gecode . int_max");
    for source in [
        "gecode.x:int",
        "let gecode.x=1",
        "param gecode.x",
        "type gecode.x=int",
        "list gecode.x:int until 0",
        "repeat 1 as gecode.x {}",
        "repeat 1 {each gecode.x}",
        "repeat until \"X\" store gecode.x {}",
        "let x=sum(gecode.x in a:1)",
        "let _=1",
        "param _",
        "type _=int",
        "let x=sum(_ in a:1)",
    ] {
        rejected(source, C::Name, 1);
    }
    accepted("_:int list _:int until 0 repeat 1 as _ {each _} repeat until \"END\" store _ {}");
    let error = rejected("let x=foo\n(1)\na:int", C::Function, 1);
    assert!(
        error.diagnostics()[0]
            .to_string()
            .starts_with("spec.iris:1:")
    );
}

fn initializer(source: &str) -> SyntaxNode {
    accepted(source)
        .syntax()
        .children()
        .find(|node| node.kind() == K::Let)
        .unwrap()
        .children()
        .last()
        .unwrap()
}
fn expression_children(node: &SyntaxNode) -> Vec<SyntaxNode> {
    node.children().collect()
}

#[test]
fn precedence_and_association_are_in_the_tree() {
    let unary = initializer("let x=-2^2");
    assert_eq!(unary.kind(), K::Unary);
    assert_eq!(expression_children(&unary)[0].kind(), K::Power);
    let power = initializer("let x=2^3^2");
    assert_eq!(power.kind(), K::Power);
    assert_eq!(expression_children(&power)[1].kind(), K::Power);
    let power = initializer("let x=2^-1");
    assert_eq!(expression_children(&power)[1].kind(), K::Unary);
    for source in ["let x=8/2/2", "let x=1<2<3"] {
        let binary = initializer(source);
        assert_eq!(binary.kind(), K::Binary);
        assert_eq!(expression_children(&binary)[0].kind(), K::Binary);
    }
    let binary = initializer("let x=1||0&&2==3<4+5*6");
    let mut right = binary;
    for _ in 0..5 {
        assert_eq!(right.kind(), K::Binary);
        right = expression_children(&right)[1].clone();
    }
    assert_eq!(right.kind(), K::Binary);
    let postfix = initializer("let x=a[0].b[1]");
    assert_eq!(postfix.kind(), K::IndexExpression);
    assert_eq!(expression_children(&postfix)[0].kind(), K::Field);
}

#[test]
fn patterns_keep_raw_spelling_and_distinct_nodes() {
    let result = accepted("type t=0.. line \"é\\n} {x:t} {w} {} {:int} {_} {_:num} {$x} ... \\t\"");
    let root = result.syntax();
    assert_eq!(nodes(&root, K::Pattern).len(), 1);
    assert_eq!(nodes(&root, K::Capture).len(), 6);
    assert_eq!(nodes(&root, K::CurrentValue).len(), 1);
    assert_eq!(nodes(&root, K::Ellipsis).len(), 1);
    assert_eq!(nodes(&root, K::TypeName).len(), 1);
    let result = accepted(r#"line "{x:(\"a\\nb\")..1}""#);
    let range = nodes(&result.syntax(), K::Type).remove(0).text_range();
    assert_eq!(
        &r#"line "{x:(\"a\\nb\")..1}""#[usize::from(range.start())..usize::from(range.end())],
        r#"(\"a\\nb\")..1"#
    );
    accepted("line \"a\u{b}\u{c}b\" find \"}\" sections {\"{x}\"}");
    let result = accepted("expect \"{x}\" let s=\"{x}\" repeat until \"{x}\" {}");
    assert!(nodes(&result.syntax(), K::Pattern).is_empty());
    rejected("line \"...\r\"", C::Pattern, 1);
    rejected("line \"{bad name}\"", C::Pattern, 1);
    rejected("line \"{$x:y}\"", C::Pattern, 1);
    rejected(r#"line "{x:(\"}\")..1}""#, C::String, 1);
    rejected(r#"line "{x:\n}""#, C::Pattern, 1);
    // Coordinator amendment: all structure precedes lexing any capture type.
    rejected(r#"line "{x:@}{bad name}""#, C::Pattern, 1);
    rejected(r#"line "{x:@}...x""#, C::Pattern, 1);
    rejected(r#"line "{x:\n@}""#, C::Character, 2);
    rejected(r#"line "\n{x:@}""#, C::Character, 1);
    rejected(r#"line "{x:\nint}{y:@}""#, C::Character, 1);
    rejected(r#"line "{x:1.2.3}""#, C::Number, 1);
    rejected(r#"line "{x:0..<}""#, C::Syntax, 1);
}

#[test]
fn recovery_retains_later_declarations_and_siblings() {
    for broken in [
        "bogus",
        "let x=+",
        "x:0..<",
        "x[+]:int",
        "line \"{x:}\"",
        "line \"...x\"",
        "line \"{$}\"",
        "line \"{x:int int}\"",
        "line \"{x:@}\"",
        "let s=\"broken\n",
    ] {
        let source = format!("repeat 1 {{ {broken} after:int }} sibling:word");
        let result = checked(&source);
        assert!(!result.diagnostics().is_empty(), "{broken}");
        let declarations = nodes(&result.syntax(), K::Declaration);
        assert!(
            declarations
                .iter()
                .any(|node| node.text().to_string().starts_with("after:")),
            "{broken}: {:#?}",
            result.syntax()
        );
        assert!(
            result
                .syntax()
                .children()
                .any(|node| node.kind() == K::Declaration && node.text() == "sibling:word")
        );
        assert!(!nodes(&result.syntax(), K::Error).is_empty());
    }
    let result = checked("sections {bad \"good\" {after:int}} sibling:int");
    assert_eq!(nodes(&result.syntax(), K::Pattern).len(), 1);
    assert_eq!(nodes(&result.syntax(), K::Declaration).len(), 2);
}

#[test]
fn operational_nesting_boundaries() {
    // Each case tests the accepted boundary and its immediate successor.
    type NestingSource = fn(usize) -> String;
    let cases: [(usize, NestingSource); 8] = [
        (256, |n| {
            format!("{}{}", "repeat 0 {".repeat(n), "}".repeat(n))
        }),
        (255, |n| {
            format!("let x={}1{}", "(".repeat(n), ")".repeat(n))
        }),
        (255, |n| format!("let x={}1", "-".repeat(n))),
        (255, |n| format!("let x=1{}", "+1".repeat(n))),
        (255, |n| format!("let x=1{}", "^1".repeat(n))),
        (254, |n| format!("let x=a{}", "[0]".repeat(n))),
        (255, |n| format!("let x=a{}", ".a".repeat(n))),
        (255, |n| format!("if 0 {{}}{}", " else if 0 {}".repeat(n))),
    ];
    for (limit, source) in cases {
        accepted(&source(limit));
        rejected(&source(limit + 1), C::Nesting, 1);
    }
    let expression = format!("{}1", "-".repeat(255));
    accepted(&format!(
        "let a={expression} let b={expression} a[{expression}][{expression}]:int"
    ));
    let argument = format!("{}1", "-".repeat(254));
    accepted(&format!("let c=sum({argument},{argument})"));
    accepted(&format!(
        "line \"{{x:({})..1}}{{y:({})..1}}\"",
        "-".repeat(254) + "1",
        "-".repeat(254) + "1"
    ));
    rejected(
        &format!("line \"{{x:({})..1}}\"", "-".repeat(255) + "1"),
        C::Nesting,
        1,
    );
    rejected(&format!("let x={}\n1", "-".repeat(256)), C::Nesting, 2);
    rejected(&format!("let x=1{}\n+1", "+1".repeat(255)), C::Nesting, 2);
    rejected(&format!("let x=a{}.\na", ".a".repeat(255)), C::Nesting, 2);
    rejected(&format!("let x=a{}[\n0]", "[0]".repeat(254)), C::Nesting, 2);
    rejected(&format!("let x=1{}^\n1", "^1".repeat(255)), C::Nesting, 2);
    rejected(
        &format!(
            "{}repeat 0 {{\n}}{}",
            "repeat 0 {".repeat(256),
            "}".repeat(256)
        ),
        C::Nesting,
        2,
    );
    accepted(&format!(
        "{} sections {{\"x\"}} {}",
        "repeat 0 {".repeat(256),
        "}".repeat(256)
    ));
}

#[test]
fn fixture_truncation_sweep() {
    let mut count = 0;
    for (_, source) in FIXTURES {
        for end in 0..=source.len() {
            if source.is_char_boundary(end) {
                checked(&source[..end]);
                count += 1;
            }
        }
    }
    println!("fixture truncations checked: {count}");
}
