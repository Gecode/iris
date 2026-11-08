//! Kotlin identifiers for the names of a spec.
use std::collections::{HashMap, HashSet};

/// What an identifier stands for. Iris keeps these namespaces apart. In Kotlin
/// a local value hides a property of the same name, so every name gets an
/// identifier of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Role {
    Value,
    Type,
    Variable,
}

/// Kotlin's hard keywords, and `_`. Soft and modifier keywords are free as
/// names.
const KEYWORDS: &[&str] = &[
    "_",
    "as",
    "break",
    "class",
    "continue",
    "do",
    "else",
    "false",
    "for",
    "fun",
    "if",
    "in",
    "interface",
    "is",
    "null",
    "object",
    "package",
    "return",
    "super",
    "this",
    "throw",
    "true",
    "try",
    "typealias",
    "typeof",
    "val",
    "var",
    "when",
    "while",
];

/// The properties of the builder's scope classes, which a block's statements
/// see before the program's own properties. Functions have a namespace of
/// their own, so the builder's functions and the program's are free as names.
const SCOPE_PROPERTIES: &[&str] = &["depth", "record", "session"];

/// The prefix of the temporaries `e1`, `e2` and so on.
const TEMPORARY: &str = "e";

/// Whether `name` is a temporary, such as `e1`.
fn is_numbered(name: &str) -> bool {
    name.strip_prefix(TEMPORARY)
        .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
}

fn reserved(name: &str) -> bool {
    KEYWORDS.contains(&name) || SCOPE_PROPERTIES.contains(&name) || is_numbered(name)
}

/// The widest identifier of a value, record or type before any suffix. A
/// longer name stays whole in the string that the builder prints.
const WIDEST_NAME: usize = 24;

/// The widest comprehension variable, which a lambda repeats.
const WIDEST_VARIABLE: usize = 12;

/// `name` in lower camel case, as Kotlin's coding conventions want it: other
/// characters separate words, each word after the first starts with a capital,
/// and a name that starts with a digit gets `fallback` first.
fn camel_case(name: &str, fallback: &str) -> String {
    let characters: Vec<char> = name.chars().collect();
    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    for (index, &character) in characters.iter().enumerate() {
        if !character.is_ascii_alphanumeric() {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            continue;
        }
        if character.is_ascii_uppercase() && index > 0 && !word.is_empty() {
            let previous = characters[index - 1];
            let next_is_lower = characters
                .get(index + 1)
                .is_some_and(char::is_ascii_lowercase);
            let boundary = previous.is_ascii_lowercase()
                || previous.is_ascii_digit()
                || (previous.is_ascii_uppercase() && next_is_lower);
            if boundary {
                words.push(std::mem::take(&mut word));
            }
        }
        word.push(character.to_ascii_lowercase());
    }
    if !word.is_empty() {
        words.push(word);
    }
    let mut text = String::with_capacity(name.len() + fallback.len());
    for (index, word) in words.iter().enumerate() {
        if index == 0 {
            text.push_str(word);
        } else {
            let mut letters = word.chars();
            if let Some(first) = letters.next() {
                text.push(first.to_ascii_uppercase());
                text.extend(letters);
            }
        }
    }
    match text.chars().next() {
        None => fallback.to_owned(),
        Some(first) if first.is_ascii_digit() => format!("{fallback}{text}"),
        Some(_) => text,
    }
}

/// Gives every name a Kotlin identifier that nothing else has.
#[derive(Debug, Default)]
pub(super) struct Identifiers {
    taken: HashSet<String>,
    /// The first suffix of each base that `unused` has not tried.
    next_suffixes: HashMap<String, usize>,
    assigned: HashMap<(Role, String), String>,
}

impl Identifiers {
    /// The identifier of `name`, the same one each time.
    pub(super) fn named(&mut self, role: Role, name: &str) -> String {
        if let Some(identifier) = self.assigned.get(&(role, name.to_owned())) {
            return identifier.clone();
        }
        let widest = match role {
            Role::Variable => WIDEST_VARIABLE,
            Role::Value | Role::Type => WIDEST_NAME,
        };
        let identifier = self.unused(shortened(camel_case(name, "name"), widest));
        self.assigned
            .insert((role, name.to_owned()), identifier.clone());
        identifier
    }

