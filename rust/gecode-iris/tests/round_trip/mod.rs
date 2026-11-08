//! What the builder round trips share: the corpus, the specs beyond it, and
//! the comparison of a printed spec with its original.
#[path = "../../../iris-gen/tests/equal_operands/mod.rs"]
mod equal_operands;
#[path = "../../../iris-gen/tests/layout_forms/mod.rs"]
mod layout_forms;

use gecode_iris_cli::check_spec;
use gecode_iris_conformance::compare::compare;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

/// Every distinct `.iris` file of the conformance cases and format library.
pub fn corpus(repo: &Path) -> BTreeSet<PathBuf> {
    let mut pending = vec![repo.join("conformance/cases/0.1"), repo.join("formats")];
    let mut specs = BTreeSet::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else if path
                .extension()
                .is_some_and(|extension| extension == "iris")
            {
                specs.insert(fs::canonicalize(path).unwrap());
            }
        }
    }
    specs
}

/// Lengths of names and text for every layout form, on either side of the widths
/// where a call, then a line, is full: 16 is below the longest C++ identifier,
/// and 72 is where a C++ string constant takes a second line.
const LAYOUT_LENGTHS: &[usize] = &[16, 33, 56, 72, 96];
/// The forms whose closures end a block with a value, which is where
/// `clippy::let_and_return` looks, with every length up to this.
const BLOCK_ENDS: (&[&str], usize) = (
    &[
        "comprehension-binary",
        "comprehension-block",
        "comprehension-nested-domain",
        "comprehension-tail",
    ],
    100,
);

/// Specs beyond the corpus: every layout form at a few widths, the ends of
/// closures at every width, the operands of `clippy::eq_op`, the names and
/// types that a backend must handle, by name, and literals near constants.
pub fn extra_specs() -> Vec<(String, String)> {
    let mut specs = Vec::new();
    let mut add = |title: &str, form: &str, length: usize, depth: usize| {
        let (name, text) = ("n".repeat(length), "t".repeat(length));
        let module = format!("{}_{depth}_{length}", title.replace('-', "_"));
        specs.push((module, layout_forms::spec(form, &name, &text, depth)));
    };
    for (title, form) in layout_forms::FORMS {
        let (ends, longest) = BLOCK_ENDS;
        for depth in [0, 4, 7] {
            if depth > 0 && !layout_forms::nests(form) {
                continue;
            }
            for &length in LAYOUT_LENGTHS {
                add(title, form, length, depth);
            }
            if ends.contains(title) && depth < 7 {
                (1..=longest)
                    .filter(|length| !LAYOUT_LENGTHS.contains(length))
                    .for_each(|length| add(title, form, length, depth));
            }
        }
    }
    let operands = [
        ("equal", equal_operands::EQUAL),
        ("different", equal_operands::DIFFERENT),
    ];
    for (title, expressions) in operands {
        for (index, expression) in expressions.iter().enumerate() {
            specs.push((format!("{title}_{index}"), equal_operands::spec(expression)));
        }
    }
    specs.push(("reserved_names".to_owned(), reserved_names()));
    specs.push(("numbered_prefixes".to_owned(), numbered_prefixes()));
    specs.push(("helper_types".to_owned(), helper_types()));
    specs.push(("host_constants".to_owned(), host_constants()));
    specs.push(("near_constants".to_owned(), NEAR_CONSTANTS.to_owned()));
    specs
}

/// Float literals at or near mathematical constants, which linters such as
/// clippy's `approx_constant`, clang-tidy's `modernize-use-std-numbers` and
/// oxlint's `approx-constant` take for the constants. A builder prints them
/// as the spec writes them.
const NEAR_CONSTANTS: &str = "let pi = 3.141592653589793\nlet e = 2.718281828459045\n\
                              let half = -0.7071\nrepeat 1 {\n  let ln = 0.6931 + 2.3026\n  \
                              let near = 3.14 + 1.5708\n}\n";

