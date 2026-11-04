//! The Specification files that the runner embeds, compared with `spec/0.1`.
//!
//! The crate embeds copies so that it builds outside the repository, as in
//! the Python source distribution.

use std::fs;
use std::path::Path;

fn read(path: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join(path)).unwrap()
}

#[test]
fn embedded_grammar_matches_the_specification() {
    assert!(
        read("embedded/0.1/grammar.ebnf") == read("../../spec/0.1/grammar.ebnf"),
        "copy spec/0.1/grammar.ebnf to rust/iris-conformance/embedded/0.1/grammar.ebnf"
    );
}

#[test]
fn embedded_error_codes_match_the_registry() {
    let registered: String = read("../../spec/0.1/errors.md")
        .lines()
        .filter_map(|line| line.strip_prefix("| `"))
        .filter_map(|tail| tail.split_once('`'))
        .filter(|(code, _)| code.starts_with("E-"))
        .map(|(code, _)| format!("{code}\n"))
        .collect();
    assert_eq!(
        read("embedded/0.1/error-codes.txt"),
        registered,
        "list each code in the first column of spec/0.1/errors.md, in order, in \
         rust/iris-conformance/embedded/0.1/error-codes.txt"
    );
}