    /// An identifier based on `name` that nothing has yet.
    pub(super) fn fresh(&mut self, name: &str, fallback: &str) -> String {
        self.unused(shortened(camel_case(name, fallback), WIDEST_NAME))
    }

    /// `base`, with the first numeric suffix that leaves it free if Kotlin or
    /// the builder reserves it or something has it already. A temporary's name,
    /// or `e` itself, takes a `v` first, so no suffix makes a temporary's name
    /// and the search ends.
    fn unused(&mut self, base: String) -> String {
        // Every name of the form `e1`, `e12` is a temporary's, so no suffix
        // frees one.
        let base = if is_numbered(&base) {
            format!("{base}v")
        } else {
            base
        };
        if !reserved(&base) && !self.taken.contains(&base) {
            self.taken.insert(base.clone());
            return base;
        }
        // Every suffix of `e` makes a temporary's name.
        if base == TEMPORARY {
            return self.unused(format!("{base}v"));
        }
        let mut number = self.next_suffixes.get(&base).copied().unwrap_or(2);
        let mut candidate = format!("{base}{number}");
        while reserved(&candidate) || self.taken.contains(&candidate) {
            number += 1;
            candidate = format!("{base}{number}");
        }
        self.next_suffixes.insert(base, number + 1);
        self.taken.insert(candidate.clone());
        candidate
    }
}

/// `base` cut to `widest` characters.
fn shortened(mut base: String, widest: usize) -> String {
    base.truncate(widest);
    base
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_lower_camel_case() {
        assert_eq!(camel_case("n_jobs", "name"), "nJobs");
        assert_eq!(camel_case("nJobs", "name"), "nJobs");
        assert_eq!(camel_case("HTTPServer", "name"), "httpServer");
        assert_eq!(camel_case("a__b-c", "name"), "aBC");
        assert_eq!(camel_case("_x_", "name"), "x");
        assert_eq!(camel_case("9a", "name"), "name9a");
        assert_eq!(camel_case("_", "record"), "record");
        assert_eq!(camel_case("N", "name"), "n");
        assert_eq!(camel_case("N_MAX", "name"), "nMax");
    }

    #[test]
    fn reserved_names_get_a_number() {
        let mut identifiers = Identifiers::default();
        assert_eq!(identifiers.named(Role::Value, "if"), "if2");
        assert_eq!(identifiers.named(Role::Type, "if"), "if3");
        assert_eq!(identifiers.named(Role::Value, "val"), "val2");
        assert_eq!(identifiers.named(Role::Value, "e1"), "e1v");
        assert_eq!(identifiers.named(Role::Value, "record"), "record2");
        assert_eq!(identifiers.named(Role::Value, "data"), "data");
        assert_eq!(identifiers.named(Role::Value, "sum"), "sum");
        assert_eq!(identifiers.named(Role::Value, "x"), "x");
        assert_eq!(identifiers.named(Role::Type, "x"), "x2");
        assert_eq!(identifiers.named(Role::Value, "x"), "x");
        assert_eq!(
            identifiers.named(Role::Variable, &"v".repeat(40)),
            "vvvvvvvvvvvv"
        );
    }

    #[test]
    fn suffixes_are_the_first_that_nothing_has() {
        let mut identifiers = Identifiers::default();
        for expected in ["x", "x2", "x3"] {
            assert_eq!(identifiers.fresh("x", "record"), expected);
        }
        assert_eq!(identifiers.fresh("x4", "record"), "x4");
        assert_eq!(identifiers.fresh("x", "record"), "x5");
    }

    #[test]
    fn the_temporary_prefix_takes_a_letter_before_a_suffix() {
        let mut identifiers = Identifiers::default();
        let names = [
            identifiers.named(Role::Value, "_e"),
            identifiers.named(Role::Value, "e"),
            identifiers.named(Role::Type, "e"),
            identifiers.named(Role::Variable, "e"),
            identifiers.fresh("e", "record"),
        ];
        assert_eq!(names, ["e", "ev", "ev2", "ev3", "ev4"]);
        assert!(!names.iter().any(|name| reserved(name)), "{names:?}");
    }
}
