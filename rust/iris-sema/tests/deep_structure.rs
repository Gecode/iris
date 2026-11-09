//! Analysis of valid specs whose trees are far deeper than any thread's stack
//! would allow a walk with one frame per level. Each test runs on a small
//! thread.

use gecode_iris_syntax::LanguageVersion;

const SMALL_STACK: usize = 2 * 1024 * 1024;

fn on_small_stack(work: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(SMALL_STACK)
        .spawn(work)
        .unwrap()
        .join()
        .unwrap();
}

/// A left-nested chain through 254 parentheses where each level adds
/// `suffix` up to `budget - depth` times, the most Specification §5.12
/// allows: 32,640 suffixes for a budget of 255.
fn left_spine(operand: &str, suffix: &str, budget: usize) -> String {
    let mut text = "(".repeat(254);
    text.push_str(operand);
    for depth in (0..=254).rev() {
        text.push_str(&suffix.repeat(budget - depth));
        if depth > 0 {
            text.push(')');
        }
    }
    text
}

fn assert_valid(source: &str) {
    let parsed =
        gecode_iris_syntax::parse(source.as_bytes(), "deep.iris", LanguageVersion::V0_1).unwrap();
    let analysis = gecode_iris_sema::analyze(&parsed);
    assert!(analysis.is_valid(), "{:?}", analysis.first_error());
}

#[test]
fn deep_left_operands_analyze_on_a_small_stack() {
    on_small_stack(|| {
        assert_valid(&format!(
            "n: int\nlet s = {}\ncheck s > 0\n",
            left_spine("n", "+n", 255)
        ));
        assert_valid(&format!("n: int\ncheck {}\n", left_spine("n", "&&n", 255)));
        assert_valid(&format!("n: int\nx: 0..{}\n", left_spine("n", "*n", 255)));
        assert_valid(&format!(
            "x[3]: int\nlet s = sum(i in x: {})\n",
            left_spine("i", "-i", 254)
        ));
        let indices = left_spine("x", "[0]", 254);
        let dimensions = indices.matches("[0]").count();
        assert_valid(&format!(
            "x{}: int\nlet y = {indices}\ncheck y == 5\n",
            "[1]".repeat(dimensions)
        ));
    });
}

/// Statements and the operands that are not left operands still recurse, as
/// §5.12 bounds their nesting. Each construct here is at its limit.
#[test]
fn nesting_at_the_limits_fits_a_small_stack() {
    on_small_stack(|| {
        for expression in [
            "-".repeat(255) + "n",
            "n".to_owned() + &"^1".repeat(255),
            "max(".repeat(255) + "n" + &")".repeat(255),
            "(".repeat(255) + "n" + &")".repeat(255),
            "n+(".repeat(127) + "n" + &")".repeat(127),
            "-(".repeat(127) + "n" + &")".repeat(127),
            "x[".repeat(127) + "0" + &"]".repeat(127),
        ] {
            assert_valid(&format!(
                "n: int\nx[1]: int\n{}let y = {expression}\n{}",
                "if n == 1 {\n".repeat(256),
                "}\n".repeat(256)
            ));
            assert_valid(&format!(
                "n: int\nx[1]: int\nif n == 0 {{\n}}{} else if n == 255 {{\n  let y = {expression}\n}}\n",
                (1..255)
                    .map(|arm| format!(" else if n == {arm} {{\n  let y = n\n}}"))
                    .collect::<String>(),
            ));
        }
        assert_valid(&format!(
            "n: int\n{}y: int\n{}",
            "repeat n {\n".repeat(256),
            "}\n".repeat(256)
        ));
    });
}

fn deep_spec() -> gecode_iris_syntax::Parse {
    let source = format!("n: int\nlet s = {}\n", left_spine("n", "+n", 255));
    gecode_iris_syntax::parse(source.as_bytes(), "deep.iris", LanguageVersion::V0_1).unwrap()
}

/// The last owner of a deep tree frees it, whichever owner that is.
#[test]
fn an_analysis_frees_a_deep_tree_after_its_parse() {
    on_small_stack(|| {
        let analysis = {
            let parsed = deep_spec();
            gecode_iris_sema::analyze(&parsed)
        };
        assert!(analysis.is_valid(), "{:?}", analysis.first_error());
        drop(analysis);
    });
}

#[test]
fn a_semantic_model_frees_a_deep_tree_after_its_parse_and_analysis() {
    on_small_stack(|| {
        let parsed = deep_spec();
        let model = gecode_iris_sema::analyze(&parsed).model;
        let copy = model.clone();
        drop(parsed);
        assert!(!model.expressions().is_empty());
        drop(model);
        drop(copy);
    });
}
