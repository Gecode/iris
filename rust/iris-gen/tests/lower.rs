//! Lowering a reader spec to the language-independent program.
use gecode_iris_gen::lower;
use gecode_iris_gen::program::{Event, Name, NameKind, Program, RepeatNames};
use gecode_iris_sema::analyze;
use gecode_iris_syntax::{LanguageVersion, parse};

fn program(source: &str) -> Program {
    let parsed = parse(source.as_bytes(), "test.iris", LanguageVersion::V0_1).unwrap();
    let analysis = analyze(&parsed);
    lower(&parsed, &analysis).expect("the spec is accepted")
}

fn opener_names(program: &Program) -> Vec<&'static str> {
    let names = program.events.iter().filter_map(|event| match event {
        Event::If(_) => Some("if"),
        Event::ElseIf(_) => Some("else if"),
        Event::Else => Some("else"),
        Event::ElseRead { .. } => Some("else read"),
        Event::Close => Some("close"),
        _ => None,
    });
    names.collect()
}

#[test]
fn a_rejected_spec_gives_the_first_error_of_the_check() {
    let parsed = parse(b"let x = nothing(1)\n", "test.iris", LanguageVersion::V0_1).unwrap();
    let analysis = analyze(&parsed);
    let error = lower(&parsed, &analysis).expect_err("the function does not exist");
    assert_eq!(Some(&error), analysis.first_error());
}

#[test]
fn a_quoted_format_name_keeps_every_word() {
    let program = program("format \"two words\"\n");
    assert_eq!(program.events, [Event::Format("two words".to_owned())]);
}

#[test]
fn an_unquoted_format_name_joins_its_tokens() {
    let program = program("format tsp-lib\n");
    assert_eq!(program.events, [Event::Format("tsp-lib".to_owned())]);
}

#[test]
fn an_if_chain_lowers_to_flat_branches() {
    let source = "x: int\nif x > 0 {\n} else if x > 1 {\n} else {\n}\n";
    assert_eq!(
        opener_names(&program(source)),
        ["if", "close", "else if", "close", "else", "close"]
    );
}

#[test]
fn an_else_declaration_of_the_name_if_is_not_a_block() {
    let source = "x: int\nif x > 0 {\n} else if: int\n";
    assert_eq!(opener_names(&program(source)), ["if", "close", "else read"]);
}

#[test]
fn a_name_defined_nowhere_is_undefined_and_a_defined_one_forward() {
    let program = program("x: int\nlet y = defined(z)\n");
    let names: Vec<_> = program
        .names
        .iter()
        .map(|Name { text, kind }| (text.as_str(), *kind))
        .collect();
    assert_eq!(
        names,
        [
            ("x", NameKind::Forward),
            ("z", NameKind::Undefined),
            ("y", NameKind::Forward)
        ]
    );
}

#[test]
fn a_repeat_that_nothing_refers_to_keeps_its_names_in_place() {
    let program = program("repeat 2 as r {\n  each c\n}\n");
    let names = program.events.iter().find_map(|event| match event {
        Event::Repeat { names, .. } => Some(names),
        _ => None,
    });
    let Some(RepeatNames::InPlace { each, list }) = names else {
        panic!("the repeat is not forward: {names:?}");
    };
    assert_eq!((each.as_deref(), list.as_deref()), (Some("c"), Some("r")));
    assert!(program.records.is_empty());
}

#[test]
fn a_referred_repeat_without_an_each_name_gets_an_unused_invented_one() {
    let program = program("record_1: int\nrepeat 2 as r {\n  x: int\n}\nlet y = r[0].x\n");
    assert_eq!(program.records.len(), 1);
    assert_eq!(program.records[0].list, "r");
    assert_eq!(program.records[0].each, "record_2");
}

#[test]
fn closure_depth_counts_blocks_arms_and_comprehensions() {
    assert_eq!(program("x: int\n").closure_depth, 0);
    assert_eq!(
        program("repeat 1 {\n  repeat 1 {\n  }\n}\n").closure_depth,
        2
    );
    // `sections` opens one closure for its arms and each arm one for its block.
    assert_eq!(program("sections {\n  \"a\" {\n  }\n}\n").closure_depth, 2);
    assert_eq!(
        program("xs[1]: int\nlet y = sum(v in xs: sum(w in xs: w))\n").closure_depth,
        2
    );
}

#[test]
fn lowering_does_not_use_the_stack_for_long_chains() {
    // Far more statements than any stack could hold frames for: only nesting
    // pushes a frame, and the language bounds that.
    let source = "skip line\n".repeat(200_000);
    assert_eq!(program(&source).events.len(), 200_000);
}
