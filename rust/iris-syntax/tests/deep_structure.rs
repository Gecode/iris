//! Valid specs whose trees are far deeper than any thread's stack would allow
//! a walk with one frame per level. Each test runs on a small thread.

use gecode_iris_syntax::{
    FormatOptions, LanguageVersion, SyntaxKind as K, WalkEvent, format, parse,
};

const SMALL_STACK: usize = 2 * 1024 * 1024;

fn on_small_stack(work: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(SMALL_STACK)
        .spawn(work)
        .unwrap()
        .join()
        .unwrap();
}

/// A left-nested chain through 254 parentheses. Specification §5.12 counts
/// the enclosing parentheses against each chain, so the chain inside depth
/// `d` has `255 - d` operators: 32,641 operands in all.
fn left_spine(operand: &str, operator: &str) -> String {
    left_spine_within(operand, operator, 255)
}

/// A [`left_spine`] for a place that already holds `255 - budget` entries.
fn left_spine_within(operand: &str, operator: &str, budget: usize) -> String {
    let mut text = "(".repeat(254);
    text.push_str(operand);
    for depth in (0..=254).rev() {
        text.push_str(&format!("{operator}{operand}").repeat(budget - depth));
        if depth > 0 {
            text.push(')');
        }
    }
    text
}

/// Index suffixes, then operators, at every level: a left spine of about
/// twice the depth of [`left_spine`].
fn mixed_spine(levels: usize) -> String {
    let mut text = "(".repeat(levels);
    text.push('x');
    for depth in (0..=levels).rev() {
        text.push_str(&"[0]".repeat(254 - depth));
        text.push_str(&"+1".repeat(254 - depth));
        if depth > 0 {
            text.push(')');
        }
    }
    text
}

fn depth(source: &str) -> usize {
    let parsed = parse(source.as_bytes(), "deep.iris", LanguageVersion::V0_1).unwrap();
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let (mut depth, mut deepest) = (0, 0);
    for event in parsed.syntax().preorder() {
        match event {
            WalkEvent::Enter(_) => depth += 1,
            WalkEvent::Leave(_) => depth -= 1,
        }
        deepest = deepest.max(depth);
    }
    deepest
}

fn significant_tokens(source: &str) -> Vec<(K, String)> {
    let parsed = parse(source.as_bytes(), "deep.iris", LanguageVersion::V0_1).unwrap();
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    parsed
        .syntax()
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| token.kind() != K::Whitespace)
        .map(|token| (token.kind(), token.text().to_owned()))
        .collect()
}

fn formatted(source: &str) -> String {
    let options = FormatOptions { line_width: 100 };
    let output = format(
        source.as_bytes(),
        "deep.iris",
        LanguageVersion::V0_1,
        options,
    )
    .unwrap();
    assert_eq!(significant_tokens(&output), significant_tokens(source));
    output
}

#[test]
fn deep_left_operands_parse_and_free_on_a_small_stack() {
    on_small_stack(|| {
        assert!(depth(&format!("let s = {}\n", left_spine("1", "+"))) > 32_000);
        assert!(depth(&format!("check {}\n", left_spine("1", "&&"))) > 32_000);
        assert!(depth(&format!("let s = {}\n", mixed_spine(254))) > 64_000);
    });
}

/// The last owner of a deep tree frees it, whichever owner that is.
#[test]
fn every_owner_of_a_deep_tree_frees_it_last() {
    on_small_stack(|| {
        let source = format!("let s = {}\n", left_spine("1", "+"));
        let parse = || parse(source.as_bytes(), "deep.iris", LanguageVersion::V0_1).unwrap();

        let parsed = parse();
        let copy = parsed.clone();
        drop(parsed);
        drop(copy);

        let parsed = parse();
        let tree = parsed.tree().clone();
        let node = parsed.syntax().last_child().unwrap();
        drop(parsed);
        assert_eq!(node.kind(), K::Let);
        drop(tree.root());
        drop(node);
        drop(tree);
    });
}

#[test]
fn deep_left_operands_format_on_a_small_stack() {
    on_small_stack(|| {
        let source = format!("let s = {}\n", left_spine("n", "+"));
        let output = formatted(&source);
        assert_eq!(formatted(&output), output);
        formatted(&format!("x: 0..{}\n", left_spine("1", "*")));
        formatted(&format!(
            "let s = sum(i in x: {})\n",
            left_spine_within("i", "||", 254)
        ));
        // Formatting a long run of index suffixes takes quadratic time, so
        // this spine is shorter than the one parsed above.
        formatted(&format!("let s = {}\n", mixed_spine(16)));
    });
}

/// Statements and the operands that are not left operands still recurse, as
/// §5.12 bounds their nesting. Each construct here is at its limit.
#[test]
fn nesting_at_the_limits_fits_a_small_stack() {
    on_small_stack(|| {
        let expressions = [
            "-".repeat(255) + "n",
            "n".to_owned() + &"^1".repeat(255),
            "max(".repeat(255) + "n" + &")".repeat(255),
            "(".repeat(255) + "n" + &")".repeat(255),
            "n+(".repeat(127) + "n" + &")".repeat(127),
            "-(".repeat(127) + "n" + &")".repeat(127),
            "x[".repeat(127) + "0" + &"]".repeat(127),
        ];
        for (number, expression) in expressions.iter().enumerate() {
            let source = format!(
                "n: int\nx[1]: int\n{}let y = {expression}\n{}",
                "if n == 1 {\n".repeat(256),
                "}\n".repeat(256)
            );
            depth(&source);
            // Nested index expressions take exponential time to format once
            // they overflow the line, so the last one is only parsed.
            if number + 1 < expressions.len() {
                formatted(&source);
            }
        }
        let else_if = format!(
            "n: int\nif n == 0 {{\n}}{} else if n == 255 {{\n  let y = {}\n}}\n",
            (1..255)
                .map(|arm| format!(" else if n == {arm} {{\n  let y = n\n}}"))
                .collect::<String>(),
            expressions[2]
        );
        depth(&else_if);
        formatted(&else_if);
        let repeats = format!(
            "n: int\n{}y: int\n{}",
            "repeat n {\n".repeat(256),
            "}\n".repeat(256)
        );
        depth(&repeats);
        formatted(&repeats);
    });
}
