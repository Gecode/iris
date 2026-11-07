//! Python identifiers for the names of a spec.
use std::collections::{HashMap, HashSet};

/// What an identifier stands for. Iris keeps these namespaces apart, and a
/// Python function has only one for its local variables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Role {
    Value,
    Type,
    Variable,
}

/// Python keywords in every version from 3.10 on.
const KEYWORDS: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue",
    "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if", "import",
    "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while",
    "with", "yield",
];

/// Python's soft keywords from 3.10 on. A variable may have one of these names,
/// but it reads as the statement, and `_` is the program's unused lambda
/// parameter.
const SOFT_KEYWORDS: &[&str] = &["_", "case", "match", "type"];

/// The names in `builtins` of Python 3.10 and of Python 3.15, which has every
/// name of 3.10, without the dunder names that `reserved` refuses anyway. A
/// variable with one of them hides the builtin from the rest of the function,
/// and the program uses `int` in annotations and `print` to write the spec.
const BUILTINS: &[&str] = &[
    "ArithmeticError",
    "AssertionError",
    "AttributeError",
    "BaseException",
    "BaseExceptionGroup",
    "BlockingIOError",
    "BrokenPipeError",
    "BufferError",
    "BytesWarning",
    "ChildProcessError",
    "ConnectionAbortedError",
    "ConnectionError",
    "ConnectionRefusedError",
    "ConnectionResetError",
    "DeprecationWarning",
    "EOFError",
    "Ellipsis",
    "EncodingWarning",
    "EnvironmentError",
    "Exception",
    "ExceptionGroup",
    "False",
    "FileExistsError",
    "FileNotFoundError",
    "FloatingPointError",
    "FutureWarning",
    "GeneratorExit",
    "IOError",
    "ImportCycleError",
    "ImportError",
    "ImportWarning",
    "IndentationError",
    "IndexError",
    "InterruptedError",
    "IsADirectoryError",
    "KeyError",
    "KeyboardInterrupt",
    "LookupError",
    "MemoryError",
    "ModuleNotFoundError",
    "NameError",
    "None",
    "NotADirectoryError",
    "NotImplemented",
    "NotImplementedError",
    "OSError",
    "OverflowError",
    "PendingDeprecationWarning",
    "PermissionError",
    "ProcessLookupError",
    "PythonFinalizationError",
    "RecursionError",
    "ReferenceError",
    "ResourceWarning",
    "RuntimeError",
    "RuntimeWarning",
    "StopAsyncIteration",
    "StopIteration",
    "SyntaxError",
    "SyntaxWarning",
    "SystemError",
    "SystemExit",
    "TabError",
    "TimeoutError",
    "True",
    "TypeError",
    "UnboundLocalError",
    "UnicodeDecodeError",
    "UnicodeEncodeError",
    "UnicodeError",
    "UnicodeTranslateError",
    "UnicodeWarning",
    "UserWarning",
    "ValueError",
    "Warning",
    "ZeroDivisionError",
    "_IncompleteInputError",
    "abs",
    "aiter",
    "all",
    "anext",
    "any",
    "ascii",
    "bin",
    "bool",
    "breakpoint",
    "bytearray",
    "bytes",
    "callable",
    "chr",
    "classmethod",
    "compile",
    "complex",
    "copyright",
    "credits",
    "delattr",
    "dict",
    "dir",
    "divmod",
    "enumerate",
    "eval",
    "exec",
    "exit",
    "filter",
    "float",
    "format",
    "frozendict",
    "frozenset",
    "getattr",
    "globals",
    "hasattr",
    "hash",
    "help",
    "hex",
    "id",
    "input",
    "int",
    "isinstance",
    "issubclass",
    "iter",
    "len",
    "license",
    "list",
    "locals",
    "map",
    "max",
    "memoryview",
    "min",
    "next",
    "object",
    "oct",
    "open",
    "ord",
    "pow",
    "print",
    "property",
    "quit",
    "range",
    "repr",
    "reversed",
    "round",
    "sentinel",
    "set",
    "setattr",
    "slice",
    "sorted",
    "staticmethod",
    "str",
    "sum",
    "super",
    "tuple",
    "type",
    "vars",
    "zip",
];

