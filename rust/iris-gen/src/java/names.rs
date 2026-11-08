//! Java identifiers for the names of a spec.
use std::collections::{HashMap, HashSet};

/// What an identifier stands for. Iris keeps these namespaces apart. In Java a
/// local variable hides a field of the same name, so every name gets an
/// identifier of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Role {
    Value,
    Type,
    Variable,
}

/// Java's keywords, literals and contextual keywords, as of Java 27, and `_`.
const KEYWORDS: &[&str] = &[
    "_",
    "abstract",
    "assert",
    "boolean",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "class",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "exports",
    "extends",
    "false",
    "final",
    "finally",
    "float",
    "for",
    "goto",
    "if",
    "implements",
    "import",
    "instanceof",
    "int",
    "interface",
    "long",
    "module",
    "native",
    "new",
    "null",
    "open",
    "opens",
    "package",
    "permits",
    "private",
    "protected",
    "provides",
    "public",
    "record",
    "requires",
    "return",
    "sealed",
    "short",
    "static",
    "strictfp",
    "super",
    "switch",
    "synchronized",
    "this",
    "throw",
    "throws",
    "to",
    "transient",
    "transitive",
    "true",
    "try",
    "uses",
    "var",
    "void",
    "volatile",
    "when",
    "while",
    "with",
    "yield",
];

/// The variables of the generated program: `main`'s parameter. Methods have
/// a namespace of their own, so the builder's methods and the program's are
/// free as variable names.
const PROGRAM_NAMES: &[&str] = &["args"];

/// The prefix of the temporaries `e1`, `e2` and so on.
const TEMPORARY: &str = "e";

/// Whether `name` is a temporary, such as `e1`.
fn is_numbered(name: &str) -> bool {
    name.strip_prefix(TEMPORARY)
        .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
}

fn reserved(name: &str) -> bool {
    KEYWORDS.contains(&name) || PROGRAM_NAMES.contains(&name) || is_numbered(name)
}

/// The names that a view's handles leave free: the members that the annotation
/// processor adds beside them, `spec` and `reader`, the names that a record
/// component may not have, since the processor's record takes the handles'
/// identifiers, and `java`, which would hide the package of the qualified
/// names that the processor's class uses in expressions. `iris gen --lang
/// java` and the processor give a spec the same identifiers.
const VIEW_NAMES: &[&str] = &[
    "clone",
    "finalize",
    "getClass",
    "hashCode",
    "java",
    "notify",
    "notifyAll",
    "reader",
    "spec",
    "toString",
    "wait",
];

/// Whether a view may not use `name` for a handle.
fn reserved_in_view(name: &str) -> bool {
    KEYWORDS.contains(&name) || VIEW_NAMES.contains(&name)
}

/// The simple names of the types that a view names, which its class may not
/// take: `java.lang`'s value types, and the imported ones.
const VIEW_TYPES: &[&str] = &[
    "Double",
    "DynField",
    "FloatField",
    "IntField",
    "List",
    "Long",
    "Nullable",
    "NumField",
    "Number",
    "Object",
    "String",
    "StringField",
    "Value",
];

/// The widest identifier of a value, record or type before any suffix. A
/// longer name stays whole in the string that the builder prints.
const WIDEST_NAME: usize = 24;

/// The widest comprehension variable, which a lambda repeats.
const WIDEST_VARIABLE: usize = 12;

/// `name` in lower camel case, as the Google Java style wants it: other
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

/// Gives every name a Java identifier that nothing else has.
#[derive(Debug, Default)]
pub(super) struct Identifiers {
    /// Whether the identifiers name a view's handles, which need no temporaries
    /// and leave the names of [`VIEW_NAMES`] free.
    handles_only: bool,
    taken: HashSet<String>,
    /// The first suffix of each base that `unused` has not tried.
    next_suffixes: HashMap<String, usize>,
    assigned: HashMap<(Role, String), String>,
}

impl Identifiers {
    /// Identifiers for the handles of a view.
    pub(super) fn for_handles() -> Self {
        Self {
            handles_only: true,
            ..Self::default()
        }
    }