/// The host constants in every position where `iris check` accepts one: as
/// operands beside each kind of value, and as the arrays, domains and bounds
/// that need an expression, such as `size(gecode.int_max)`.
fn host_constants() -> String {
    const OPERATORS: &[&str] = &[
        "+", "-", "*", "/", "%", "^", "==", "!=", "<", "<=", ">", ">=", "&&", "||",
    ];
    // An integer, a string, a float, an integer handle, an array and a host
    // constant, on either side.
    const PARTNERS: &[&str] = &["1", "\"s\"", "1.5", "x", "a", "H"];
    let mut expressions = vec![
        "-H".to_owned(),
        "!H".to_owned(),
        "-(-H)".to_owned(),
        "H[0]".to_owned(),
        "a[H]".to_owned(),
        "H[H]".to_owned(),
        "size(H)".to_owned(),
        "count(H)".to_owned(),
    ];
    for operator in OPERATORS {
        for partner in PARTNERS {
            expressions.push(format!("H {operator} {partner}"));
            if *partner != "H" {
                expressions.push(format!("{partner} {operator} H"));
            }
        }
    }
    for function in ["min", "max", "sum", "distinct"] {
        expressions.extend([
            format!("{function}(H)"),
            format!("{function}(H, 1)"),
            format!("{function}(1, H)"),
        ]);
    }
    for comprehension in ["sum", "count", "all", "any"] {
        expressions.extend([
            format!("{comprehension}(v in H: v)"),
            format!("{comprehension}(v in a: H)"),
            format!("{comprehension}(v in H: v == H)"),
            format!("{comprehension}(v in H: v[0])"),
            format!("{comprehension}(v in a: sum(w in H: v + w))"),
        ]);
    }
    let statements = [
        "check H \"m\"",
        "check H",
        "if H {\n  } else if H {\n  }",
        "repeat H {\n  }",
        "t[H]: int",
        "u: H..",
        "u: 0..<H",
        "u: H..H",
        "type tt = H..H",
    ];
    // Alternate the two constants, so that each meets the other.
    let mut next = 0;
    let mut host = |text: &str| {
        text.split('H')
            .enumerate()
            .map(|(index, piece)| {
                if index == 0 {
                    piece.to_owned()
                } else {
                    next += 1;
                    let name = ["gecode.int_max", "gecode.int_min"][next % 2];
                    format!("{name}{piece}")
                }
            })
            .collect::<String>()
    };
    let mut text = String::from("param p = gecode.int_max\nx: int\na[2]: int\nrepeat 2 {\n");
    for (index, expression) in expressions.iter().enumerate() {
        text.push_str(&format!("  let h{index} = {}\n", host(expression)));
    }
    for statement in statements {
        text.push_str(&format!("  {}\n", host(statement)));
    }
    text.push_str("}\n");
    text
}

/// Names that Python or C++ reserves or the generated programs use, also
/// inside a block deep enough for a helper function, as values, types and
/// comprehension variables.
fn reserved_names() -> String {
    let names = [
        "list",
        "match",
        "case",
        "int",
        "print",
        "str",
        "ib",
        "b",
        "s1",
        "auto",
        "else",
        "e1",
        "a2",
        "c1",
        "t1",
        "block_1",
        "body_1",
        "std",
        "main",
        "namespace",
        "unix",
        "b_2",
    ];
    let mut text: String = names.iter().map(|name| format!("{name}: int\n")).collect();
    text.push_str("type range = 1..9\ntype len = range\ntype double = len\ntype t2 = double\n");
    let body = "let x = count(sum in list: sum > 0)\n\
                let y = sum(dict in list: sum(id in list: dict + id))\n\
                let w = all(e2 in list: any(s3 in list: e2 == s3))\n\
                z: len\n\
                u: t2\n";
    text.push_str(&format!(
        "{}{body}{}",
        "repeat 1 {\n".repeat(21),
        "}\n".repeat(21)
    ));
    text
}

/// The prefixes of the numbered names that the generated programs make, such
/// as `e1`, `block1` and `block_1`, each naming two values once a leading
/// underscore is gone, also inside a block deep enough for a helper function.
fn numbered_prefixes() -> String {
    let prefixes = ["e", "s", "a", "t", "c", "block", "body"];
    let mut text: String = prefixes
        .iter()
        .map(|prefix| format!("_{prefix}: int\n{prefix}: int\n"))
        .collect();
    let sum = prefixes
        .iter()
        .map(|prefix| format!("_{prefix} + {prefix}"))
        .collect::<Vec<_>>()
        .join(" + ");
    text.push_str(&format!(
        "{}let x = {sum}\n{}",
        "repeat 1 {\n".repeat(21),
        "}\n".repeat(21)
    ));
    text
}

/// Named types that blocks deep enough for helper functions define and use:
/// within one helper, passed from one helper to the next, and from the
/// outermost function, for every base type and an alphabet.
fn helper_types() -> String {
    let repeats = |count: usize| "repeat 1 {\n".repeat(count);
    format!(
        "type top = 0..9\n{}\
         type a = 1..9\ntype f = float\ntype n = num\ntype w = word\n\
         type al = alphabet \"ab\"\n{}\
         type b = a\ntype al2 = al\n{}\
         x: b\ny: f\nz: n\nv: w\nq[1][2]: al2\nk: top\n\
         type own = 1..3\ntype own2 = own\nlocal: own2\n{}",
        repeats(18),
        repeats(20),
        repeats(20),
        "}\n".repeat(58)
    )
}

