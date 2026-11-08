//! The imports from `dev.gecode.iris.kotlin.build` that a program needs.
//!
//! detekt refuses a wildcard import, and ktfmt removes an unused one, so the
//! program imports exactly the builder's declarations it names. They are
//! read off the finished text, which only the backend writes: string
//! literals, identifiers, numbers and punctuation, with no comments.
//! Operators that the Kotlin module provides as extensions, `%`, prefix
//! `-` and `..`, need their functions; the builder's Java methods provide
//! the others and need no import. A program writes no `..<`, whose import
//! ktfmt would remove.

use std::collections::BTreeSet;

/// The builder's top-level functions that a program calls.
const FUNCTIONS: &[&str] = &[
    "all",
    "alphabet",
    "any",
    "atLeast",
    "atMost",
    "below",
    "blank",
    "capture",
    "count",
    "defined",
    "discard",
    "distinct",
    "dynamic",
    "float",
    "int",
    "irisBuilder",
    "lit",
    "max",
    "min",
    "num",
    "over",
    "recordName",
    "range",
    "ref",
    "rest",
    "restOfLine",
    "size",
    "skipOther",
    "sum",
    "until",
    "validate",
    "word",
];

/// The builder's infix functions.
const INFIX: &[&str] = &["and", "eq", "ge", "gt", "le", "lt", "ne", "or"];

/// The builder's classes that a program names as types.
const CLASSES: &[&str] = &[
    "DeepScope",
    "IrisBuilder",
    "OptionalDeepScope",
    "OptionalScope0",
    "OptionalScope1",
    "OptionalScope2",
    "OptionalScope3",
    "Scope0",
    "Scope1",
    "Scope2",
    "Scope3",
    "SpecScope",
];

/// The keywords a program uses, after which a value starts.
const KEYWORDS: &[&str] = &["const", "fun", "private", "return", "val"];

/// The builder's declarations that `code` names, by simple name.
pub(super) fn builder_imports(code: &str) -> BTreeSet<&'static str> {
    let mut scan = Scan {
        bytes: code.as_bytes(),
        at: 0,
        used: BTreeSet::new(),
        after_operand: false,
        after_dot: false,
    };
    scan.run();
    scan.used
}

/// A scan of a program's tokens, which notes the declarations they use.
struct Scan<'a> {
    bytes: &'a [u8],
    at: usize,
    used: BTreeSet<&'static str>,
    /// Whether the last token ends an operand, so that an operator or an
    /// infix function may follow.
    after_operand: bool,
    /// Whether the last token is `.`, so that an identifier is a member.
    after_dot: bool,
}

