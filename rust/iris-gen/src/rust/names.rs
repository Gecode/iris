//! Rust identifiers for the names of a spec.
use std::collections::{HashMap, HashSet};

/// What an identifier stands for. Iris keeps these namespaces apart, and Rust
/// has only one for local variables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Role {
    Value,
    Type,
    Variable,
}

/// Rust keywords, strict and reserved, and names that the generated code uses.
const RESERVED: &[&str] = &[
    // Keywords.
    "abstract",
    "as",
    "async",
    "await",
    "become",
    "box",
    "break",
    "const",
    "continue",
    "crate",
    "do",
    "dyn",
    "else",
    "enum",
    "extern",
    "false",
    "final",
    "fn",
    "for",
    "gen",
    "if",
    "impl",
    "in",
    "let",
    "loop",
    "macro",
    "match",
    "mod",
    "move",
    "mut",
    "override",
    "priv",
    "pub",
    "ref",
    "return",
    "self",
    "static",
    "struct",
    "super",
    "trait",
    "true",
    "try",
    "type",
    "typeof",
    "unsafe",
    "unsized",
    "use",
    "virtual",
    "where",
    "while",
    "yield",
    // The generated program's own names.
    "a",
    "b",
    "s",
    "builder",
    "main",
    "gecode",
    "source",
    // The lowercase public functions of `gecode_iris::build`.
    "all",
    "alphabet",
    "any",
    "bind",
    "blank",
    "count",
    "count_over",
    "defined",
    "discard",
    "discard_as",
    "distinct",
    "eol",
    "float",
    "int",
    "lit",
    "max",
    "min",
    "num",
    "over",
    "reference",
    "rest",
    "rest_of_line",
    "size",
    "sum",
    "sum_over",
    "ty",
    "untyped",
    "validate",
    "word",
    // Names that `clippy::disallowed_names` refuses.
    "foo",
    "baz",
    "quux",
];

fn reserved(name: &str) -> bool {
    RESERVED.contains(&name) || is_temporary(name)
}

/// Whether `name` has the form of a hoisted expression's name.
fn is_temporary(name: &str) -> bool {
    name.strip_prefix('e')
        .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
}

/// `name` in snake case, as `non_snake_case` wants it, with no leading digit.
fn snake_case(name: &str, fallback: &str) -> String {
    let characters: Vec<char> = name.chars().collect();
    let mut text = String::with_capacity(name.len() + 2);
    for (index, &character) in characters.iter().enumerate() {
        if !character.is_ascii_alphanumeric() {
            if !text.is_empty() && !text.ends_with('_') {
                text.push('_');
            }
            continue;
        }
        if character.is_ascii_uppercase() && index > 0 {
            let previous = characters[index - 1];
            let next_is_lower = characters
                .get(index + 1)
                .is_some_and(char::is_ascii_lowercase);
            let boundary = previous.is_ascii_lowercase()
                || previous.is_ascii_digit()
                || (previous.is_ascii_uppercase() && next_is_lower);
            if boundary && !text.ends_with('_') {
                text.push('_');
            }
        }
        text.push(character.to_ascii_lowercase());
    }
    let text = text.trim_end_matches('_');
    match text.chars().next() {
        None => fallback.to_owned(),
        Some(first) if first.is_ascii_digit() => format!("{fallback}_{text}"),
        Some(_) => text.to_owned(),
    }
}

/// The widest comprehension variable. A closure's parameter cannot go into a
/// `let`, so one that fills the first line of its call has no layout of its own.
const WIDEST_VARIABLE: usize = 16;

/// Gives every name a Rust identifier that nothing else has.
#[derive(Debug, Default)]
pub(super) struct Identifiers {
    taken: HashSet<String>,
    assigned: HashMap<(Role, String), String>,
}

impl Identifiers {
    /// The identifier of `name`, the same one each time.
    pub(super) fn named(&mut self, role: Role, name: &str) -> String {
        if let Some(identifier) = self.assigned.get(&(role, name.to_owned())) {
            return identifier.clone();
        }
        let mut base = snake_case(name, "name");
        if role == Role::Variable {
            base.truncate(WIDEST_VARIABLE);
            base.truncate(base.trim_end_matches('_').len());
        }
        let identifier = self.unused(base);
        self.assigned
            .insert((role, name.to_owned()), identifier.clone());
        identifier
    }

    /// An identifier based on `name` that nothing has yet.
    pub(super) fn fresh(&mut self, name: &str) -> String {
        self.unused(snake_case(name, "name"))
    }

    /// `base`, with a numeric suffix if something has it already.
    fn unused(&mut self, base: String) -> String {
        let mut candidate = base.clone();
        let mut number = 2;
        while reserved(&candidate) || self.taken.contains(&candidate) {
            candidate = format!("{base}_{number}");
            number += 1;
        }
        self.taken.insert(candidate.clone());
        candidate
    }
}
