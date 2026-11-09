//! Reading with valid specs whose trees are far deeper than any thread's
//! stack would allow a walk with one frame per level. Each test runs on a
//! small thread, and drops the spec there too.

use gecode_iris_core::{Params, Spec};

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

fn read(spec: &str, input: &str) -> (String, String) {
    let spec = Spec::parse(spec.as_bytes(), "deep.iris").unwrap();
    let data = spec
        .read(input.as_bytes(), "deep.txt", &Params::new())
        .unwrap();
    let dzn = String::from_utf8(data.to_dzn().unwrap()).unwrap();
    let json = String::from_utf8(data.to_json().unwrap()).unwrap();
    (dzn, json)
}

#[test]
fn deep_left_operands_read_on_a_small_stack() {
    on_small_stack(|| {
        let (dzn, _) = read(
            &format!(
                "n: int\nlet s = {}\ncheck s == 32641\n",
                left_spine("n", "+n", 255)
            ),
            "1\n",
        );
        assert!(dzn.ends_with("s = 32641;\n"), "{dzn}");
        read(
            &format!(
                "n: int\ncheck {}\nif {} {{ m: int }}\n",
                left_spine("n", "&&n", 255),
                left_spine("n", "||n", 255)
            ),
            "1 2\n",
        );
        let (dzn, _) = read(
            &format!(
                "x[3]: int\nlet s = sum(i in x: {})\n",
                left_spine("i", "*i", 254)
            ),
            "1 1 -1\n",
        );
        // Each product has 32,386 factors, an even number.
        assert!(dzn.ends_with("s = 3;\n"), "{dzn}");
    });
}

#[test]
fn deep_index_chains_and_dimensions_read_on_a_small_stack() {
    on_small_stack(|| {
        let indices = left_spine("x", "[0]", 254);
        let dimensions = indices.matches("[0]").count();
        let (dzn, json) = read(
            &format!(
                "x{}: int\nlet y = {indices}\ncheck y == 5\n",
                "[1]".repeat(dimensions)
            ),
            "5\n",
        );
        assert!(dzn.ends_with("y = 5;\n"), "{dzn}");
        assert!(json.contains(&format!(
            "{}5{}",
            "[".repeat(dimensions),
            "]".repeat(dimensions)
        )));
    });
}