impl Scan<'_> {
    fn run(&mut self) {
        while let Some(&byte) = self.bytes.get(self.at) {
            match byte {
                b' ' | b'\n' => self.at += 1,
                b'"' => {
                    self.string();
                    self.operand();
                }
                b'0'..=b'9' => {
                    self.number();
                    self.operand();
                }
                b'A'..=b'Z' | b'a'..=b'z' | b'_' => self.identifier(),
                _ => self.punctuation(byte),
            }
        }
    }

    fn operand(&mut self) {
        self.after_operand = true;
        self.after_dot = false;
    }

    fn operator(&mut self) {
        self.after_operand = false;
        self.after_dot = false;
    }

    fn peek(&self, offset: usize) -> Option<u8> {
        self.bytes.get(self.at + offset).copied()
    }

    fn string(&mut self) {
        self.at += 1;
        while let Some(byte) = self.peek(0) {
            self.at += if byte == b'\\' { 2 } else { 1 };
            if byte == b'"' {
                return;
            }
        }
    }

    fn digits(&mut self) {
        while self.peek(0).is_some_and(|byte| byte.is_ascii_digit()) {
            self.at += 1;
        }
    }

    /// An integer or a float, which leaves the `..` of a range.
    fn number(&mut self) {
        self.digits();
        if self.peek(0) == Some(b'.') && self.peek(1).is_some_and(|byte| byte.is_ascii_digit()) {
            self.at += 1;
            self.digits();
        }
        if matches!(self.peek(0), Some(b'e' | b'E')) {
            self.at += 1;
            if matches!(self.peek(0), Some(b'+' | b'-')) {
                self.at += 1;
            }
            self.digits();
        }
    }

    fn identifier(&mut self) {
        let start = self.at;
        while self
            .peek(0)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            self.at += 1;
        }
        let word = std::str::from_utf8(&self.bytes[start..self.at]).expect("ASCII");
        if self.after_dot {
            self.operand();
            return;
        }
        if KEYWORDS.contains(&word) {
            self.operator();
            return;
        }
        if self.after_operand
            && let Some(infix) = INFIX.iter().find(|infix| **infix == word)
        {
            self.used.insert(infix);
            self.operator();
            return;
        }
        let next = self.bytes[self.at..]
            .iter()
            .find(|byte| **byte != b' ')
            .copied();
        let declaration = if matches!(next, Some(b'(' | b'{')) {
            FUNCTIONS.iter().find(|function| **function == word)
        } else {
            CLASSES.iter().find(|class| **class == word)
        };
        if let Some(declaration) = declaration {
            self.used.insert(declaration);
        }
        self.operand();
    }

    fn punctuation(&mut self, byte: u8) {
        match byte {
            b'.' if self.peek(1) == Some(b'.') => {
                self.used.insert("rangeTo");
                self.at += 2;
                self.operator();
            }
            b'.' => {
                self.at += 1;
                self.after_operand = false;
                self.after_dot = true;
            }
            b'%' => {
                self.used.insert("rem");
                self.at += 1;
                self.operator();
            }
            b'-' if self.peek(1) == Some(b'>') => {
                self.at += 2;
                self.operator();
            }
            b'-' if !self.after_operand => {
                // A negative literal is a Kotlin number.
                if !self.peek(1).is_some_and(|byte| byte.is_ascii_digit()) {
                    self.used.insert("unaryMinus");
                }
                self.at += 1;
                self.operator();
            }
            b')' | b']' | b'}' => {
                self.at += 1;
                self.operand();
            }
            _ => {
                self.at += 1;
                self.operator();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::builder_imports;

    fn names(code: &str) -> Vec<&'static str> {
        builder_imports(code).into_iter().collect()
    }

    #[test]
    fn calls_and_classes_need_imports_but_members_do_not() {
        assert_eq!(
            names("fun specBuilder(): IrisBuilder<Unit> = irisBuilder { read(x, int()) }"),
            ["IrisBuilder", "int", "irisBuilder"]
        );
        assert_eq!(names("check(t.until(word()).dim(2))"), ["word"]);
        assert_eq!(
            names("private fun Scope2.block1(sum: DynField) {"),
            ["Scope2"]
        );
    }

    #[test]
    fn infix_functions_follow_an_operand() {
        assert_eq!(names("check(a le b)"), ["le"]);
        assert_eq!(names("check(and eq or)"), ["eq"]);
        assert_eq!(
            names("check((a + 1) gt f(2) and \"x\" ne s)"),
            ["and", "gt", "ne"]
        );
        assert_eq!(names("read(le, int())"), ["int"]);
    }

    #[test]
    fn extension_operators_need_imports() {
        assert_eq!(names("read(x, range(0).below(n))"), ["range"]);
        assert_eq!(names("read(x, lit(1)..2)"), ["lit", "rangeTo"]);
        assert_eq!(names("let(x, a % 2)"), ["rem"]);
        assert_eq!(names("let(x, -a)"), ["unaryMinus"]);
        assert_eq!(names("let(x, -(a + b) - -3)"), ["unaryMinus"]);
        assert_eq!(names("let(x, a - 3 * -1.5e-3)"), Vec::<&str>::new());
        assert_eq!(names("sum(d) { j -> j }"), ["sum"]);
    }

    #[test]
    fn strings_hide_their_text() {
        assert_eq!(names("check(lit(\"a le b % -c\\\" int(\"))"), ["lit"]);
    }
}
