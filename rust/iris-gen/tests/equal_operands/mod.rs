//! Expressions that do or do not need the `clippy::eq_op` allowance.
//!
//! The round trip lints these builders, with `#[expect]` in place of `#[allow]`,
//! so a case in the wrong list fails there.

/// What every expression below may refer to.
const DECLARATIONS: &str = "x: int\ny: int\na[2]: int\nrepeat 2 as r {\neach e\nf: int\n";

/// Operands that `clippy::eq_op` takes to be the same.
pub const EQUAL: &[&str] = &[
    "x - x",
    "x / x",
    "x || x",
    "x && x",
    "(x - y) - (x - y)",
    "(x || y) || (x || y)",
    "(x && y) && (x && y)",
    "(x - y) / (x - y)",
    "(x - 1) - (x - 1)",
    "(-x) - (-x)",
    "!x || !x",
    "((x + y) * 2) - ((x + y) * 2)",
    "(x - (y - y)) * 2",
    // `-` is not the outermost operator, but it still has two equal operands.
    "x * y - x * y + 1",
];

/// Operands that differ, or that only look alike once they are text.
pub const DIFFERENT: &[&str] = &[
    // The text on both sides of the middle operator is the same, but the
    // operator is not the outermost one.
    "x + y - x + y",
    "x - y - x - y",
    "x || y || x || y",
    "x && y && x && y",
    // These operators mean something between equal operands.
    "x + x",
    "x * x",
    "x % x",
    "(x - y) * (x - y)",
    "(x - y) + (x - y)",
    // Other operands in another order.
    "(x - y) - (y - x)",
    "(x * y) - (y * x)",
    "(x + y) - (y + x)",
    "(x || y) && (y || x)",
    // A call may have side effects, so the lint never takes two to be equal.
    "(2 - x) - (2 - x)",
    "(x ^ 2) - (x ^ 2)",
    "a[0] - a[0]",
    "e.f - e.f",
    "sum(a) - sum(a)",
    "(x == y) || (x == y)",
    "sum(v in a: v - v)",
    // Each side is too long for one line, so each goes into a `let` of its own.
    "(x + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1) \
     - (x + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1)",
];

/// The spec that evaluates `expression` once.
pub fn spec(expression: &str) -> String {
    format!("{DECLARATIONS}let z = {expression}\n}}\n")
}
