//! Rust identifiers for the names of a spec.
use std::collections::{HashMap, HashSet};

/// What an identifier stands for. Iris keeps these namespaces apart, and Rust
/// has only one for local variables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Role {
    Value,
    Type,
    Variable,
    /// A `const` handle of a view. A declaration has no `let` to shorten it.
    Handle,
}

/// Rust keywords, strict and reserved.
const KEYWORDS: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "crate",
    "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl",
    "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref",
    "return", "self", "static", "struct", "super", "trait", "true", "try", "type", "typeof",
    "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

/// Names that a handle cannot have. The `iris!` macro of `gecode-iris` puts the
/// handles beside a `spec` field, and `iris gen` names them the same way.
const HANDLE_NAMES: &[&str] = &["spec"];

/// Names that a builder program uses or that its local variables cannot have.
const PROGRAM_NAMES: &[&str] = &[
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

/// The widest handle before any numeric suffix. With the longest handle type,
/// `Field::declared(` still fits on the line of the declaration.
const WIDEST_HANDLE: usize = 40;

/// Gives every name a Rust identifier that nothing else has.
#[derive(Debug, Default)]
pub(super) struct Identifiers {
    taken: HashSet<String>,
    /// The first suffix of each base that `unused` has not tried. Nothing leaves
    /// `taken`, so a suffix it rejected once stays rejected.
    next_suffixes: HashMap<String, usize>,
    assigned: HashMap<(Role, String), String>,
    /// Whether only keywords and [`HANDLE_NAMES`] are off limits, since no
    /// program surrounds the names.
    handles_only: bool,
    /// How many candidates `unused` has rejected.
    #[cfg(test)]
    rejected: usize,
}

impl Identifiers {
    /// Identifiers for a module of handles.
    pub(super) fn for_handles() -> Self {
        Self {
            handles_only: true,
            ..Self::default()
        }
    }

    fn reserved(&self, name: &str) -> bool {
        KEYWORDS.contains(&name)
            || if self.handles_only {
                HANDLE_NAMES.contains(&name)
            } else {
                PROGRAM_NAMES.contains(&name) || is_temporary(name)
            }
    }

    /// The identifier of `name`, the same one each time.
    pub(super) fn named(&mut self, role: Role, name: &str) -> String {
        if let Some(identifier) = self.assigned.get(&(role, name.to_owned())) {
            return identifier.clone();
        }
        let mut base = snake_case(name, "name");
        let widest = match role {
            Role::Variable => WIDEST_VARIABLE,
            Role::Handle => WIDEST_HANDLE,
            Role::Value | Role::Type => base.len(),
        };
        base.truncate(widest);
        base.truncate(base.trim_end_matches('_').len());
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
        let mut number = self.next_suffixes.get(&base).copied().unwrap_or(2);
        while self.reserved(&candidate) || self.taken.contains(&candidate) {
            candidate = format!("{base}_{number}");
            number += 1;
            #[cfg(test)]
            {
                self.rejected += 1;
            }
        }
        self.next_suffixes.insert(base, number);
        self.taken.insert(candidate.clone());
        candidate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `base`, searching every suffix from the start.
    fn searched(base: &str, identifiers: &Identifiers, taken: &mut HashSet<String>) -> String {
        let mut candidate = base.to_owned();
        let mut number = 2;
        while identifiers.reserved(&candidate) || taken.contains(&candidate) {
            candidate = format!("{base}_{number}");
            number += 1;
        }
        taken.insert(candidate.clone());
        candidate
    }

    #[test]
    fn suffixes_are_the_first_that_nothing_has() {
        // Each base collides with the others' suffixed forms in some order.
        let bases = [
            "x", "x_2", "x_3", "x_2_2", "x_4", "fn", "fn_2", "e", "e1", "a", "a_2", "main",
        ];
        for mut identifiers in [Identifiers::default(), Identifiers::for_handles()] {
            let mut taken = HashSet::new();
            let mut position = 0;
            for step in 0..2_000 {
                position = (position * 7 + step + 3) % bases.len();
                let base = bases[position];
                let expected = searched(base, &identifiers, &mut taken);
                assert_eq!(identifiers.unused(base.to_owned()), expected);
            }
        }
    }

    #[test]
    fn colliding_names_cost_a_bounded_search_each() {
        const NAMES: usize = 4_000;
        let prefix = "n".repeat(WIDEST_HANDLE);
        let mut identifiers = Identifiers::for_handles();
        for index in 0..NAMES {
            identifiers.named(Role::Handle, &format!("{prefix}{index}"));
        }
        assert_eq!(identifiers.taken.len(), NAMES);
        assert!(
            identifiers.rejected <= 2 * NAMES,
            "{} candidates rejected",
            identifiers.rejected
        );
    }
}