    fn reserved(&self, name: &str) -> bool {
        if self.handles_only {
            reserved_in_view(name)
        } else {
            reserved(name)
        }
    }

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

    /// `base`, with the first numeric suffix that leaves it free if Java or
    /// the program reserves it or something has it already. A temporary's name,
    /// or `e` itself, takes a `v` first, so no suffix makes a temporary's name
    /// and the search ends.
    fn unused(&mut self, base: String) -> String {
        // Every name of the form `e1`, `e12` is a temporary's, so no suffix
        // frees one.
        let base = if !self.handles_only && is_numbered(&base) {
            format!("{base}v")
        } else {
            base
        };
        if !self.reserved(&base) && !self.taken.contains(&base) {
            self.taken.insert(base.clone());
            return base;
        }
        // Every suffix of `e` makes a temporary's name.
        if !self.handles_only && base == TEMPORARY {
            return self.unused(format!("{base}v"));
        }
        let mut number = self.next_suffixes.get(&base).copied().unwrap_or(2);
        let mut candidate = format!("{base}{number}");
        while self.reserved(&candidate) || self.taken.contains(&candidate) {
            number += 1;
            candidate = format!("{base}{number}");
        }
        self.next_suffixes.insert(base, number + 1);
        self.taken.insert(candidate.clone());
        candidate
    }
}

/// The class of a view of the spec read from `origin`: its file name without
/// the extension, in upper camel case, as the Google Java style wants a class
/// name. A name that a view's types take gets a number.
pub(super) fn view_class(origin: &str) -> String {
    let class = upper_camel_case(&file_stem(origin));
    if !VIEW_TYPES.contains(&class.as_str()) {
        return class;
    }
    let mut number = 2;
    while VIEW_TYPES.contains(&format!("{class}{number}").as_str()) {
        number += 1;
    }
    format!("{class}{number}")
}

/// The file name of `origin` without its extension. A Windows path separates
/// with `\`, which `Path` sees only on Windows.
fn file_stem(origin: &str) -> String {
    let file = origin.rsplit(['/', '\\']).next().unwrap_or(origin);
    match file.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem.to_owned(),
        _ => file.to_owned(),
    }
}

/// `name` in upper camel case, with `Spec` before a name that would start with
/// a digit or be empty.
fn upper_camel_case(name: &str) -> String {
    let lower = camel_case(name, "spec");
    let mut letters = lower.chars();
    letters.next().map_or_else(String::new, |first| {
        first.to_ascii_uppercase().to_string() + letters.as_str()
    })
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
        assert_eq!(identifiers.named(Role::Value, "var"), "var2");
        assert_eq!(identifiers.named(Role::Value, "e1"), "e1v");
        assert_eq!(identifiers.named(Role::Value, "args"), "args2");
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

    #[test]
    fn handles_leave_keywords_and_the_processor_members_free() {
        let mut identifiers = Identifiers::for_handles();
        assert_eq!(identifiers.named(Role::Value, "e"), "e");
        assert_eq!(identifiers.named(Role::Value, "e1"), "e1");
        assert_eq!(identifiers.named(Role::Value, "args"), "args");
        assert_eq!(identifiers.named(Role::Value, "var"), "var2");
        assert_eq!(identifiers.named(Role::Value, "spec"), "spec2");
        assert_eq!(identifiers.named(Role::Value, "reader"), "reader2");
        assert_eq!(identifiers.named(Role::Value, "java"), "java2");
        assert_eq!(identifiers.named(Role::Value, "hash_code"), "hashCode2");
        assert_eq!(identifiers.named(Role::Value, "hashCode"), "hashCode3");
    }

    #[test]
    fn classes_are_named_after_the_file() {
        assert_eq!(view_class("formats/jobshop/jobshop.iris"), "Jobshop");
        assert_eq!(view_class("C:\\specs\\dimacs-cnf.iris"), "DimacsCnf");
        assert_eq!(view_class("<stdin>"), "Stdin");
        assert_eq!(view_class("9.iris"), "Spec9");
        assert_eq!(view_class(".iris"), "Iris");
        assert_eq!(view_class("value.iris"), "Value2");
        assert_eq!(view_class("list"), "List2");
    }
}