/// Specs at the limits of Specification §5.12, by name. None of them is in the
/// corpus count, and the deep ones show that generation needs no stack of its
/// own and that the generated program still compiles.
pub fn limit_specs() -> Vec<(&'static str, String)> {
    let nested = |open: &str, inner: &str, close: &str, count: usize| {
        format!("{}{inner}{}", open.repeat(count), close.repeat(count))
    };
    let else_ifs: String = (1..=255)
        .map(|arm| format!("else if x == {arm} {{\n  skip line\n}}\n"))
        .collect();
    let comprehension_levels = 255;
    let comprehensions: String = (0..comprehension_levels)
        .map(|level| format!("sum(v{level} in a: "))
        .collect();
    // Each level holds the next one's list, so the chain resolves at every step.
    // A field of a repeat needs an index first, so the longest chain that name
    // resolution accepts alternates the two suffixes and stops at 254 of them.
    let record_levels = 127;
    let mut records = String::new();
    let mut chain = String::from("r0");
    for level in 0..record_levels {
        records.push_str(&format!("repeat 1 as r{level} {{\n"));
        if level > 0 {
            chain.push_str(&format!("[0].r{level}"));
        }
    }
    records.push_str("v: int\n");
    records.push_str(&"}\n".repeat(record_levels));
    vec![
        (
            "limit_repeat",
            nested("repeat 1 {\n", "v: int\n", "}\n", 256),
        ),
        (
            "limit_if",
            format!("x: int\n{}", nested("if x > 0 {\n", "y: int\n", "}\n", 256)),
        ),
        (
            "limit_arm",
            nested("sections {\n\"a\" {\n", "v: int\n", "}\n}\n", 256),
        ),
        (
            "limit_else_if",
            format!("x: int\nif x == 0 {{\n  skip line\n}}\n{else_ifs}"),
        ),
        (
            "limit_parentheses",
            format!("let x = {}\n", nested("(", "1", ")", 255)),
        ),
        ("limit_unary", format!("let x = {}1\n", "-".repeat(255))),
        (
            "limit_additions",
            format!("let x = 1{}\n", "+1".repeat(255)),
        ),
        ("limit_powers", format!("let x = 1{}\n", "^1".repeat(255))),
        (
            "limit_indices",
            format!("x: int\nlet y = x{}\n", "[0]".repeat(254)),
        ),
        ("limit_fields", format!("{records}let x = {chain}[0].v\n")),
        (
            "limit_comprehensions",
            format!(
                "let a = 1\nlet x = {comprehensions}v0{}\n",
                ")".repeat(comprehension_levels)
            ),
        ),
    ]
}

/// Whether the builder `name` ran and printed a spec equal to `original`: it
/// left `out/NAME.iris`, or `out/NAME.error` saying why not.
pub fn round_trip(out: &Path, original: &[u8], name: &str) -> Result<(), String> {
    let error = out.join(format!("{name}.error"));
    if error.exists() {
        return Err(format!("builder: {}", fs::read_to_string(error).unwrap()));
    }
    let printed = fs::read(out.join(format!("{name}.iris")))
        .map_err(|error| format!("no printed spec: {error}"))?;
    check_spec(&printed, name).map_err(|error| format!("the printed spec is rejected: {error}"))?;
    match compare(original, &printed) {
        Ok(None) => Ok(()),
        Ok(Some(difference)) => Err(difference.to_string()),
        Err(failure) => Err(failure.to_string()),
    }
}

/// The counts of a round trip.
pub struct Tally {
    /// Corpus specs that went through the builder.
    pub compared: usize,
    /// Corpus specs that `iris check` rejects.
    pub skipped: usize,
    /// Specs beyond the corpus that went through the builder.
    pub extra: usize,
    /// The size of the corpus.
    pub corpus: usize,
}

impl Tally {
    /// Print the counts and the differences, and fail on any difference, on a
    /// corpus spec that went nowhere, or on too few compared specs.
    pub fn report(&self, label: &str, differences: &[String]) {
        println!(
            "{label} round trip: compared {}, skipped {}, extra {}, differences {}",
            self.compared,
            self.skipped,
            self.extra,
            differences.len()
        );
        for difference in differences {
            println!("DIFF {difference}");
        }
        assert_eq!(self.compared + self.skipped, self.corpus);
        assert!(
            self.compared >= 770,
            "too few accepted specs: {}",
            self.compared
        );
        assert!(differences.is_empty(), "{} differences", differences.len());
    }
}
