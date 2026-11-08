//! C++ identifiers for the names of a spec.
use std::collections::{HashMap, HashSet};

/// What an identifier stands for. Iris keeps these namespaces apart, and a C++
/// function has only one for its local variables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Role {
    Value,
    Type,
    Variable,
}

/// C++ keywords, alternative tokens and the identifiers with a special
/// meaning, as of C++26.
const KEYWORDS: &[&str] = &[
    "alignas",
    "alignof",
    "and",
    "and_eq",
    "asm",
    "auto",
    "bitand",
    "bitor",
    "bool",
    "break",
    "case",
    "catch",
    "char",
    "char8_t",
    "char16_t",
    "char32_t",
    "class",
    "compl",
    "concept",
    "const",
    "consteval",
    "constexpr",
    "constinit",
    "const_cast",
    "continue",
    "contract_assert",
    "co_await",
    "co_return",
    "co_yield",
    "decltype",
    "default",
    "delete",
    "do",
    "double",
    "dynamic_cast",
    "else",
    "enum",
    "explicit",
    "export",
    "extern",
    "false",
    "final",
    "float",
    "for",
    "friend",
    "goto",
    "if",
    "import",
    "inline",
    "int",
    "long",
    "module",
    "mutable",
    "namespace",
    "new",
    "noexcept",
    "not",
    "not_eq",
    "nullptr",
    "operator",
    "or",
    "or_eq",
    "override",
    "post",
    "pre",
    "private",
    "protected",
    "public",
    "reflexpr",
    "register",
    "reinterpret_cast",
    "replaceable_if_eligible",
    "requires",
    "return",
    "short",
    "signed",
    "sizeof",
    "static",
    "static_assert",
    "static_cast",
    "struct",
    "switch",
    "template",
    "this",
    "thread_local",
    "throw",
    "trivially_relocatable_if_eligible",
    "true",
    "try",
    "typedef",
    "typeid",
    "typename",
    "union",
    "unsigned",
    "using",
    "virtual",
    "void",
    "volatile",
    "wchar_t",
    "while",
    "xor",
    "xor_eq",
];

/// Names that a builder program uses, that `using namespace iris::build`
/// brings in, or that a standard or platform header may define as a macro.
const PROGRAM_NAMES: &[&str] = &[
    // The generated program's own names.
    "b",
    "builder",
    "main",
    "error",
    "std",
    "iris",
    "build",
    "detail",
    "host",
    // The lowercase public names of `iris::build`.
    "all",
    "alphabet",
    "any",
    "bind",
    "blank",
    "count",
    "defined",
    "discard",
    "discard_as",
    "distinct",
    "dynamic_depth",
    "eol",
    "float_",
    "int_",
    "int_max",
    "int_min",
    "lit",
    "max",
    "min",
    "num",
    "over",
    "pattern",
    "pow",
    "ref",
    "rest",
    "rest_of_line",
    "size",
    "sum",
    "ty",
    "type_dims",
    "untyped",
    "validate",
    "word",
    // Macros of the C and POSIX headers and of compilers.
    "assert",
    "errno",
    "linux",
    "major",
    "makedev",
    "minor",
    "offsetof",
    "setjmp",
    "stderr",
    "stdin",
    "stdout",
    "unix",
    "va_arg",
    "va_copy",
    "va_end",
    "va_start",
];

/// The prefixes of numbered names that the program makes: temporaries, the
/// scopes and arms of blocks, `if` chains, string constants and helpers.
const NUMBERED: &[&str] = &["e", "s", "a", "c", "t", "block_", "body_"];

/// Whether `name` is a numbered program name, such as `e1` or `block_2`.
fn is_numbered(name: &str) -> bool {
    NUMBERED.iter().any(|prefix| {
        name.strip_prefix(prefix)
            .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
    })
}

/// Whether every numeric suffix of `base` makes a numbered program name, as
/// `block_2` is for `block`.
fn suffixes_are_numbered(base: &str) -> bool {
    NUMBERED
        .iter()
        .any(|prefix| prefix.strip_suffix('_') == Some(base))
}

fn reserved(name: &str) -> bool {
    KEYWORDS.contains(&name) || PROGRAM_NAMES.contains(&name) || is_numbered(name)
}

/// The widest identifier of a value, record or type before any suffix. A
/// longer name stays whole in the string that the builder prints.
const WIDEST_NAME: usize = 24;

/// The widest comprehension variable. A lambda's parameter cannot be moved out
/// of its line, so it is kept short.
const WIDEST_VARIABLE: usize = 12;

