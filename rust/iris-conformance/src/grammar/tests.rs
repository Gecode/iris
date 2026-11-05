use super::*;

fn accepts(grammar: &Grammar, source: impl AsRef<[u8]>) -> bool {
    match Recognizer::recognize(grammar, source.as_ref()) {
        Ok(()) => true,
        Err(Failure::Rejected(_)) => false,
        Err(error) => panic!("recognizer configuration failure: {error:?}"),
    }
}

#[test]
fn notation_is_interpreted_and_consumed_completely() {
    let fixture = include_str!("../../tests/fixtures/notation.ebnf");
    let grammar = Grammar::parse(fixture).unwrap();
    for text in ["ab\\", "acde\te\\"] {
        Recognizer::lex_source(&grammar, text, 1).unwrap();
    }
    assert!(Recognizer::lex_source(&grammar, "ax\\", 1).is_err());
    let changed = Grammar::parse(&fixture.replace("'a'", "'z'")).unwrap();
    assert!(Recognizer::lex_source(&changed, "ab\\", 1).is_err());
    Recognizer::lex_source(&changed, "zb\\", 1).unwrap();
    for text in [
        "source = 'a'; @",
        "source = 'a'; (*",
        "source = 'a;",
        "source = ? x;",
        "source = missing;",
        "source = 'a'; source = 'b';",
        "source = ? unknown ?;",
        "source = ['a';",
        "source = 'a',;",
        "source = , 'a';",
        "source = 'a'||'b';",
    ] {
        assert!(Grammar::parse(text).is_err(), "{text}");
    }
    for text in ["source = { ['a'] };", "source = source | 'a';"] {
        let grammar = Grammar::parse(text).unwrap();
        assert!(matches!(
            Recognizer::lex_source(&grammar, "", 1),
            Err(Failure::Configuration(_))
        ));
    }
}