/// Names that a builder program uses, or that a linter refuses for a variable:
/// E741 refuses `l`, `O` and `I`.
const PROGRAM_NAMES: &[&str] = &["b", "ib", "builder", "l", "O", "I"];

/// The prefixes of numbered names that the program makes: temporaries, the
/// scopes and arms of blocks, and helper functions.
const NUMBERED: &[&str] = &["e", "s", "a", "block_", "body_"];

/// Whether `name` is a numbered program name, such as `e1` or `block_2`.
fn is_numbered(name: &str) -> bool {
    NUMBERED.iter().any(|prefix| {
        name.strip_prefix(prefix)
            .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
    })
}

fn reserved(name: &str) -> bool {
    KEYWORDS.contains(&name)
        || SOFT_KEYWORDS.contains(&name)
        || BUILTINS.contains(&name)
        || PROGRAM_NAMES.contains(&name)
        || is_numbered(name)
        || (name.len() > 4 && name.starts_with("__") && name.ends_with("__"))
}

/// The widest identifier of a value, record or type before any suffix. A
/// longer name stays whole in the string that the builder prints.
const WIDEST_NAME: usize = 24;

/// The widest comprehension variable. A lambda's parameter cannot be moved out
/// of its line, so it is kept short.
const WIDEST_VARIABLE: usize = 12;

/// `name` as an ASCII identifier: other characters become `_`, and a name
/// that starts with a digit gets a letter first.
fn identifier(name: &str, fallback: &str) -> String {
    let text: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect();
    if text.bytes().all(|byte| byte == b'_') {
        return fallback.to_owned();
    }
    if text.starts_with(|character: char| character.is_ascii_digit()) {
        return format!("{fallback}_{text}");
    }
    text
}

/// Gives every name a Python identifier that nothing else has.
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
        let identifier = self.unused(shortened(identifier(name, "name"), widest));
        self.assigned
            .insert((role, name.to_owned()), identifier.clone());
        identifier
    }

    /// An identifier based on `name` that nothing has yet.
    pub(super) fn fresh(&mut self, name: &str, fallback: &str) -> String {
        self.unused(shortened(identifier(name, fallback), WIDEST_NAME))
    }

    /// `base`, or `base_` if Python or the program reserves it, with a numeric
    /// suffix if something has it already.
    fn unused(&mut self, base: String) -> String {
        if !reserved(&base) && !self.taken.contains(&base) {
            self.taken.insert(base.clone());
            return base;
        }
        let underscored = format!("{base}_");
        if reserved(&base) && !reserved(&underscored) && !self.taken.contains(&underscored) {
            self.taken.insert(underscored.clone());
            return underscored;
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
    fn reserved_names_get_an_underscore_then_a_number() {
        let mut identifiers = Identifiers::default();
        assert_eq!(identifiers.named(Role::Value, "if"), "if_");
        assert_eq!(identifiers.named(Role::Type, "if"), "if_2");
        assert_eq!(identifiers.named(Role::Value, "if_"), "if__2");
        assert_eq!(identifiers.named(Role::Value, "e1"), "e1_");
        assert_eq!(identifiers.named(Role::Value, "l"), "l_");
        assert_eq!(identifiers.named(Role::Value, "match"), "match_");
        assert_eq!(identifiers.named(Role::Value, "int"), "int_");
        assert_eq!(identifiers.named(Role::Value, "frozendict"), "frozendict_");
        assert_eq!(identifiers.named(Role::Value, "x"), "x");
        assert_eq!(identifiers.named(Role::Type, "x"), "x_2");
        assert_eq!(identifiers.named(Role::Value, "x"), "x");
        assert_eq!(identifiers.named(Role::Value, "__init__"), "__init___2");
        assert_eq!(identifiers.named(Role::Value, "9a"), "name_9a");
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
        assert_eq!(identifiers.fresh("_", "record"), "record");
    }
}