/// `name` in lower snake case, as clang-tidy's `lower_case` wants it: other
/// characters become one `_`, and a name that starts with a digit gets a
/// letter first. No identifier has `__` or a leading `_`, which C++ reserves.
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

/// Gives every name a C++ identifier that nothing else has.
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
        let identifier = self.unused(shortened(snake_case(name, "name"), widest));
        self.assigned
            .insert((role, name.to_owned()), identifier.clone());
        identifier
    }

    /// An identifier based on `name` that nothing has yet.
    pub(super) fn fresh(&mut self, name: &str, fallback: &str) -> String {
        self.unused(shortened(snake_case(name, fallback), WIDEST_NAME))
    }

    /// `base`, with the first numeric suffix that leaves it free if C++ or the
    /// program reserves it or something has it already. clang-tidy's
    /// `lower_case` refuses a trailing `_`, so there is no `base_`. A base
    /// whose every suffix is a numbered name, such as `block`, takes `_v`
    /// first, so the search ends.
    fn unused(&mut self, base: String) -> String {
        if !reserved(&base) && !self.taken.contains(&base) {
            self.taken.insert(base.clone());
            return base;
        }
        if suffixes_are_numbered(&base) {
            return self.unused(format!("{base}_v"));
        }
        let mut number = self.next_suffixes.get(&base).copied().unwrap_or(2);
        let mut candidate = format!("{base}_{number}");
        while reserved(&candidate) || self.taken.contains(&candidate) {
            number += 1;
            candidate = format!("{base}_{number}");
        }
        self.next_suffixes.insert(base, number + 1);
        self.taken.insert(candidate.clone());
        candidate
    }
}

/// `base` cut to `widest` columns, without a trailing `_` from the cut.
fn shortened(mut base: String, widest: usize) -> String {
    if base.len() > widest {
        base.truncate(widest);
        let kept = base.trim_end_matches('_').len().max(1);
        base.truncate(kept);
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_lower_snake_case() {
        assert_eq!(snake_case("nJobs", "name"), "n_jobs");
        assert_eq!(snake_case("HTTPServer", "name"), "http_server");
        assert_eq!(snake_case("a__b-c", "name"), "a_b_c");
        assert_eq!(snake_case("_x_", "name"), "x");
        assert_eq!(snake_case("9a", "name"), "name_9a");
        assert_eq!(snake_case("_", "record"), "record");
        assert_eq!(snake_case("N", "name"), "n");
    }

    #[test]
    fn reserved_names_get_a_number() {
        let mut identifiers = Identifiers::default();
        assert_eq!(identifiers.named(Role::Value, "if"), "if_2");
        assert_eq!(identifiers.named(Role::Type, "if"), "if_3");
        assert_eq!(identifiers.named(Role::Value, "int"), "int_2");
        assert_eq!(identifiers.named(Role::Value, "e1"), "e1_2");
        assert_eq!(identifiers.named(Role::Value, "count"), "count_2");
        assert_eq!(identifiers.named(Role::Value, "x"), "x");
        assert_eq!(identifiers.named(Role::Type, "x"), "x_2");
        assert_eq!(identifiers.named(Role::Value, "x"), "x");
        assert_eq!(
            identifiers.named(Role::Variable, &"v".repeat(40)),
            "vvvvvvvvvvvv"
        );
    }

    #[test]
    fn suffixes_are_the_first_that_nothing_has() {
        let mut identifiers = Identifiers::default();
        for expected in ["x", "x_2", "x_3"] {
            assert_eq!(identifiers.fresh("x", "record"), expected);
        }
        assert_eq!(identifiers.fresh("x_4", "record"), "x_4");
        assert_eq!(identifiers.fresh("x", "record"), "x_5");
    }

    #[test]
    fn every_numbered_prefix_leaves_a_suffix_free() {
        for prefix in NUMBERED {
            let word = prefix.trim_end_matches('_');
            let mut identifiers = Identifiers::default();
            let names = [
                identifiers.named(Role::Value, &format!("_{word}")),
                identifiers.named(Role::Value, word),
                identifiers.named(Role::Type, word),
                identifiers.named(Role::Variable, word),
                identifiers.fresh(word, "record"),
            ];
            let distinct: HashSet<&String> = names.iter().collect();
            assert_eq!(distinct.len(), names.len(), "{names:?}");
            assert!(!names.iter().any(|name| reserved(name)), "{names:?}");
        }
        let mut identifiers = Identifiers::default();
        assert_eq!(identifiers.named(Role::Value, "block"), "block");
        assert_eq!(identifiers.named(Role::Type, "block"), "block_v");
        assert_eq!(identifiers.named(Role::Variable, "block"), "block_v_2");
    }
}