#[test]
fn lexical_productions_control_recognition() {
    let grammar = Grammar::parse(GRAMMAR).unwrap();
    assert!(accepts(&grammar, r#"let x="\t""#));
    let no_tab = Grammar::parse(&GRAMMAR.replace("'n' | 't' |", "'n' |")).unwrap();
    assert!(!accepts(&no_tab, r#"let x="\t""#));
    let no_plus = Grammar::parse(&GRAMMAR.replace("| '+' | '-' | '*'", "| '-' | '*'")).unwrap();
    assert!(!accepts(&no_plus, "let x=1+2"));
    let reordered = Grammar::parse(
        &GRAMMAR
            .replace("host_name | ordinary_name", "ordinary_name | host_name")
            .replace("'..<' | '..'", "'..' | '..<'"),
    )
    .unwrap();
    assert!(accepts(&reordered, "a:0..<9 let x=gecode.int_max"));
    // Even if punctuation could consume a quote, malformed selected strings
    // must not fall back to punctuation during lexical dispatch.
    let quote_punctuation =
        Grammar::parse(&GRAMMAR.replace("'..<' | '..' | '=='", "'\"' | '..<' | '..' | '=='"))
            .unwrap();
    assert!(source::lex(&quote_punctuation, b"\"", 1).is_err());
}

#[test]
fn sources_preserve_lexical_boundaries_and_token_kinds() {
    let grammar = Grammar::parse(GRAMMAR).unwrap();
    for text in [
        "",
        "# café\rstill comment",
        "a:int",
        "let s=\"λ\r\0\"\r\na:int",
        r#"let s="a\nb\tc\\d\"e""#,
        "let n=007 let x=1.e2 let y=01e+2",
        "let n=9223372036854775807 let z=0e-9999",
        "let n=2.2250738585072014e-308",
        "let n=2.2250738585072012e-308",
        "let x=1e2foo:int",
        "let x=gecode.123",
    ] {
        assert!(accepts(&grammar, text), "{text:?}");
    }
    for text in [
        "\u{feff}a:int",
        "λ:int",
        "let x=@",
        "let x=\0",
        "a:int;",
        "let x=.5",
        "let x=+1",
        "let x=1.2.3",
        "let x=1e",
        "let x=9223372036854775808",
        "let x=1e9999",
        "let x=1e-9999",
        "let x=2.225073858507201e-308",
        "let x=\"unclosed",
        "let x=\"a\nb\"",
        r#"let x="\r""#,
        "\"format\" x",
        "\"a\":int",
        "if 1 \"{\" a:int \"}\"",
    ] {
        assert!(!accepts(&grammar, text), "{text:?}");
    }
    for bytes in [
        &b"#\xff"[..],
        b"let x=\"\xc0\x80\"",
        b"#\xed\xa0\x80",
        b"#\xf4\x90\x80\x80",
        b"#\xe2\x82",
    ] {
        assert!(!accepts(&grammar, bytes), "{bytes:?}");
    }
    let tokens = source::lex(&grammar, b"format a-b\r\n# hi\r\nx:int", 1).unwrap();
    assert_eq!(tokens.last().unwrap().line, 3);
    assert_eq!(tokens[1].span.end, tokens[2].span.start);
}

#[test]
fn contextual_lookahead_commits_to_the_selected_form() {
    let grammar = Grammar::parse(GRAMMAR).unwrap();
    for (text, expected) in [
        ("a:0.. layout:int", true),
        ("a:0..< layout:int", false),
        (r#"if["["]:int"#, true),
        ("if[1+]:int", false),
        ("if 1 {} else if:int", true),
        ("if 1 {} else if[2]:int", true),
        ("if 1 {format:int instances:int comment:int each:int}", true),
        ("format:int", false),
        ("format dimacs-cnf.v2 instances {} format end", true),
        ("format a-b:int", false),
        ("format iris-1.0", false),
        ("format a -b:int", false),
        ("format a b:int", true),
        ("instances {} a:int", false),
        ("repeat 1 {each x a:int}", true),
        ("repeat 1 {a:int each x}", false),
        ("if 0 {comment \"#\"}", false),
        ("type t=int a:t", true),
        ("type t=int a:t..", false),
        ("type t=int a:(t)..", true),
        ("a:0.. format x", false),
        ("a:0.. format", true),
        ("let x=all(i in xs: i)", true),
        ("let x=any(i in xs: i)", true),
        ("let x=sum(i in xs: i)", true),
        ("let x=count(i in xs: i)", true),
        ("let x=other(i in xs: i)", false),
        ("let x=all(i in xs)", false),
        (
            "let x=unknown(1,2,3) let y=all() let z=sum(count(1),2)",
            true,
        ),
        ("let x=gecode.int_max()", false),
        ("let x=1\n+2", true),
        ("let x=1\n+", false),
        ("let x=f(", false),
        ("let x=a[", false),
        ("let x=a.", false),
        ("let x=1^", false),
    ] {
        assert_eq!(accepts(&grammar, text), expected, "{text:?}");
    }
}

#[test]
fn patterns_use_scoped_types_and_full_decoded_text() {
    let grammar = Grammar::parse(GRAMMAR).unwrap();
    for text in [
        r#"type t=0.. type u=t line "{x:u} {$x} {:int} {_} {} }\n...\t""#,
        r#"if 0 {type t=int line "{x:t}"} if 0 {type t=word line "{x:t}"}"#,
        r#"type t=int if 1 {line "{x:t}"}"#,
        // Leaving a block restores the kind of a type it shadowed.
        r#"type t=int if 0 {type t=alphabet "ab"} line "{x:t}""#,
        r#"line "} literal }""#,
        r#"line "{x:0..} {y:..9} {z:0..<9}""#,
        "line \"a\rb...\t\"",
        // Semantic negatives remain recognizable.
        "let _=f() type int=word a[2]:float layout 1 offset \"x\" let z=r.missing",
        "type t=int type t=word t:int",
    ] {
        assert!(accepts(&grammar, text), "{text:?}");
    }
    for text in [
        "a:t type t=int",
        "type t=t",
        "if 1 {type t=int} a:t",
        r#"line "{x:alphabet \"ab\"}""#,
        r#"type t=alphabet "ab" type u=t line "{x:u}""#,
        r#"type t=alphabet "ab" if 0 {type t=int} line "{x:t}""#,
        r#"line "{ x:int}""#,
        r#"line "{x :int}""#,
        r#"line "{$ x}""#,
        r#"line "{x:}""#,
        r#"line "{x:int float}""#,
        r#"line "{x""#,
        r#"line "{x:\"}\"..}""#,
        "line \"...\r\"",
        r#"line "...x""#,
    ] {
        assert!(!accepts(&grammar, text), "{text:?}");
    }
    let result = Recognizer::recognize(&grammar, b"line \"{x:unknown} { bad}\"");
    let Err(Failure::Rejected(error)) = result else {
        panic!("{result:?}");
    };
    assert_eq!(error.layer, "pattern");
    assert_ne!(
        error.rule, "type",
        "structure must be checked before capture types"
    );
    let result = Recognizer::recognize(&grammar, b"\nline \"{x:unknown}\"");
    let Err(Failure::Rejected(error)) = result else {
        panic!("{result:?}");
    };
    assert_eq!(error.layer, "pattern");
    assert_eq!(error.line, 2);
    assert!(error.to_string().contains("decoded byte"));
}

#[test]
fn operational_counters_admit_exactly_the_specification_boundaries() {
    let grammar = Grammar::parse(GRAMMAR).unwrap();
    type SourceBuilder = fn(usize) -> String;
    let constructions: [(usize, SourceBuilder); 8] = [
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
    for (boundary, make_source) in constructions {
        let admitted = make_source(boundary);
        assert!(
            accepts(&grammar, &admitted),
            "boundary {boundary}: {admitted}"
        );
        assert!(
            !accepts(&grammar, make_source(boundary + 1)),
            "limit {}",
            boundary + 1
        );
    }
    let nested_sections = |n| format!("{}{}", "sections { \"x\" {".repeat(n), "}}".repeat(n));
    assert!(accepts(&grammar, nested_sections(256)));
    assert!(!accepts(&grammar, nested_sections(257)));
    // Completed argument and dimension entries do not accumulate.
    let deep = format!("{}1{}", "(".repeat(254), ")".repeat(254));
    assert!(accepts(
        &grammar,
        format!("a[{deep}][{deep}]:int let x=f({deep},{deep})")
    ));
    assert!(!accepts(&grammar, format!("let x=-({deep})")));
    // Captured range expressions retain their enclosing block counter.
    assert!(accepts(
        &grammar,
        format!(
            "{}line \"{{x:0..}}\"{}",
            "if 1 {".repeat(256),
            "}".repeat(256)
        )
    ));
}

fn tree(source: &str) -> Vec<TreeEvent<'static>> {
    syntax_tree(source.as_bytes()).expect("the source is recognized")
}

fn token_texts(tree: &[TreeEvent<'_>]) -> Vec<String> {
    tree.iter()
        .filter_map(|event| match event {
            TreeEvent::Token(token) => Some(token.text.clone()),
            _ => None,
        })
        .collect()
}

fn opened<'t>(tree: &'t [TreeEvent<'_>]) -> Vec<&'t str> {
    tree.iter()
        .filter_map(|event| match event {
            TreeEvent::Open(rule) => Some(*rule),
            _ => None,
        })
        .collect()
}

fn assert_balanced(tree: &[TreeEvent<'_>]) {
    let mut depth = 0_usize;
    for event in tree {
        match event {
            TreeEvent::Open(_) => depth += 1,
            TreeEvent::Close => depth = depth.checked_sub(1).expect("a Close matches an Open"),
            TreeEvent::Token(_) => assert!(depth > 0, "a token is inside a rule"),
        }
    }
    assert_eq!(depth, 0);
}

#[test]
fn a_tree_holds_every_token_in_rules() {
    let tree = tree("let x = (1 + y)");
    assert_balanced(&tree);
    assert_eq!(
        token_texts(&tree),
        ["let", "x", "=", "(", "1", "+", "y", ")"]
    );
    let rules = opened(&tree);
    assert_eq!(rules[0], "reader_spec");
    assert!(rules.contains(&"let_stmt"));
    assert!(rules.contains(&"additive"));
}

#[test]
fn backtracking_truncates_the_tree() {
    // The first range alternative reads `1` and fails at `..<`, and `starts`
    // tries the operators that never follow; neither leaves anything behind.
    let tree = tree("a:1..<2 b:1..2 c:..3");
    assert_balanced(&tree);
    assert_eq!(
        token_texts(&tree),
        [
            "a", ":", "1", "..<", "2", "b", ":", "1", "..", "2", "c", ":", "..", "3"
        ]
    );
}

#[test]
fn a_failed_block_alternative_leaves_no_type_or_rule() {
    let tree = tree("type t=int if a { x:t } else if b { y:t }");
    assert_balanced(&tree);
    let rules = opened(&tree);
    assert_eq!(rules.iter().filter(|r| **r == "if_stmt").count(), 2);
    assert_eq!(rules.iter().filter(|r| **r == "type_stmt").count(), 1);
}

#[test]
fn patterns_are_in_the_tree_with_capture_types_in_place() {
    let tree = tree(r#"line "a {x:0..2} {_} ... ""#);
    assert_balanced(&tree);
    assert_eq!(
        token_texts(&tree),
        [
            "line", "a", " ", "{", "x", ":", "0", "..", "2", "}", " ", "{", "_", "}", " ", "...",
            " "
        ]
    );
    let rules = opened(&tree);
    let capture_type = rules
        .iter()
        .position(|rule| *rule == "capture_type")
        .expect("the capture has a type");
    assert_eq!(rules[capture_type + 1], "type");
}

#[test]
fn several_capture_types_keep_their_order_and_lines() {
    let tree = tree("\n\nline \"{a:int}{b:word}\"");
    assert_balanced(&tree);
    let texts = token_texts(&tree);
    let int = texts.iter().position(|text| text == "int").unwrap();
    let word = texts.iter().position(|text| text == "word").unwrap();
    assert!(int < word);
    assert!(tree.iter().all(|event| match event {
        TreeEvent::Token(token) => token.line == 3,
        _ => true,
    }));
}

#[test]
fn tree_rejections_are_the_recognizers() {
    let error = syntax_tree(b"let").expect_err("the statement is incomplete");
    assert!(error.contains("tokens / "), "{error}");
}

#[test]
fn corpus_trees_are_balanced() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cases = case::discover(&[repository.join("formats")], None).unwrap();
    assert!(!cases.is_empty());
    for path in cases {
        let case = Case::load(&path, "0.1").unwrap();
        assert_balanced(&syntax_tree(&fs::read(&case.spec).unwrap()).unwrap());
    }
}
