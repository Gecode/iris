//! TypeScript identifiers for the names of a spec.
use std::collections::{HashMap, HashSet};

/// What an identifier stands for. Iris keeps these namespaces apart, but in
/// TypeScript they all share the builder function's scope, and oxlint's
/// `no-shadow` refuses an inner one that hides an outer one, so every name
/// gets an identifier of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Role {
    Value,
    Type,
    Variable,
}

/// JavaScript's reserved words in strict mode code, the names that strict
/// mode refuses to bind, and the global values that oxlint's
/// `no-shadow-restricted-names` keeps: `undefined`, `NaN`, `Infinity` and
/// `globalThis`. Other globals, such as `Object` or `require`, are free: of
/// them the program uses only `process`, which `PROGRAM_NAMES` holds.
const KEYWORDS: &[&str] = &[
    "arguments",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "eval",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "globalThis",
    "if",
    "implements",
    "import",
    "in",
    "Infinity",
    "instanceof",
    "interface",
    "let",
    "NaN",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "undefined",
    "var",
    "void",
    "while",
    "with",
    "yield",
];

/// The names that a program imports from `gecode-iris/build` or uses itself.
/// Every export is reserved, used or not, so a spec's identifiers do not
/// depend on what else it holds.
const PROGRAM_NAMES: &[&str] = &[
    "Alphabet",
    "Arms",
    "BuildError",
    "BuiltSpec",
    "Discard",
    "Eol",
    "Expr",
    "Field",
    "HandleDepth",
    "If",
    "ListReading",
    "Lookup",
    "NestedScope",
    "Over",
    "Part",
    "Pattern",
    "Presence",
    "Range",
    "Record",
    "RecordExpr",
    "RecordList",
    "Scope",
    "SpecBuilder",
    "Type",
    "all",
    "alphabet",
    "any",
    "atLeast",
    "atMost",
    "b",
    "below",
    "bind",
    "blank",
    "builder",
    "count",
    "defined",
    "discard",
    "discardAs",
    "distinct",
    "div",
    "eol",
    "float",
    "floatLit",
    "gecode",
    "int",
    "lit",
    "max",
    "min",
    "num",
    "over",
    "p",
    "pattern",
    "pow",
    "process",
    "range",
    "ref",
    "rem",
    "rest",
    "restOfLine",
    "size",
    "spec",
    "sum",
    "trackCaller",
    "validate",
    "word",
];

/// The prefixes of the numbered names that a program makes: scopes `s1`,
/// arm lists `a1`, temporaries `e1`, string constants `t1`, and the
/// functions `block1` and `body1`.
const NUMBERED: &[&str] = &["s", "a", "e", "t", "block", "body"];

/// Whether `name` is one of a program's numbered names, such as `e1`.
fn is_numbered(name: &str) -> bool {
    NUMBERED.iter().any(|prefix| {
        name.strip_prefix(prefix)
            .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
    })
}

fn reserved(name: &str) -> bool {
    KEYWORDS.contains(&name) || PROGRAM_NAMES.contains(&name) || is_numbered(name)
}

/// The widest identifier of a value, record or type before any suffix. A
/// longer name stays whole in the string that the builder prints.
const WIDEST_NAME: usize = 24;

/// The widest comprehension variable, which an arrow function repeats.
const WIDEST_VARIABLE: usize = 12;

/// Gives every name a TypeScript identifier that nothing else has. An Iris
/// name is ASCII `[A-Za-z_][A-Za-z0-9_]*`, which is a JavaScript identifier,
/// so it keeps its spelling unless something else has it.
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
        let identifier = self.unused(shortened(name, widest));
        self.assigned
            .insert((role, name.to_owned()), identifier.clone());
        identifier
    }

    /// An identifier based on `name` that nothing has yet.
    pub(super) fn fresh(&mut self, name: &str) -> String {
        self.unused(shortened(name, WIDEST_NAME))
    }

    /// `base`, with the first numeric suffix that leaves it free if
    /// JavaScript or the program reserves it or something has it already.
    /// A numbered name, or a bare prefix of one, takes a `v` first, so no
    /// suffix makes a numbered name and the search ends.
    fn unused(&mut self, base: String) -> String {
        // Every name of the form `e1` or `s12` is the program's own, so no
        // suffix frees one.
        let base = if is_numbered(&base) {
            format!("{base}v")
        } else {
            base
        };
        if !reserved(&base) && !self.taken.contains(&base) {
            self.taken.insert(base.clone());
            return base;
        }
        // Every suffix of a bare prefix such as `e` makes a numbered name.
        if NUMBERED.contains(&base.as_str()) {
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

/// `name` cut to `widest` characters, which are ASCII, without an
/// underscore at either end: oxlint's `no-underscore-dangle` refuses one in
/// any name but `_`.
fn shortened(name: &str, widest: usize) -> String {
    let name = &name[..name.len().min(widest)];
    if name == "_" {
        return name.to_owned();
    }
    match name.trim_matches('_') {
        "" => "u".to_owned(),
        trimmed => trimmed.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_keep_their_spelling() {
        let mut identifiers = Identifiers::default();
        assert_eq!(identifiers.named(Role::Value, "n_jobs"), "n_jobs");
        assert_eq!(identifiers.named(Role::Value, "nJobs"), "nJobs");
        assert_eq!(identifiers.named(Role::Value, "_"), "_");
        assert_eq!(identifiers.named(Role::Value, "_name"), "name");
        assert_eq!(identifiers.named(Role::Value, "name"), "name2");
        assert_eq!(identifiers.named(Role::Value, "__"), "u");
        assert_eq!(identifiers.named(Role::Value, "N_MAX"), "N_MAX");
    }

    #[test]
    fn reserved_names_get_a_number() {
        let mut identifiers = Identifiers::default();
        assert_eq!(identifiers.named(Role::Value, "if"), "if2");
        assert_eq!(identifiers.named(Role::Type, "if"), "if3");
        assert_eq!(identifiers.named(Role::Value, "int"), "int2");
        assert_eq!(identifiers.named(Role::Value, "undefined"), "undefined2");
        assert_eq!(identifiers.named(Role::Value, "globalThis"), "globalThis2");
        assert_eq!(identifiers.named(Role::Value, "e1"), "e1v");
        assert_eq!(identifiers.named(Role::Value, "s12"), "s12v");
        assert_eq!(identifiers.named(Role::Value, "block_1"), "block_1");
        assert_eq!(identifiers.named(Role::Value, "body1"), "body1v");
        assert_eq!(identifiers.named(Role::Value, "b"), "b2");
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
            assert_eq!(identifiers.fresh("x"), expected);
        }
        assert_eq!(identifiers.fresh("x4"), "x4");
        assert_eq!(identifiers.fresh("x"), "x5");
    }

    #[test]
    fn bare_prefixes_take_a_letter_before_a_suffix() {
        for prefix in NUMBERED {
            let mut identifiers = Identifiers::default();
            let names = [
                identifiers.named(Role::Value, &format!("_{prefix}")),
                identifiers.named(Role::Value, prefix),
                identifiers.named(Role::Type, prefix),
                identifiers.named(Role::Variable, prefix),
                identifiers.fresh(prefix),
            ];
            assert_eq!(names[..2], [(*prefix).to_owned(), format!("{prefix}v")]);
            let distinct: HashSet<&String> = names.iter().collect();
            assert_eq!(distinct.len(), names.len(), "{names:?}");
            assert!(!names.iter().any(|name| reserved(name)), "{names:?}");
        }
    }
}
